import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';
import vm from 'node:vm';

const dashboardSource = await readFile(new URL('../assets/dashboard.js', import.meta.url), 'utf8');

class Element {
  constructor(tagName) {
    this.tagName = tagName;
    this.children = [];
    this.listeners = {};
    this.hidden = false;
    this.value = '';
    this.checked = false;
    this._textContent = '';
  }

  set textContent(value) {
    this._textContent = String(value);
    this.children = [];
  }

  get textContent() {
    return this._textContent + this.children.map((child) => child.textContent || '').join('');
  }

  set innerHTML(_value) {
    this._textContent = '';
    this.children = [];
  }

  appendChild(child) {
    this.children.push(child);
    if (this.tagName === 'select' && child.tagName === 'option' && this.children.length === 1) {
      this.value = child.value;
    }
    return child;
  }

  addEventListener(type, listener) {
    this.listeners[type] = listener;
  }

  querySelectorAll(selector) {
    const matches = [];
    const visit = (element) => {
      for (const child of element.children) {
        if (
          child.tagName === 'input' &&
          (selector === 'input' || (selector === 'input:checked' && child.checked))
        ) {
          matches.push(child);
        }
        if (child.children) visit(child);
      }
    };
    visit(this);
    return matches;
  }
}

async function loadDashboard({ search = '', fetchResponse }) {
  const elements = new Map(
    [
      ['status', 'section'],
      ['controls', 'section'],
      ['charts', 'section'],
      ['package-select', 'select'],
      ['function-list', 'div'],
      ['window-info', 'span'],
      ['source-line', 'p'],
      ['history-link', 'a'],
    ].map(([id, tagName]) => [id, new Element(tagName)]),
  );
  const chartConfigs = [];
  const openedUrls = [];
  const requests = [];
  const document = {
    getElementById: (id) => elements.get(id),
    createElement: (tagName) => new Element(tagName),
    createTextNode: (text) => ({ textContent: String(text) }),
  };
  class Chart {
    constructor(_canvas, config) {
      chartConfigs.push(config);
    }

    destroy() {}
  }

  vm.runInNewContext(dashboardSource, {
    Chart,
    document,
    fetch: async (url) => {
      requests.push(url);
      return fetchResponse;
    },
    location: {
      search,
      hostname: 'example.test',
      pathname: '/',
    },
    URLSearchParams,
    window: {
      open: (...args) => openedUrls.push(args),
    },
  });
  await new Promise((resolve) => setImmediate(resolve));

  return { elements, chartConfigs, openedUrls, requests };
}

test('reads history, limit, and repository query parameters', async () => {
  const historyUrl = 'https://example.test/metrics/history.json';
  const history = [
    { commit: 'aaaaaaaa1111111', data: [] },
    {
      commit: 'bbbbbbbb2222222',
      data: [{ package: 'pkg', function: 'run', metric: 'CPU Instructions', value: 20 }],
    },
  ];
  const dashboard = await loadDashboard({
    search: `?history=${encodeURIComponent(historyUrl)}&limit=1&repo=owner/project`,
    fetchResponse: { ok: true, json: async () => history },
  });

  assert.deepEqual(dashboard.requests, [historyUrl]);
  assert.equal(dashboard.elements.get('history-link').href, historyUrl);
  assert.match(dashboard.elements.get('source-line').textContent, /Repo: owner\/project/);
  assert.match(dashboard.elements.get('window-info').textContent, /^1 of 2 recorded commits/);
  assert.equal(dashboard.chartConfigs[0].data.labels.join(','), 'bbbbbbb');

  dashboard.chartConfigs[0].options.onClick({}, [{ index: 0 }]);
  assert.equal(
    dashboard.openedUrls[0][0],
    'https://github.com/owner/project/commit/bbbbbbbb2222222',
  );
});

test('pivots CPU, memory, and storage metrics across history entries', async () => {
  const metrics = ['CPU Instructions', 'Read Bytes', 'Write Bytes'];
  const history = [
    {
      commit: 'aaaaaaaa1111111',
      data: metrics.map((metric, index) => ({
        package: 'pkg',
        function: 'run',
        metric,
        value: index + 1,
      })),
    },
    {
      commit: 'bbbbbbbb2222222',
      data: metrics.map((metric, index) => ({
        package: 'pkg',
        function: 'run',
        metric,
        value: (index + 1) * 10,
      })),
    },
  ];
  const dashboard = await loadDashboard({
    fetchResponse: { ok: true, json: async () => history },
  });

  assert.equal(dashboard.chartConfigs.length, 3);
  assert.deepEqual(
    dashboard.chartConfigs.map((config) => config.options.scales.y.title.text),
    metrics,
  );
  assert.deepEqual(
    dashboard.chartConfigs.map((config) => config.data.datasets[0].data.join(',')),
    ['1,10', '2,20', '3,30'],
  );
});

test('shows a visible error when the history response is invalid JSON', async () => {
  const dashboard = await loadDashboard({
    fetchResponse: {
      ok: true,
      json: async () => {
        throw new SyntaxError('invalid JSON');
      },
    },
  });

  const status = dashboard.elements.get('status');
  assert.equal(status.hidden, false);
  assert.equal(status.children[0].className, 'error');
  assert.match(status.textContent, /not valid JSON/);
  assert.equal(dashboard.elements.get('controls').hidden, true);
});
