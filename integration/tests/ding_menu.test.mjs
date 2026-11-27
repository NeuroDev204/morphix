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
