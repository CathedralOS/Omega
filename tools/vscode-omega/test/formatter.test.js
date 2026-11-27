'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { formatDocument, formatLineRange } = require('../src/formatter');
const { loadExtension } = require('./fake-vscode');

test('indents nested blocks and normalizes operator spacing', () => {
  const input = [
    'public   machine   add(a:u64,b:u64)->u64{',
    'let   x:u64=a+b;',
    'let y:u64=x+%1u64;',
    'if(y>10){',
    'return y-1u64;',
    '}',
    'y',
    '}',
  ].join('\n');

  const expected = [
    'public machine add(a: u64, b: u64) -> u64 {',
    '    let x: u64 = a + b;',
    '    let y: u64 = x +% 1u64;',
    '    if (y > 10) {',
    '        return y - 1u64;',
    '    }',
    '    y',
    '}',
    '',
  ].join('\n');

  assert.equal(formatDocument(input), expected);
  assert.equal(formatDocument(expected), expected);
});

test('preserves use paths, import aliases, lifetimes, borrows and generics', () => {
  const input = [
    'use   {omega}/language/core/extent.omg   as   range;',
    'use   main.omg   as   backing;',
    '',
    'public data   ActivationClaim   [linear];',
    'public data   VecCell   {value:u32;}',
    'public data   Empty   {}',
    'public machine with_alias<\'storage>(desc:@mutable   Dependency<\'storage>,alias:@\'storage   [u8])->@|range|::Extent{',
    'let   v:u64=|integer|::max::<u64>();',
    'let   s:@[u8]=items[1..];',
    'let   neg:i64=-1i64;',
    'let   inv:bool=!flag;',
    'let   shifted:u64=v>>%2u64;',
    'let   nested:Vec<Option<u64>>=empty;',
    'let   raw:u64=path as - |paths|::Path;',
    '}',
  ].join('\n');

  const expected = [
    'use {omega}/language/core/extent.omg as range;',
    'use main.omg as backing;',
    '',
    'public data ActivationClaim [linear];',
    'public data VecCell { value: u32; }',
    'public data Empty {}',
    'public machine with_alias<\'storage>(desc: @mutable Dependency<\'storage>, alias: @\'storage [u8]) -> @|range|::Extent {',
    '    let v: u64 = |integer|::max::<u64>();',
    '    let s: @[u8] = items[1..];',
    '    let neg: i64 = -1i64;',
    '    let inv: bool = !flag;',
    '    let shifted: u64 = v >>% 2u64;',
    '    let nested: Vec<Option<u64>> = empty;',
    '    let raw: u64 = path as - |paths|::Path;',
    '}',
    '',
  ].join('\n');

  assert.equal(formatDocument(input), expected);
  assert.equal(formatDocument(expected), expected);
});

test('aligns contract clauses and indents continuation lines', () => {
  const input = [
    'boundary trait ArrayAccess {',
    '  machine index<T, const N: u64>(items: @[T; N], index: u64) -> T',
    '    reaches none',
    '  requires',
    '  index < items.length',
    '    crashes none;',
    '}',
    '',
    'public machine push<Role>(vec: Vec, value: u32) -> Vec',
    '  requires vec.length < vec.capacity',
    '  ensures result.length == vec.length + 1,',
    '  result.capacity == vec.capacity',
    '  crashes Trap',
    '{',
    'vec',
    '}',
  ].join('\n');

  const expected = [
    'boundary trait ArrayAccess {',
    '    machine index<T, const N: u64>(items: @[T; N], index: u64) -> T',
    '    reaches none',
    '    requires',
    '        index < items.length',
    '    crashes none;',
    '}',
    '',
    'public machine push<Role>(vec: Vec, value: u32) -> Vec',
    'requires vec.length < vec.capacity',
    'ensures result.length == vec.length + 1,',
    '    result.capacity == vec.capacity',
    'crashes Trap',
    '{',
    '    vec',
    '}',
    '',
  ].join('\n');

  assert.equal(formatDocument(input), expected);
  assert.equal(formatDocument(expected), expected);
});

test('preserves line comments, nested block comments and collapses extra blank lines', () => {
  const input = [
    '',
    '',
    '// Top comment   ',
    '/* outer /* inner */ outer */',
    'machine foo() {',
    '  /* multi',
    '   * line',
    '   */',
    '  let x:u64=1;   // inline   ',
    '',
    '',
    '',
    '  x',
    '}',
    '',
    '',
  ].join('\n');

  const expected = [
    '// Top comment',
    '/* outer /* inner */ outer */',
    'machine foo() {',
    '    /* multi',
    '     * line',
    '     */',
    '    let x: u64 = 1; // inline',
    '',
    '    x',
    '}',
    '',
  ].join('\n');

  assert.equal(formatDocument(input), expected);
  assert.equal(formatDocument(expected), expected);
});

test('respects tabSize, insertSpaces and CRLF line endings', () => {
  const input = 'machine f() {\r\nlet x:u64=1;\r\n}\r\n';
  assert.equal(formatDocument(input, { tabSize: 2, insertSpaces: true }), 'machine f() {\r\n  let x: u64 = 1;\r\n}\r\n');
  assert.equal(formatDocument(input, { tabSize: 4, insertSpaces: false }), 'machine f() {\r\n\tlet x: u64 = 1;\r\n}\r\n');
});

test('formats a selected line range using surrounding indentation context', () => {
  const input = [
    'machine f() {',
    '  let a:u64=1;',
    'let b:u64=2+3;',
    '  let c:u64=4;',
    '}',
    '',
  ].join('\n');

  assert.equal(formatLineRange(input, 2, 2), '    let b: u64 = 2 + 3;');
});

test('VS Code document and range formatting providers work via extension activation', () => {
  const workspace = loadExtension([path.resolve(__dirname, '..')]);
  const messy = 'machine f(){\nlet x:u64=1+2;\n}\n';
  const clean = 'machine f() {\n    let x: u64 = 1 + 2;\n}\n';
  assert.equal(workspace.format(messy), clean);
  assert.equal(workspace.format(clean), clean);
  assert.equal(
    workspace.formatRange('machine f() {\nlet a:u64=1;\nlet b:u64=2;\n}\n', 2, 2),
    'machine f() {\nlet a:u64=1;\n    let b: u64 = 2;\n}\n',
  );
});

test('formatting is idempotent across every bundled library .omg file', () => {
  const libraryDir = path.resolve(__dirname, '../../../source/library');
  if (!fs.existsSync(libraryDir)) return;
  const files = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.name.endsWith('.omg')) files.push(full);
    }
  };
  walk(libraryDir);
  assert.ok(files.length > 0, 'expected bundled library .omg files');
  for (const file of files) {
    const source = fs.readFileSync(file, 'utf8');
    const once = formatDocument(source);
    const twice = formatDocument(once);
    assert.equal(twice, once, `formatter was not idempotent on ${file}`);
  }
});
