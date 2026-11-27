import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';

const source = fs.readFileSync(new URL('../ding/morphixMenu.js', import.meta.url), 'utf8');
const ctx = vm.createContext({ imports: {}, log() {} });
vm.runInContext(source, ctx);
const plain = (value) => JSON.parse(JSON.stringify(value));

test('regular local files list actions', () =>
  assert.deepEqual(plain(ctx.listCommand(['file:///a b.png'])), ['@MORPHIX@', '--json', 'menu', '--list', '--uris', 'file:///a b.png']));
test('invalid selection opens khay with error', () =>
  assert.deepEqual(plain(ctx.commandForSelection(['file:///d'], false)), ['@MORPHIX@', 'tray', '--unsupported-selection']));
test('empty selection opens khay', () =>
  assert.deepEqual(plain(ctx.commandForSelection([], true)), ['@MORPHIX@', 'tray']));
test('valid selection without listing opens the menu', () =>
  assert.deepEqual(plain(ctx.commandForSelection(['file:///x.png'], true)), ['@MORPHIX@', 'menu', '--uris', 'file:///x.png']));
test('action command', () =>
  assert.deepEqual(plain(ctx.actionCommand('image:webp', ['file:///x.png'])), ['@MORPHIX@', 'menu', '--action', 'image:webp', '--uris', 'file:///x.png']));
test('listing parses', () =>
  assert.deepEqual(plain(ctx.parseListing('{"items":[{"id":"a","label":"A","is_tool":false}],"too_many":false}')), { tooMany: false, items: [{ id: 'a', label: 'A', isTool: false }] }));
test('broken listing is null', () => {
  for (const s of ['', 'x', '{}', '{"items":[{"id":1}],"too_many":false}', null]) assert.equal(ctx.parseListing(s), null);
});
test('limits match the Nautilus menu', () => {
  assert.equal(ctx.MAX_FILES, 100);
  assert.equal(ctx.TOO_MANY_LABEL, 'Chọn quá nhiều file (tối đa 100)');
});

// ---- addFileItems/addBackgroundItem with a fake Gtk/Gio: a DING quirk must never break DING's own menu ----
function fakeEnv() {
  const added = [];
  class MenuItem { constructor(props) { this.label = props.label; this.sub = null; } connect() {} set_submenu(s) { this.sub = s; } set_sensitive() {} }
  class Menu { constructor() { this.items = []; } add(i) { this.items.push(i); } }
  const logs = [];
  const env = vm.createContext({
    imports: { gi: { Gtk: { MenuItem, Menu }, Gio: { SubprocessFlags: {}, FileType: { REGULAR: 1 }, FileQueryInfoFlags: {}, SubprocessLauncher: class { unsetenv() {} spawnv() { throw new Error('no spawn in tests'); } } } } },
    log: (m) => logs.push(m),
  });
  vm.runInContext(source, env);
  const menu = { items: [], _menu: { add: (i) => added.push(i) }, _separators: 0, _addSeparator() { this._separators++; }, _desktopManager: { getCurrentSelection: () => [] } };
  return { env, menu, added, logs };
}

test('a stack marker never gets a Morphix item and never throws', () => {
  const { env, menu, added } = fakeEnv();
  const marker = { isStackMarker: true, file: 'text/plain' };
  env.addFileItems(menu, marker);
  assert.equal(added.length, 0);
  assert.equal(menu._separators, 0);
});

test('a selection that contains a stack marker is skipped quietly', () => {
  const { env, menu, added } = fakeEnv();
  menu._desktopManager.getCurrentSelection = () => [{ file: 'text/plain' }];
  env.addFileItems(menu, { file: { get_uri: () => 'file:///a.png' } });
  assert.equal(added.length, 0);
});

test('unexpected DING objects are logged, not thrown', () => {
  const { env, menu, logs } = fakeEnv();
  assert.doesNotThrow(() => env.addFileItems(menu, null));
  assert.doesNotThrow(() => env.addBackgroundItem(null));
  assert.ok(logs.length >= 1);
});
