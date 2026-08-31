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

  const HISTORY_URL = params.get('history') || './history.json';
  const WINDOW_SIZE = (() => {
    const n = parseInt(params.get('limit'), 10);
    return Number.isFinite(n) && n > 0 ? n : 200;
  })();
  const REPO = params.get('repo') || detectRepo();

  const statusEl = document.getElementById('status');
  const controlsEl = document.getElementById('controls');
  const chartsEl = document.getElementById('charts');
  const packageSelect = document.getElementById('package-select');
  const functionList = document.getElementById('function-list');
  const windowInfo = document.getElementById('window-info');
  const sourceLine = document.getElementById('source-line');
  const historyLink = document.getElementById('history-link');

  historyLink.href = HISTORY_URL;
  sourceLine.textContent = REPO
    ? `Repo: ${REPO}  ·  History: ${HISTORY_URL}`
    : `History: ${HISTORY_URL} (pass ?repo=owner/name to link commits to GitHub)`;

  let windowed = [];
  let charts = [];

  function detectRepo() {
    const host = location.hostname;
    const path = location.pathname.split('/').filter(Boolean);
    if (host.endsWith('.github.io') && path.length > 0) {
      return `${host.split('.')[0]}/${path[0]}`;
    }
    return null;
  }

  function shortSha(sha) {
    return typeof sha === 'string' && sha.length >= 7 ? sha.slice(0, 7) : (sha || '?');
  }

  function isValidEntry(entry) {
    return entry && typeof entry === 'object' && Array.isArray(entry.data);
  }

  // ── Status / error / empty-state rendering ────────────────────────────
  // All messages are built with `textContent`/`createTextNode`: the URL and
  // any detail string are user-supplied and must never be parsed as markup.

  function textNode(value) {
    return document.createTextNode(value);
  }

  function codeNode(value) {
    const code = document.createElement('code');
    code.textContent = value;
    return code;
  }

  // Replace the `#status` section with a paragraph built from text nodes.
  function setStatus(className, nodes) {
    statusEl.textContent = '';
    const p = document.createElement('p');
    p.className = className;
    nodes.forEach((node) => p.appendChild(node));
    statusEl.appendChild(p);
    statusEl.hidden = false;
  }

  // A failed load. `kind` distinguishes the failure classes so the visitor
  // can tell "no data here" from "the deploy is broken" apart:
  //   network — the request itself failed (missing file, blocked request)
  //   http    — the server answered with a non-2xx status
  //   json    — the body was not valid JSON (e.g. an HTML error page)
  //   shape   — valid JSON, but not the expected array of history entries
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

  // Valid data, but no measurements: an explicit empty state, not a blank page.
  function showEmptyState() {
    chartsEl.textContent = '';
    const p = document.createElement('p');
    p.className = 'empty';
    p.textContent = 'No recorded measurements yet. history.json contains an empty dataset; it will populate as CI records budget reports.';
    chartsEl.appendChild(p);
  }

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

  function valueFor(entry, pkg, fn, metric) {
    if (!isValidEntry(entry)) return null;
    const row = entry.data.find(
      (r) => r && r.package === pkg && r.function === fn && r.metric === metric
    );
    return row && typeof row.value === 'number' ? row.value : null;
  }

  function pctChange(values) {
    const nonNull = values.filter((v) => v !== null && v !== undefined);
    if (nonNull.length < 2) return null;
    const first = nonNull[0];
    const last = nonNull[nonNull.length - 1];
    if (first === 0) return null;
    return ((last - first) / first) * 100;
  }

  const PALETTE = ['#2563eb', '#dc2626', '#059669', '#d97706', '#7c3aed', '#0891b2', '#db2777', '#65a30d'];
  const colorFor = (i) => PALETTE[i % PALETTE.length];

  function metricOrder(a, b) {
    const known = ['CPU Instructions', 'Read Bytes', 'Write Bytes'];
    const ai = known.indexOf(a);
    const bi = known.indexOf(b);
    if (ai !== -1 && bi !== -1) return ai - bi;
    if (ai !== -1) return -1;
    if (bi !== -1) return 1;
    return a.localeCompare(b);
  }

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

    function renderFunctionList(pkg) {
      functionList.innerHTML = '';
      const fnMap = byPackage.get(pkg) || new Map();
      Array.from(fnMap.keys()).sort().forEach((fn, i) => {
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
