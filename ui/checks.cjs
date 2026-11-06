// Run with node ui/checks.cjs. No browser, dependencies, or audit writes.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');

class Element {
  constructor(tag) { this.tag = tag; this.nodeType = 1; this.children = []; this.attrs = {}; this.value = ''; }
  append(...nodes) { this.children.push(...nodes); }
  replaceChildren(...nodes) { this.children = nodes; }
  setAttribute(key, value) { this.attrs[key] = value; }
  addEventListener(key, fn) { this['on' + key] = fn; }
  focus() {}
  get textContent() { return this.children.map(n => n.textContent).join(''); }
  set textContent(value) { this.children = [{ nodeType: 3, textContent: String(value) }]; }
}
const elements = new Map();
const document = {
  getElementById(id) { if (!elements.has(id)) elements.set(id, new Element('div')); return elements.get(id); },
  createElement: tag => new Element(tag),
  createTextNode: text => ({ nodeType: 3, textContent: String(text) }),
  querySelectorAll: () => [],
  addEventListener() {},
};
const context = vm.createContext({ document, setTimeout() {}, clearTimeout() {} });
const script = fs.readFileSync(__dirname + '/app.js', 'utf8');
assert.match(script, /\nmain\(\);\s*$/);
vm.runInContext(script.replace(/\nmain\(\);\s*$/, ''), context);
const app = vm.runInContext('({ S, renderQueue, selectCase, stepItem, showEvidence, activityItem, readableReport })', context);
function find(node, text) {
  if (node.tag === 'button' && node.textContent === text) return node;
  for (const child of node.children || []) { const result = find(child, text); if (result) return result; }
}

module.exports = (async () => {
  app.S.queue = { cases: [{ id: 'CC-26-000001', neighborhood: 'FREMONT', block: '42XX BLOCK', flags: {} }] };
  document.getElementById('queue-search').value = 'fReMoNt';
  app.renderQueue();
  assert.equal(elements.get('queue-list').children.length, 1);
  document.getElementById('queue-search').value = 'no such case';
  app.renderQueue();
  assert.equal(elements.get('queue-list').children.length, 0);
  assert.equal(elements.get('queue-empty').hidden, false);

  app.showEvidence('property');
  assert.equal(elements.get('evidence-property').hidden, false);
  assert.equal(elements.get('evidence-report').hidden, true);
  assert.equal(elements.get('evidence-tab-property').attrs['aria-selected'], 'true');
  assert.equal(elements.get('evidence-tab-report').tabIndex, -1);
  elements.get('evidence-tab-property').onkeydown({ key: 'ArrowRight', preventDefault() {} });
  assert.equal(elements.get('evidence-related').hidden, false, 'Arrow keys must switch evidence tabs');

  assert.equal(app.readableReport('RP saw UNK SUSP. POE FD.'), 'Reporting person saw Unknown suspect. Entry point front door.');
  const event = app.activityItem({ action: 'accept_step', subject: 'CC-26-000001', actor: 'det.demo', seq: 0, ts: 0, hash: 'abc', detail: { step: 'Review serial', recommendation: { step: 'Review serial', why: 'Evidence' } } });
  assert.ok(event.textContent.includes('Accepted next step'));
  assert.ok(event.textContent.includes('Review serial'));
  assert.ok(!event.textContent.includes('recommendation'), 'Activity must not expose raw JSON field names');

  const pending = new Map();
  context.fetch = path => new Promise(resolve => pending.set(path, resolve));
  const data = id => ({ case: { id, status: { kind: 'open' }, block: '42XX BLOCK', neighborhood: 'FREMONT', beat: 'B2', narrative: 'Report' }, solvability: { score: 50, contributions: [] }, recorded: {}, pawn: [], similar: [], next_steps: [], victim_template: { message: 'Draft' } });
  const first = app.selectCase('first');
  const second = app.selectCase('second');
  pending.get('/api/case/second')({ ok: true, json: async () => data('second') });
  await second;
  pending.get('/api/case/first')({ ok: true, json: async () => data('first') });
  await first;
  assert.equal(app.S.current.case.id, 'second', 'A late response must not replace the selected case');
  assert.equal(elements.get('case-switcher').open, false, 'Selecting a case must close the switcher');
  assert.equal(elements.get('selected-case-label').textContent, 'second');

  const step = { step: 'Review record', why: 'Serial lead', lawful_basis_note: 'Internal review', est_minutes: 10, source: 'triage' };
  const disclosure = app.stepItem(step, 0, app.S.current);
  const details = disclosure.children[0].children.find(n => n.tag === 'details');
  assert.ok(details.textContent.includes(step.step), 'Full action wording must remain available inside Details');

  context.fetch = async () => ({ ok: false, status: 503, json: async () => ({ error: 'Unavailable' }) });
  const failed = app.stepItem(step, 0, app.S.current);
  await find(failed, 'Accept').onclick();
  assert.equal(app.S.decided['second#0'], undefined, 'A failed save must not show Accepted');
  assert.equal(find(failed, 'Accept').disabled, false, 'A failed save must allow retry');
  await find(failed, 'Reject').onclick();
  await find(failed, 'Confirm reject').onclick();
  assert.equal(app.S.decided['second#0'], undefined, 'A failed save must not show Rejected');

  context.fetch = async path => ({ ok: true, json: async () => path === '/api/audit' ? { chain: { intact: true, entries: 1 }, entries: [] } : { entry: { seq: 1 } } });
  await find(failed, 'Accept').onclick();
  assert.equal(app.S.decided['second#0'].kind, 'accepted');
  console.log('UI checks passed: search, empty results, selection race, failed decisions, retry.');
})();
