'use strict';

// Runs the definition provider on a small workspace written to a temporary
// folder: two packages that both declare `Main` and `double`, a bundled
// library, and a loose file only the whole-workspace search reaches. Each
// check clicks one name and expects the nearest declaration.

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { loadExtension } = require('./fake-vscode');

const root = fs.mkdtempSync(path.join(os.tmpdir(), 'omega-definition-'));
const files = {
  'source/library/core/extent.omg': 'public data Extent { base: address; length: u64; }\npublic domain Extent::Granted;\n',
  'source/library/core/integer.omg': 'public machine max<T>() -> T;\n',
  'source/library/system/console.omg': 'public boundary trait Console {\n    machine write_line(text: @[u8]);\n}\n',
  'app/build.omg': 'machine build(builder: @mutable Build) {\n    dependency(Source::Path { location: "../dep" }); dependency(Source::Bundled { name: "system" });\n}\n',
  'dep/build.omg': 'machine build(builder: @mutable Build) {}\n',
  'dep/sound/sound.omg': 'public machine play(note: u64);\n',
  'dep/wave/wave.omg': 'public data Wave {}\n',
  'app/calling.omg': 'public machine plan(x: u64) -> u64 { x }\npublic data Main {}\n',
  'app/helpers/math.omg': 'public machine double(x: u64) -> u64 { x + x }\n',
  // A test fixture inside the package is its own package.
  'app/tests/fixtures/case/build.omg': 'machine build(builder: @mutable Build) {}\n',
  'app/tests/fixtures/case/main.omg': 'machine double(x: u64) -> u64 { x }\n',
  'app/main.omg': [
    'use calling.omg as abi;',
    'use {omega}/language/core/extent.omg;',
    'use {system}/console.omg;',
    'use {dep}/sound/sound.omg as sound;',
    'use {dep}/wave/wave.omg;',
    'data Main { extent: |extent|::Extent; count: u64; }',
    'machine entry_start(main: @mutable Main) {',
    '    let total: u64 = |abi|::plan(main.count);',
    '    let doubled: u64 = double(total);',
    '    let largest: u64 = |integer|::max::<u64>();',
    '    |console|::Console::write_line(orphan_target());',
    '    let wave: |wave|::Wave = |sound|::play(0);',
    '    |absent|::plan();',
    '    |abi|::play();',
    '    // Main in a comment',
    '}',
    'use {omega}/language/core/integer.omg;',
    'use {absent}/sound/sound.omg as missing;',
    'machine missing_lookup() { |missing|::play(0); }',
    '',
  ].join('\n'),
  'other/build.omg': 'machine build(builder: @mutable Build) {}\n',
  'other/main.omg': 'data Main {}\nmachine double(x: u64) -> u64 { x }\nmachine play(note: u64);\ndata Wave {}\n',
  'loose/notes.omg': 'machine orphan_target() -> u64 { 0 }\n',
  '_recycling/old.omg': 'machine orphan_target() -> u64 { 1 }\n',
};
for (const [name, text] of Object.entries(files)) {
  fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
  fs.writeFileSync(path.join(root, name), text);
}
test.after(() => fs.rmSync(root, { recursive: true, force: true }));

const definitionAt = loadExtension([root]);
const main = path.join(root, 'app/main.omg');
const where = async (context) =>
  (await definitionAt(main, context)).map((item) => `${path.relative(root, item.file).split(path.sep).join('/')}:${item.line}`);

test('an alias leads to the file its use line names', async () => {
  assert.deepEqual(await where('|abi|::pl$an'), ['app/calling.omg:1']);
  assert.deepEqual(await where('|absent|::pl$an'), []);
  assert.deepEqual(await where('|abi|::pl$ay'), []);
});

test('the current file wins over its imports and other packages', async () => {
  assert.deepEqual(await where('@mutable Ma$in'), ['app/main.omg:6']);
});

test('a member access finds the field', async () => {
  assert.deepEqual(await where('main.cou$nt'), ['app/main.omg:6']);
});

test('local let and parameter bindings', async () => {
  assert.deepEqual(await where('double(tot$al)'), ['app/main.omg:8']);
  assert.deepEqual(await where('(ma$in.count)'), ['app/main.omg:7']);
});

test('the package is searched before other packages', async () => {
  assert.deepEqual(await where('dou$ble(total)'), ['app/helpers/math.omg:1']);
});

test('library imports and module-qualified library names', async () => {
  assert.deepEqual(await where('extent: |extent|::Ext$ent'), ['source/library/core/extent.omg:1']);
  assert.deepEqual(await where('|integer|::ma$x'), ['source/library/core/integer.omg:1']);
  assert.deepEqual(await where('Console::write_$line'), ['source/library/system/console.omg:2']);
});

test('dependency packages from build.omg locations', async () => {
  assert.deepEqual(await where('|sound|::pl$ay'), ['dep/sound/sound.omg:1']);
  assert.deepEqual(await where('|missing|::pl$ay'), []);
  assert.deepEqual(await where('let wave: |wave|::Wa$ve'), ['dep/wave/wave.omg:1']);
});

test('the whole workspace is the last resort and skips the archive', async () => {
  assert.deepEqual(await where('orphan_tar$get()'), ['loose/notes.omg:1']);
});

test('names in comments lead nowhere', async () => {
  assert.deepEqual(await where('// Ma$in in'), []);
});
