/**
 * @file dashboard.js
 * @description Client-side controller for the Soroban Budget Assert cost-over-time dashboard.
 *
 * ## Overview
 * This module is loaded by `dashboard.html` and is responsible for the full
 * client-side dashboard pipeline:
 *
 *   1. Read query-parameter configuration from the page URL.
 *   2. Fetch a `history.json` dataset (same-origin or cross-origin, caller-supplied).
 *   3. Validate the shape of the fetched document and classify failures into
 *      distinct on-page error states (network / HTTP / JSON / shape).
 *   4. Pivot the flat row-per-measurement input into a nested
 *      `package → function → metric` index for fast lookups.
 *   5. Build the interactive controls (package selector, function checkboxes).
 *   6. Render one Chart.js line chart per selected metric, with a per-function
 *      %-change summary and click-to-open-GitHub-commit interactions.
 *
 * ## Security model
 * The `?history=` query parameter is deliberately unrestricted — consumers are
 * expected to point the dashboard at `history.json` files hosted by CI on
 * arbitrary origins. Because the fetched document is fully untrusted, every
 * value originating from it is inserted into the DOM via `textContent` /
 * `document.createTextNode` only — `innerHTML` is never used with fetched
 * data, and URLs/identifiers are coerced to plain text before display.
 *
 * ## Data contract (history.json)
 * The top-level value MUST be an array of entries of the form:
 *
 * ```json
 * {
 *   "commit":     "abcdef0123456789...",
 *   "timestamp":  "2024-01-15T12:34:56Z",
 *   "data": [
 *     { "package": "amm-pool-contract", "function": "do_work",
 *       "metric":  "CPU Instructions",   "value":    950000 },
 *     ...
 *   ]
 * }
 * ```
 *
 * Rows missing `value`, or entries missing a `data` array, are skipped rather
 * than hard-failing the whole page, so a single bad CI artifact does not
 * blank the entire history view.
 *
 * ## Query parameters
 * | Param      | Default            | Purpose                                               |
 * |------------|--------------------|-------------------------------------------------------|
 * | `history`  | `./history.json`   | URL to the `history.json` dataset to load.            |
 * | `limit`    | `200`              | Maximum number of most-recent commits to render.      |
 * | `repo`     | auto-detect        | `owner/name` GitHub repo used to link commit points.  |
 *
 * @see site/dashboard.html       The page that embeds this script.
 * @see site/scripts/validate.sh Pre-deploy syntax/link validation script.
 */
