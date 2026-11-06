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
  createElementNS: (namespace, tag) => new Element(tag),
  createTextNode: text => ({ nodeType: 3, textContent: String(text) }),
  querySelectorAll: () => [],
  addEventListener() {},
};
const context = vm.createContext({ document, setTimeout() {}, clearTimeout() {} });
const script = fs.readFileSync(__dirname + '/app.js', 'utf8');
assert.match(script, /\nmain\(\);\s*$/);
vm.runInContext(script.replace(/\nmain\(\);\s*$/, ''), context);
const app = vm.runInContext('({ S, renderQueue, selectCase, stepItem, showEvidence, activityItem, readableReport, pendingSteps, renderPacket, renderVictim, renderCase, renderSeries, pickSeries })', context);
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
  assert.ok(elements.get('case-heading').textContent.includes('Solvability'), 'The requested summary strip must be visible');
  assert.ok(elements.get('case-heading').textContent.includes('Not recorded'), 'Missing windows must not turn into zero hours');
  const overview = () => elements.get('case-card').children.find(n => n.attrs.id === 'evidence-report');
  assert.ok(!overview().textContent.includes('Source record'));
  assert.ok(!overview().textContent.includes('Rules engine'));
  assert.ok(overview().textContent.includes('Property'), 'Overview must show report and property together');
  const inlineReport = overview().children[0].children.find(n => n.className === 'narr report-readable');
  assert.equal(inlineReport.tag, 'p', 'Plain-English wording must be directly visible, without a disclosure');
  assert.ok(!overview().textContent.includes('Plain-English wording'), 'Do not add a classifier above the inline wording');
  const factors = elements.get('review-factors');
  assert.equal(factors.children[0].tag, 'div');
  assert.equal(factors.children[1].className, 'bars', 'Factors must be visible in the right column, outside a dropdown');
  elements.get('tab-victim').onclick();
  assert.equal(elements.get('review-factors'), factors, 'Changing action tabs must keep the factor panel visible');
  assert.notEqual(factors.hidden, true);

  app.S.current.case.window_start = 0;
  app.S.current.case.window_end = 1800;
  app.S.current.pawn = [{ match: { status: 'exact_serial_lead', pawn_id: 'PW-1', days_after_offense: 3, reason: 'Serial match' }, item: { description: 'Synthetic laptop', serial: 'ABC' }, pawn: { serial: 'ABC', dealer: 'Demo dealer' } }];
  app.renderCase();
  assert.ok(elements.get('case-heading').textContent.includes('PW-1'), 'Summary must show the matching property record');
  assert.ok(elements.get('case-heading').textContent.includes('0.5 h'), 'Short windows must not round to zero');
  assert.ok(overview().textContent.includes('PW-1'), 'Property evidence must remain available');
  assert.ok(overview().textContent.includes('Exact serial lead'));

  app.S.current.recorded.victim_liaison = { output: { language: 'es', message: 'Estimado residente' } };
  app.S.current.victim_template = { language: 'en', message: 'Your report has been received.' };
  app.renderVictim();
  assert.ok(elements.get('pane-victim').textContent.includes('Your report has been received.'));
  assert.ok(!elements.get('pane-victim').textContent.includes('Estimado'));

  const step = { step: 'Review record', why: 'Serial lead', lawful_basis_note: 'Internal review', est_minutes: 10, source: 'triage' };
  const action = app.stepItem(step, 0, app.S.current);
  assert.equal(action.children[0].tag, 'div');
  assert.equal(action.children[0].textContent, step.step);
  assert.ok(!action.textContent.includes(step.why) && !action.textContent.includes(step.lawful_basis_note), 'Action explanations must be removed');

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
  const actions = () => elements.get('pane-packet').children.find(n => n.className === 'steps');
  assert.equal(actions().children.length, 2, 'Every pending action must be visible');
  assert.ok(actions().textContent.includes('Second action') && actions().textContent.includes('Third action'));
  assert.ok(!elements.get('pane-packet').textContent.includes('remaining') && !elements.get('pane-packet').textContent.includes('No recorded triage'));
  assert.ok(!elements.get('pane-packet').textContent.includes('Lead packet') && !elements.get('pane-packet').textContent.includes('Triage rationale'));
  app.S.decided['second#1'] = { kind: 'rejected', reason: 'Other' };
  assert.equal(app.pendingSteps(progression)[0].index, 2, 'Rejected action must advance too');
  app.renderPacket();
  assert.equal(actions().children.length, 1);
  assert.ok(actions().textContent.includes('Third action'));
  app.S.decided['second#2'] = { kind: 'accepted' };
  assert.equal(app.pendingSteps(progression).length, 0, 'All reviewed must leave no pending action');
  app.renderPacket();
  assert.equal(actions(), undefined);
  assert.ok(elements.get('pane-packet').textContent.includes('All actions reviewed'));

  const linkedCase = (id, start) => ({ id, block: '42XX BLOCK OF FREMONT AVE N', neighborhood: 'FREMONT', location: { lat: 47.6, lon: -122.3 }, window_start: start, window_end: start + 1800, status: { kind: 'open' } });
  const group = (id, verdict, ids) => ({ id, case_ids: ids, cases: ids.map((name, i) => linkedCase(name, i * 3600)), in_queue: [ids[0]], verify: { verdict, offending_cases: verdict === 'fail' ? ids : [], checks: [{ name: 'spatiotemporal_feasibility', verdict, reason: 'Travel windows conflict.' }] } });
  app.S.series = { series: [group('S-1', 'pass', ['second', 'other']), group('S-2', 'fail', ['far']), group('S-3', 'weak', ['uncertain'])] };
  app.S.seriesPick = null;
  app.renderSeries();
  const seriesPane = elements.get('pane-series');
  assert.ok(seriesPane.textContent.includes('other') && seriesPane.textContent.includes('42XX Block Of Fremont Ave N'));
  assert.ok(!seriesPane.textContent.includes('Why these cases?') && !seriesPane.textContent.includes('Independent review'));
  assert.equal(seriesPane.children.find(n => n.className === 'series-case-list').children.length, 2);
  const navigation = seriesPane.children[0].children[1].children;
  await navigation.find(n => n.attrs['aria-label'] === 'Next series').onclick();
  assert.equal(app.pickSeries().id, 'S-2');
  const blocked = find(seriesPane, 'Approve series alert');
  assert.ok(Object.hasOwn(blocked.attrs, 'disabled'));
  let approvalPosts = 0;
  context.fetch = async () => { approvalPosts++; throw new Error('Must not send'); };
  await blocked.onclick();
  assert.equal(approvalPosts, 0, 'Conflicted series must not submit an approval');
  assert.ok(seriesPane.textContent.includes('Travel windows conflict.'));

  app.S.seriesPick = 'S-3'; app.renderSeries();
  assert.ok(!Object.hasOwn(find(seriesPane, 'Approve series alert').attrs, 'disabled'), 'Weak checks remain reviewable');
  app.S.seriesPick = 'S-1'; app.renderSeries();
  context.fetch = async () => ({ ok: false, status: 503, json: async () => ({ error: 'Unavailable' }) });
  const retrySeries = find(seriesPane, 'Approve series alert');
  await retrySeries.onclick();
  assert.equal(app.S.decided['series:S-1'], undefined);
  assert.equal(retrySeries.disabled, false, 'Failed series approvals must allow retry');
  context.fetch = async path => ({ ok: true, json: async () => path === '/api/audit' ? { chain: { intact: true, entries: 1 }, entries: [] } : { entry: { seq: 1 } } });
  await retrySeries.onclick();
  assert.equal(app.S.decided['series:S-1'].kind, 'accepted');
  assert.ok(find(seriesPane, 'Approved'));
  const relatedRow = seriesPane.children.find(n => n.className === 'series-case-list').children[1].children[0];
  context.fetch = async () => ({ ok: true, json: async () => data('other') });
  await relatedRow.onclick();
  assert.equal(app.S.current.case.id, 'other');
  assert.equal(app.pickSeries().id, 'S-1', 'Opening a connected case must retain the series being reviewed');
  assert.equal(elements.get('tab-series').attrs['aria-selected'], 'true');
  app.S.current.case.id = 'unlinked'; app.S.seriesPick = null; app.renderSeries();
  assert.equal(app.pickSeries(), undefined, 'Do not silently show an unrelated series as linked to the selected case');
  assert.ok(seriesPane.textContent.includes('No linked series for this case.'));
  assert.equal(find(seriesPane, 'Approve series alert'), undefined);
  app.S.series = { series: [] }; app.renderSeries();
  assert.equal(seriesPane.textContent, 'No linked series.');

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
