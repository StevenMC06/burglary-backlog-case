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
const app = vm.runInContext('({ S, renderQueue, selectCase, stepItem, showEvidence, activityItem, readableReport, pendingSteps, renderPacket, renderVictim, renderCase })', context);
function find(node, text) {
  if (node.tag === 'button' && node.textContent === text) return node;
  for (const child of node.children || []) { const result = find(child, text); if (result) return result; }
}

module.exports = (async () => {
  app.S.queue = { cases: [{ id: 'CC-26-000001', neighborhood: 'FREMONT', block: '42XX BLOCK', flags: {}, solvability: { score: 78, band: 'strong' } }] };
  document.getElementById('queue-search').value = 'fReMoNt';
  app.renderQueue();
  assert.equal(elements.get('queue-list').children.length, 1);
  assert.ok(elements.get('queue-list').textContent.includes('42XX Block'), 'Inbox must show location without opening a case');
  const meter = elements.get('queue-list').children[0].children[0].children.find(n => n.className === 'queue-meter strong');
  assert.equal(meter.children[0].attrs.style, 'width:78%', 'Meter must use the engine score');
  assert.equal(meter.attrs['aria-label'], 'Solvability 78 out of 100');
  document.getElementById('queue-search').value = 'no such case';
  app.renderQueue();
  assert.equal(elements.get('queue-list').children.length, 0);
  assert.equal(elements.get('queue-empty').hidden, false);

  app.S.queue.cases.push({ id: 'pawn', neighborhood: 'FREMONT', block: '1XX', flags: { pawn_hit: 'exact_serial_lead' } }, { id: 'linked', neighborhood: 'BALLARD', block: '2XX', flags: { series_id: 'S-1' } });
  document.getElementById('queue-search').value = '';
  elements.get('queue-filter-pawn').onclick();
  assert.equal(elements.get('queue-list').children.length, 1);
  assert.ok(elements.get('queue-list').textContent.includes('pawn'));
  document.getElementById('queue-search').value = 'ballard';
  app.renderQueue();
  assert.equal(elements.get('queue-list').children.length, 0, 'Search and lead filter must combine');
  elements.get('queue-filter-related').onclick();
  assert.equal(elements.get('queue-list').children.length, 1);
  assert.ok(elements.get('queue-list').textContent.includes('linked'));
  elements.get('queue-filter-all').onclick();
  document.getElementById('queue-search').value = '';

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

  elements.get('tab-packet').onkeydown({ key: 'End', preventDefault() {} });
  assert.equal(elements.get('tab-victim').attrs['aria-selected'], 'true', 'End must select the final review tab');
  elements.get('tab-victim').onkeydown({ key: 'Home', preventDefault() {} });
  assert.equal(elements.get('tab-packet').attrs['aria-selected'], 'true', 'Home must select the first review tab');

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
  assert.ok(elements.get('case-heading').textContent.includes('Not linked'));
  assert.ok(elements.get('case-heading').textContent.includes('Not recorded'), 'Unknown offense window must not show zero hours');
  const overview = () => elements.get('case-card').children.find(n => n.attrs.id === 'evidence-report');
  assert.ok(overview().textContent.includes('Source record'));
  assert.ok(overview().textContent.includes('Property'), 'Overview must show report and property together');

  app.S.current.case.window_start = 0;
  app.S.current.case.window_end = 1800;
  app.S.current.pawn = [{ match: { status: 'exact_serial_lead', pawn_id: 'PW-1', days_after_offense: 3, reason: 'Serial match' }, item: { description: 'Synthetic laptop', serial: 'ABC' }, pawn: { serial: 'ABC', dealer: 'Demo dealer' } }];
  app.renderCase();
  assert.ok(elements.get('case-heading').textContent.includes('0.5 h'), 'Short windows must not round to zero');
  assert.ok(elements.get('case-heading').textContent.includes('PW-1'));
  assert.ok(overview().textContent.includes('Exact serial lead'));

  app.S.current.recorded.victim_liaison = { output: { language: 'es', message: 'Estimado residente' } };
  app.S.current.victim_template = { language: 'en', message: 'Your report has been received.' };
  app.renderVictim();
  assert.ok(elements.get('pane-victim').textContent.includes('Your report has been received.'));
  assert.ok(!elements.get('pane-victim').textContent.includes('Estimado'));

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
  await find(failed, 'Not available').onclick();
  await find(failed, 'Cancel').onclick();
  const rejectionForm = failed.children[1].children.find(n => n.tag === 'fieldset');
  assert.equal(rejectionForm.hidden, true, 'Cancel must hide the form without saving');
  assert.equal(app.S.decided['second#0'], undefined);
  await find(failed, 'Reject').onclick();
  assert.equal(rejectionForm.hidden, false);
  await find(failed, 'Confirm rejection').onclick();
  assert.equal(app.S.decided['second#0'], undefined, 'A failed save must not show Rejected');

  context.fetch = async path => ({ ok: true, json: async () => path === '/api/audit' ? { chain: { intact: true, entries: 1 }, entries: [] } : { entry: { seq: 1 } } });
  await find(failed, 'Accept').onclick();
  assert.equal(app.S.decided['second#0'].kind, 'accepted');
  const progression = { case: { id: 'second' }, next_steps: [step, { ...step, step: 'Second action' }, { ...step, step: 'Third action' }] };
  assert.equal(app.pendingSteps(progression)[0].index, 1, 'Accepted action must advance to next pending item');
  app.S.current = { ...app.S.current, ...progression };
  app.renderPacket();
  const featured = () => elements.get('pane-packet').children.find(n => n.className === 'steps featured-step');
  assert.ok(featured().textContent.includes('Second action'), 'Next action must render without opening a disclosure');
  app.S.decided['second#1'] = { kind: 'rejected', reason: 'Other' };
  assert.equal(app.pendingSteps(progression)[0].index, 2, 'Rejected action must advance too');
  app.renderPacket();
  assert.ok(featured().textContent.includes('Third action'));
  app.S.decided['second#2'] = { kind: 'accepted' };
  assert.equal(app.pendingSteps(progression).length, 0, 'All reviewed must leave no pending action');
  app.renderPacket();
  assert.equal(featured(), undefined);
  assert.ok(elements.get('pane-packet').textContent.includes('All actions reviewed'));

  const media = { matches: true, addEventListener(event, fn) { this.onchange = fn; } };
  const desktopContext = vm.createContext({ document, setTimeout() {}, clearTimeout() {}, matchMedia: () => media });
  vm.runInContext(script.replace(/\nmain\(\);\s*$/, ''), desktopContext);
  desktopContext.fetch = async () => ({ ok: true, json: async () => data('desktop') });
  await vm.runInContext("selectCase('desktop')", desktopContext);
  assert.equal(elements.get('case-switcher').open, true, 'Desktop selection must keep the inbox visible');
  media.onchange({ matches: false });
  assert.equal(elements.get('case-switcher').open, false, 'A narrow viewport must collapse the inbox');
  media.onchange({ matches: true });
  assert.equal(elements.get('case-switcher').open, true, 'Returning to desktop must restore the inbox');
  console.log('UI checks passed: search, empty results, selection race, failed decisions, retry.');
})();