(function () {
  // Data-source policy: the `?history=` query parameter is deliberately left
  // open — anyone can hand this page a URL. That is why every value read
  // from the fetched document is treated as untrusted text and inserted with
  // `textContent` (never `innerHTML`), and why the failure states below say
  // exactly what was tried and what went wrong. Constraining the parameter
  // to same-origin was rejected because the documented use case — pointing
  // the dashboard at a `history.json` published by CI on a different host —
  // would break; instead the page renders arbitrary supplied data as text.
  const params = new URLSearchParams(location.search);

  /**
   * Resolved URL from which the history dataset will be fetched.
   * Defaults to `./history.json` (same-origin) unless overridden by the
   * `?history=` query parameter. Cross-origin URLs are allowed; the browser's
   * CORS policy governs whether the fetch actually succeeds.
   * @type {string}
   */
  const HISTORY_URL = params.get('history') || './history.json';

  /**
   * Number of most-recent commits kept from the full history before rendering.
   * Defaults to `200`; a non-positive or non-numeric `?limit=` value is
   * ignored so a malformed URL cannot accidentally disable the window.
   * @type {number}
   */
  const WINDOW_SIZE = (() => {
    const n = parseInt(params.get('limit'), 10);
    return Number.isFinite(n) && n > 0 ? n : 200;
  })();

  /**
   * GitHub `owner/name` slug used when a user clicks a chart point to jump
   * to the corresponding commit. Auto-detected from `<owner>.github.io/<repo>`
   * URLs when not explicitly set via `?repo=`.
   * @type {string|null}
   */
  const REPO = params.get('repo') || detectRepo();

  // ── DOM element references (cached once at startup) ───────────────────
  const statusEl = document.getElementById('status');
  const controlsEl = document.getElementById('controls');
  const chartsEl = document.getElementById('charts');
  const packageSelect = document.getElementById('package-select');
  const functionList = document.getElementById('function-list');
  const windowInfo = document.getElementById('window-info');
  const sourceLine = document.getElementById('source-line');
  const historyLink = document.getElementById('history-link');

  // Populate the static chrome elements that describe *what* we're viewing.
  // HISTORY_URL / REPO both flow through `textContent` (or an `href` that is
  // already validated as an URL by `URLSearchParams` + `detectRepo()` checks)
  // so user-supplied values cannot inject markup.
  historyLink.href = HISTORY_URL;
  sourceLine.textContent = REPO
    ? `Repo: ${REPO}  ·  History: ${HISTORY_URL}`
    : `History: ${HISTORY_URL} (pass ?repo=owner/name to link commits to GitHub)`;

  /**
   * Slice of the history dataset limited to the most recent `WINDOW_SIZE`
   * entries. Used by both the control-population pass and every chart render.
   * @type {Array<{commit?: string, timestamp?: string, data?: Array}>}
   */
  let windowed = [];

  /**
   * Active Chart.js instances. Tracked so they can be `.destroy()`ed cleanly
   * before rendering a fresh selection — without this the chart canvases
   * leak listeners and produce duplicate tooltips on redraw.
   * @type {Chart[]}
   */
  let charts = [];

  /**
   * Auto-detect a GitHub `owner/name` repo slug from the page URL when the
   * dashboard is hosted on GitHub Pages (`<owner>.github.io/<repo>/...`).
   *
   * Returns `null` for custom domains or local preview servers — the caller
   * should fall back to the explicit `?repo=` parameter in that case.
   *
   * @returns {string|null} `owner/name` slug if detectable, else `null`.
   */
  function detectRepo() {
    const host = location.hostname;
    const path = location.pathname.split('/').filter(Boolean);
    if (host.endsWith('.github.io') && path.length > 0) {
      return `${host.split('.')[0]}/${path[0]}`;
    }
    return null;
  }

  /**
   * Abbreviate a full commit SHA to the short 7-character form used in
   * axis labels and tooltips. Non-string / too-short inputs fall back to
   * `?` so a malformed entry never renders a blank label.
   *
   * @param {unknown} sha  Full commit hash from a history entry.
   * @returns {string}     7-char short hash (or `?` if unavailable).
   */
  function shortSha(sha) {
    return typeof sha === 'string' && sha.length >= 7 ? sha.slice(0, 7) : (sha || '?');
  }

  /**
   * Runtime shape check for a single history entry. Called *everywhere*
   * individual fields are accessed rather than relying on one up-front pass
   * because a single bad row in an otherwise-good dataset should not blank
   * the whole page.
   *
   * @param {unknown} entry  Value read from the top-level `history` array.
   * @returns {entry is {data: unknown[]}} `true` when `entry` carries an
   *   array of measurement rows in `entry.data`.
   */
  function isValidEntry(entry) {
    return entry && typeof entry === 'object' && Array.isArray(entry.data);
  }

  // ── Status / error / empty-state rendering ────────────────────────────
  // All messages are built with `textContent`/`createTextNode`: the URL and
  // any detail string are user-supplied and must never be parsed as markup.

  /**
   * Wrap an arbitrary string into a safe `Text` node. Using this helper
   * instead of literal interpolations makes it obvious at the call-site that
   * the value is not being parsed as HTML.
   *
   * @param {string} value  Plain-text content.
   * @returns {Text}        DOM text node.
   */
  function textNode(value) {
    return document.createTextNode(value);
  }

  /**
   * Wrap a string into a `<code>` element via `textContent` — even the
   * monospace-highlighted pieces of error messages must not accept markup.
   *
   * @param {string} value  Plain-text identifier or URL.
   * @returns {HTMLElement} `<code>` element with the text content set.
   */
  function codeNode(value) {
    const code = document.createElement('code');
    code.textContent = value;
    return code;
  }

  /**
   * Replace the `#status` banner with a paragraph composed from pre-built
   * DOM nodes. Takes an array of nodes (not strings) to prevent accidental
   * HTML-injection at the call-site.
   *
   * @param {'error'|'info'} className  CSS class applied to the paragraph.
   * @param {Array<Node>}    nodes      Child nodes composing the message.
   */
  function setStatus(className, nodes) {
    statusEl.textContent = '';
    const p = document.createElement('p');
    p.className = className;
    nodes.forEach((node) => p.appendChild(node));
    statusEl.appendChild(p);
    statusEl.hidden = false;
  }

  /**
   * Render a distinguished, actionable error for a failed dataset load.
   *
   * The `kind` enum deliberately separates four *different* failure classes
   * so the visitor can tell them apart without opening DevTools:
   *
   *   - `network` — the `fetch()` itself rejected (DNS, CORS, TLS, offline).
   *   - `http`    — the server responded with a non-2xx status code.
   *   - `json`    — the body bytes could not be parsed as JSON (most often
   *                 an HTML error page served at the expected JSON path).
   *   - `shape`   — parsed to JSON successfully, but the top-level value is
   *                 not the documented `Array<{ commit, timestamp, data }>`.
   *
   * In every case the controls panel is hidden so the user cannot trigger a
   * chart render against empty/bad `windowed`.
   *
   * @param {'network'|'http'|'json'|'shape'} kind    Failure classification.
   * @param {string}                          [detail] Optional additional
   *   context string (e.g. the HTTP status code for the `http` case).
   */
  function showLoadError(kind, detail) {
    controlsEl.hidden = true;
    const nodes = [textNode('Could not load '), codeNode(HISTORY_URL)];
    if (kind === 'network') {
      nodes.push(textNode('. The request failed — the file may be missing, the server unreachable, or your browser blocked it.'));
    } else if (kind === 'http') {
      nodes.push(textNode(`. The server responded with ${detail}. The file may be missing or the deploy may be broken.`));
    } else if (kind === 'json') {
      nodes.push(textNode('. The response is not valid JSON (it may be an HTML error page). Expected a JSON array of { commit, timestamp, data: [...] } entries.'));
    } else if (kind === 'shape') {
      nodes.push(textNode('. The JSON parsed, but it is not the expected shape: an array of entries like { "commit": "...", "timestamp": "...", "data": [ ...rows ] }.'));
    }
    nodes.push(textNode(" If you're viewing a copy of this dashboard, pass ?history=URL_TO_history.json."));
    setStatus('error', nodes);
  }

  /**
   * Rendered when the loaded dataset is well-formed but contains zero
   * entries after windowing — i.e. CI has not yet appended any measurements
   * yet. An explicit empty state (instead of a blank chart area) avoids
   * the user wondering whether the fetch silently failed.
   */
  function showEmptyState() {
    chartsEl.textContent = '';
    const p = document.createElement('p');
    p.className = 'empty';
    p.textContent = 'No recorded measurements yet. history.json contains an empty dataset; it will populate as CI records budget reports.';
    chartsEl.appendChild(p);
  }

  /**
   * Build the nested index `package → function → Set<metric>` from the raw
   * history array. The pivot drives two distinct places:
   *
   *   1. **Control population** (`populateControls`): list all available
   *      packages in the `<select>`, and for the selected package, list
   *      every function with a checkbox.
   *   2. **Chart rendering** (`render`): for the selected package +
   *      functions, compute the union of metrics to decide which charts
   *      to draw.
   *
   * Invalid entries/rows are skipped inside the loop rather than filtered
   * up-front so a single malformed commit does not drop surrounding good
   * commits from the pivot index.
   *
   * @param {Array} history  Raw top-level `history.json` array.
   * @returns {Map<string, Map<string, Set<string>>>}
   *   `Map<pkg, Map<fn, Set<metric>>>` — the pivot index.
   */
  function pivot(history) {
    const byPackage = new Map(); // pkg -> Map(fn -> Set(metric))
    history.forEach((entry) => {
      if (!isValidEntry(entry)) return;
      entry.data.forEach((row) => {
        if (!row || typeof row.value !== 'number') return;
        const pkg = row.package || 'unknown';
        const fn = row.function || 'unknown';
        const metric = row.metric || 'unknown';
        if (!byPackage.has(pkg)) byPackage.set(pkg, new Map());
        const fnMap = byPackage.get(pkg);
        if (!fnMap.has(fn)) fnMap.set(fn, new Set());
        fnMap.get(fn).add(metric);
      });
    });
    return byPackage;
  }

  /**
   * Look up a single measurement value for the given (entry, package,
   * function, metric) tuple. Returns `null` when the entry is invalid,
   * the row is absent, or the value is not numeric — these gaps render as
   * breaks in the line chart (Chart.js `spanGaps: false`).
   *
   * @param {unknown} entry   Single history entry from the windowed slice.
   * @param {string}  pkg     Selected package name.
   * @param {string}  fn      Selected function name.
   * @param {string}  metric  Selected metric name.
   * @returns {number|null}   The numeric value, or `null` if not present/valid.
   */
  function valueFor(entry, pkg, fn, metric) {
    if (!isValidEntry(entry)) return null;
    const row = entry.data.find(
      (r) => r && r.package === pkg && r.function === fn && r.metric === metric
    );
    return row && typeof row.value === 'number' ? row.value : null;
  }

  /**
   * Compute the percentage change between the first and last **non-null**
   * values in a series. Used in the per-function summary row beneath each
   * chart.
   *
   * Returns `null` for degenerate inputs so the summary can omit the change
   * indicator: fewer than two non-null points, or a zero first value (which
   * would produce division by zero / an infinite percentage).
   *
   * @param {Array<number|null|undefined>} values  Series of chart values
   *   (one per history entry; `null` where a measurement is missing).
   * @returns {number|null} Percent change `((last - first) / first) * 100`,
   *   or `null` when the calculation cannot be performed meaningfully.
   */
  function pctChange(values) {
    const nonNull = values.filter((v) => v !== null && v !== undefined);
    if (nonNull.length < 2) return null;
    const first = nonNull[0];
    const last = nonNull[nonNull.length - 1];
    if (first === 0) return null;
    return ((last - first) / first) * 100;
  }

  /**
   * Fixed 8-color palette used for per-function line series. We cycle
   * through it via `colorFor()` rather than letting Chart.js auto-assign
   * so the swatch in the summary list always matches the line color, and
   * so the same function index always gets the same color across redraws.
   * @type {string[]}
   */
  const PALETTE = ['#2563eb', '#dc2626', '#059669', '#d97706', '#7c3aed', '#0891b2', '#db2777', '#65a30d'];

  /**
   * Map a function-index in the selected list to a palette color, cycling
   * the palette if the user selects more than 8 functions.
   *
   * @param {number} i  0-based index into the selected-functions array.
   * @returns {string}  CSS hex color from `PALETTE`.
   */
  const colorFor = (i) => PALETTE[i % PALETTE.length];

  /**
   * Custom `Array.sort` comparator for metric names. The three standard
   * Soroban budget metrics (`CPU Instructions`, `Read Bytes`, `Write Bytes`)
   * are ordered consistently first; any other (custom) metrics that appear
   * in the dataset fall back to alphabetical order after the known set.
   *
   * This keeps the most-important charts at the top of the page even when
   * users inject their own additional metrics into `history.json`.
   *
   * @param {string} a  Left metric name.
   * @param {string} b  Right metric name.
   * @returns {number}  Negative/zero/positive, per `Array.sort` semantics.
   */
  function metricOrder(a, b) {
    const known = ['CPU Instructions', 'Read Bytes', 'Write Bytes'];
    const ai = known.indexOf(a);
    const bi = known.indexOf(b);
    if (ai !== -1 && bi !== -1) return ai - bi;
    if (ai !== -1) return -1;
    if (bi !== -1) return 1;
    return a.localeCompare(b);
  }

  /**
   * Re-render the full charts area for the currently selected package and
   * set of checked function checkboxes. Called on first load and whenever
   * the user changes the package dropdown, checks/unchecks a function, or
   * the dataset is reloaded.
   *
   * ## Implementation notes
   * - Existing `Chart` instances are explicitly `.destroy()`ed before the
   *   container is cleared — Chart.js otherwise leaks resize listeners and
   *   retains stale tooltip DOM on the document.
   * - One `<canvas>` (and thus one `Chart` instance) is created per metric
   *   present in the union of selected functions, because each metric has a
   *   very different scale (bytes vs. millions of instructions) and would
   *   be unreadable on a shared y-axis.
   * - `spanGaps: false` means missing measurements render as a visible break
   *   in the line, making it obvious when a particular function/metric was
   *   not recorded for a given commit instead of silently interpolating.
   * - The `onClick` handler opens a commit on GitHub only when `REPO` is
   *   known (auto-detected or `?repo=`), so custom-domain deployments
   *   without a configured repo silently no-op on click rather than 404.
   *
   * @param {string}   selectedPkg  Currently selected package name.
   * @param {string[]} selectedFns  Names of the checked functions for the
   *   currently selected package.
   */
  function render(selectedPkg, selectedFns) {
    chartsEl.innerHTML = '';
    charts.forEach((c) => c.destroy());
    charts = [];

    if (!selectedFns.length) {
      chartsEl.innerHTML = '<p class="empty">Select at least one function to plot.</p>';
      return;
    }

    const byPackage = pivot(windowed);
    const metrics = new Set();
    selectedFns.forEach((fn) => {
      const fnMap = byPackage.get(selectedPkg);
      const metricSet = fnMap && fnMap.get(fn);
      if (metricSet) metricSet.forEach((m) => metrics.add(m));
    });
    const sortedMetrics = Array.from(metrics).sort(metricOrder);
    // X-axis labels: one short SHA per history entry (or `—` placeholder
    // for entries that fail the shape check so the x-axis still lines up).
    const labels = windowed.map((e) => (isValidEntry(e) ? shortSha(e.commit) : '—'));

    sortedMetrics.forEach((metric) => {
      const wrapper = document.createElement('div');
      wrapper.className = 'chart-card';
      const heading = document.createElement('h3');
      // `metric` comes from the fetched document — text, never markup.
      heading.textContent = metric;
      wrapper.appendChild(heading);
      const summary = document.createElement('ul');
      summary.className = 'summary';
      wrapper.appendChild(summary);
      const canvasHolder = document.createElement('div');
      canvasHolder.className = 'canvas-holder';
      const canvas = document.createElement('canvas');
      canvasHolder.appendChild(canvas);
      wrapper.appendChild(canvasHolder);
      chartsEl.appendChild(wrapper);

      // Build one dataset per selected function: values are produced by
      // probing each history entry for the (pkg, fn, metric) tuple, and
      // the per-series summary row is rendered inline alongside so the
      // swatch/name/%-change stay vertically aligned with the legend-less
      // chart (Chart.js's built-in legend was moved here to make room for
      // the summary percentages).
      const datasets = selectedFns.map((fn, i) => {
        const values = windowed.map((e) => valueFor(e, selectedPkg, fn, metric));
        const change = pctChange(values);
        const li = document.createElement('li');
        const swatch = document.createElement('span');
        swatch.className = 'swatch';
        swatch.style.background = colorFor(i);
        li.appendChild(swatch);
        // `fn` comes from the fetched document — text, never markup.
        li.appendChild(textNode(fn));
        if (change !== null) {
          li.appendChild(textNode(' — '));
          const strong = document.createElement('strong');
          strong.className = change > 0 ? 'up' : change < 0 ? 'down' : '';
          strong.textContent = `${change > 0 ? '+' : ''}${change.toFixed(1)}%`;
          li.appendChild(strong);
          li.appendChild(textNode(' over shown range'));
        }
        summary.appendChild(li);
        return {
          label: fn,
          data: values,
          spanGaps: false,
          borderColor: colorFor(i),
          backgroundColor: colorFor(i),
          tension: 0.15,
          pointRadius: 3,
          pointHoverRadius: 5,
        };
      });

      const chart = new Chart(canvas, {
        type: 'line',
        data: { labels, datasets },
        options: {
          responsive: true,
          // `nearest` + `intersect: false` lets the user hover near the
          // line rather than exactly on a data point — much easier to use
          // when commits are densely packed on the x-axis.
          interaction: { mode: 'nearest', intersect: false },
          plugins: {
            legend: { display: false },
            tooltip: {
              callbacks: {
                title: (items) => {
                  const entry = windowed[items[0].dataIndex];
                  if (!isValidEntry(entry)) return 'no data';
                  return `${shortSha(entry.commit)}  ·  ${entry.timestamp || ''}`;
                },
              },
            },
          },
          scales: {
            x: { ticks: { maxRotation: 60, minRotation: 60 } },
            y: { title: { display: true, text: metric } },
          },
          // Clicking a data point jumps to the commit on GitHub — the
          // `noopener` window feature keeps the opened tab from gaining a
          // cross-origin handle back to the dashboard page.
          onClick: (evt, elements) => {
            if (!elements.length || !REPO) return;
            const entry = windowed[elements[0].index];
            if (isValidEntry(entry) && entry.commit) {
              window.open(`https://github.com/${REPO}/commit/${entry.commit}`, '_blank', 'noopener');
            }
          },
        },
      });
      charts.push(chart);
    });
  }

  /**
   * Build and wire up the two interactive controls (package `<select>` and
   * function checkbox list) from the already-loaded `windowed` dataset.
   *
   * Flow:
   *   1. Pivot the history into the `package → function → metric` index.
   *   2. Fill the package `<select>` with sorted unique package names.
   *   3. On package change (or initial render), rebuild the checkbox list
   *      for the selected package and auto-check the first four entries
   *      so the dashboard is never blank on first load.
   *   4. Any change to the select or a checkbox triggers `render()` with
   *      the currently selected values.
   */
  function populateControls() {
    const byPackage = pivot(windowed);
    const packages = Array.from(byPackage.keys()).sort();

    packageSelect.innerHTML = '';
    packages.forEach((pkg) => {
      const opt = document.createElement('option');
      opt.value = pkg;
      opt.textContent = pkg;
      packageSelect.appendChild(opt);
    });

    /**
     * Rebuild the per-package function checkbox list. Called on initial
     * load and every time the selected package changes.
     *
     * @param {string} pkg Currently selected package name.
     */
    function renderFunctionList(pkg) {
      functionList.innerHTML = '';
      const fnMap = byPackage.get(pkg) || new Map();
      Array.from(fnMap.keys()).sort().forEach((fn, i) => {
        // Sanitize the id for use in HTML: only keep `[A-Za-z0-9_-]`,
        // replace any other character with `_` so function names like
        // `my_fn::inner` or names with spaces still produce valid ids.
        const id = `fn-${pkg}-${fn}`.replace(/[^a-zA-Z0-9_-]/g, '_');
        const label = document.createElement('label');
        label.className = 'checkbox';
        const input = document.createElement('input');
        input.type = 'checkbox';
        input.id = id;
        input.value = fn;
        if (i < 4) input.checked = true;
        label.appendChild(input);
        // `fn` comes from the fetched document — text, never markup.
        label.appendChild(textNode(` ${fn}`));
        functionList.appendChild(label);
      });
      functionList.querySelectorAll('input').forEach((input) => input.addEventListener('change', triggerRender));
    }

    /**
     * Read the current control state and re-render charts. Extracted as a
     * named helper so it can be shared between the package `change`
     * listener, the function-checkbox `change` listeners, and the
     * initial-population path.
     */
    function triggerRender() {
      const pkg = packageSelect.value;
      const selectedFns = Array.from(functionList.querySelectorAll('input:checked')).map((i) => i.value);
      render(pkg, selectedFns);
    }

    packageSelect.addEventListener('change', () => {
      renderFunctionList(packageSelect.value);
      triggerRender();
    });

    if (packages.length) {
      renderFunctionList(packages[0]);
      triggerRender();
    }
  }

  /**
   * Module entry point — an IIFE that fetches, validates, and wires up the
   * history dataset when the script loads. Failure paths route into the
   * classified `showLoadError` cases above; the success path populates
   * controls and hands off to user-driven interactions.
   *
   * Runs inline at the bottom of the script so DOM references captured
   * earlier in the file already exist (`dashboard.html` loads this script
   * at the end of `<body>`).
   */
  (async () => {
    let res;
    try {
      res = await fetch(HISTORY_URL);
    } catch (err) {
      showLoadError('network');
      return;
    }

    if (!res.ok) {
      showLoadError('http', `${res.status}${res.statusText ? ' ' + res.statusText : ''}`);
      return;
    }

    let history;
    try {
      history = await res.json();
    } catch (err) {
      showLoadError('json');
      return;
    }

    if (!Array.isArray(history)) {
      showLoadError('shape');
      return;
    }

    // Keep only the most recent N commits so very long histories don't
    // produce an unusably dense x-axis. Use `slice(-N)` because CI appends
    // new entries to the *end* of the `history.json` array.
    windowed = history.slice(-WINDOW_SIZE);
    windowInfo.textContent = `${windowed.length} of ${history.length} recorded commits` +
      (history.length > windowed.length ? ' (pass ?limit=N to change)' : '');
    statusEl.hidden = true;
    controlsEl.hidden = false;

    if (windowed.length === 0) {
      showEmptyState();
      return;
    }

    populateControls();
  })();
})();
