'use strict';

// Checks the text scan behind Go to Definition on Omega shapes taken from the
// bundled library and samples. Run from this folder with `bun test` or
// `node --test`.

const test = require('node:test');
const assert = require('node:assert/strict');
const scan = require('../src/declarations');

function declarationsOf(text) {
  return scan.parseDeclarations(text).map(({ kind, name, qualifier, owner }) => ({ kind, name, qualifier, owner }));
}

// Returns the reference at the `$` marker, with the marker removed.
function at(source) {
  const offset = source.indexOf('$');
  const text = source.slice(0, offset) + source.slice(offset + 1);
  const masked = scan.maskCommentsAndStrings(text);
  const identifier = scan.identifierAt(text, masked, offset);
  return { text, masked, offset, identifier, reference: identifier && scan.referenceContext(masked, identifier.start, identifier.end) };
}

test('comments and strings hide names but keep offsets', () => {
  const text = 'a /* x /* y */ z */ b // machine c\n"machine d" e';
  const masked = scan.maskCommentsAndStrings(text);
  assert.equal(masked.length, text.length);
  assert.equal(masked.indexOf('\n'), text.indexOf('\n'));
  assert.deepEqual(masked.split(/\s+/).filter(Boolean), ['a', 'b', '"', '"', 'e']);
});

test('an unfinished string ends at the line break', () => {
  const masked = scan.maskCommentsAndStrings('let s = "open\nmachine after() {}');
  assert.deepEqual(declarationsOf(masked).map((item) => item.name), ['after']);
});

test('declarations record qualifier and owner', () => {
  const text = [
    'public boundary data ActivationClaim [linear];',
    'public data ActivationClaimAttempt {',
    '    case Established(claim: ActivationClaim);',
    '    case Rejected;',
    '}',
    'public boundary machine Activation::try_claim<const Bound: u64>(committed: u64)',
    '    -> ActivationClaimAttempt;',
    'public domain VecCell::Controlled',
    '    access VecCellPolicy;',
    'public domain [u8]::Path',
    '    access PathPolicy;',
    'public VecCellLayout satisfies FixedLayout<VecCell> {',
    '    public machine plan(schema: Schema) -> LayoutPlan {}',
    '}',
    'public trait Calling<Policy satisfies CallingPolicy>',
    '{ }',
    'machine agree<',
    '    Encode satisfies ValueConversion<Pair, Frame>,',
    '    Decode satisfies ValueConversion<Frame, Pair>>() {}',
  ].join('\n');
  assert.deepEqual(declarationsOf(text), [
    { kind: 'data', name: 'ActivationClaim', qualifier: '', owner: '' },
    { kind: 'data', name: 'ActivationClaimAttempt', qualifier: '', owner: '' },
    { kind: 'case', name: 'Established', qualifier: '', owner: 'ActivationClaimAttempt' },
    { kind: 'case', name: 'Rejected', qualifier: '', owner: 'ActivationClaimAttempt' },
    { kind: 'machine', name: 'try_claim', qualifier: 'Activation', owner: '' },
    { kind: 'domain', name: 'Controlled', qualifier: 'VecCell', owner: '' },
    { kind: 'domain', name: 'Path', qualifier: '[u8]', owner: '' },
    { kind: 'implementation', name: 'VecCellLayout', qualifier: '', owner: '' },
    { kind: 'machine', name: 'plan', qualifier: '', owner: '' },
    { kind: 'trait', name: 'Calling', qualifier: '', owner: '' },
    { kind: 'machine', name: 'agree', qualifier: '', owner: '' },
  ]);
});

test('contextual words used as names are not declarations', () => {
  const text = 'data Plan { state: StatePlan; data: u8; }\nmachine run() { write(data, state); }';
  assert.deepEqual(declarationsOf(text), [
    { kind: 'data', name: 'Plan', qualifier: '', owner: '' },
    { kind: 'field', name: 'state', qualifier: '', owner: 'Plan' },
    { kind: 'field', name: 'data', qualifier: '', owner: 'Plan' },
    { kind: 'machine', name: 'run', qualifier: '', owner: '' },
  ]);
});

test('fields come from the data body, not from member machines', () => {
  const text = [
    'public data Bump',
    'requires',
    '    self.remaining == self.tail.length,',
    '{',
    '    tail: Extent in Granted & Vacant;',
    '    remaining: u64;',
    '}',
    'public trait Console {',
    '    machine write_line(text: @[u8]) { let n: u64 = 0; }',
    '    limit: u64;',
    '}',
  ].join('\n');
  const fields = declarationsOf(text).filter((item) => item.kind === 'field');
  assert.deepEqual(fields.map((item) => `${item.owner}.${item.name}`), ['Bump.tail', 'Bump.remaining', 'Console.limit']);
});

test('generic const parameters stay local', () => {
  const names = declarationsOf('machine split<const BoundAt: u64>(x: u64) {}\nconst LIMIT: u64 = 4;').map((item) => item.name);
  assert.deepEqual(names, ['split', 'LIMIT']);
});

test('reference context reads alias, qualifier and member access', () => {
  assert.deepEqual(at('block |output|::WavOutput::write_$wav(x)').reference, {
    name: 'write_wav', alias: 'output', qualifier: 'WavOutput', member: false,
  });
  assert.deepEqual(at('integer::m$ax<u32>()').reference, { name: 'max', alias: '', qualifier: 'integer', member: false });
  assert.deepEqual(at('let a = strategy.rem$aining;').reference, { name: 'remaining', alias: '', qualifier: '', member: true });
  assert.equal(at('for i in 0..li$mit').reference.member, false);
});

test('no reference inside comments, strings, keywords or lifetimes', () => {
  assert.equal(at('// Ma$in').identifier, null);
  assert.equal(at('"Ma$in"').identifier, null);
  assert.equal(at('mach$ine run() {}').identifier, null);
  assert.equal(at("view<'stor$age>").identifier, null);
  assert.equal(at('plan.sta$te').identifier.name, 'state');
});

test('local bindings: let, parameter and generic parameter', () => {
  const source = (body) => [
    'machine view_from_profile<View, const Bound: u64>(main: @mutable Main, length: u64) -> u64 {',
    '    let base: address = main.base;',
    '    if length > 0 {',
    '        let inner: u64 = 1;',
    '    }',
    `    ${body}`,
    '}',
  ].join('\n');
  const find = (body) => {
    const { text, masked, identifier } = at(source(body));
    const offset = scan.findLocalBinding(masked, scan.parseDeclarations(text, masked), identifier.name, identifier.start);
    return offset < 0 ? null : text.slice(offset, offset + identifier.name.length) + '@' + text.slice(0, offset).split('\n').length;
  };
  assert.equal(find('return ba$se;'), 'base@2');
  assert.equal(find('return len$gth;'), 'length@1');
  assert.equal(find('return ma$in.base;'), 'main@1');
  assert.equal(find('let v: Vi$ew = x;'), 'View@1');
  assert.equal(find('return Bou$nd;'), 'Bound@1');
  assert.equal(find('return other$_name;'), null);
});

test('a state sees its own parameters and the machine around it', () => {
  const text = [
    'machine search(main: @mutable Main) {',
    '    let limit: u64 = 3;',
    '    state step(index: u64) {',
    '        transition index + limit + main.n;',
    '    }',
    '}',
  ].join('\n');
  const masked = scan.maskCommentsAndStrings(text);
  const declarations = scan.parseDeclarations(text, masked);
  const line = (name, after) => {
    const offset = scan.findLocalBinding(masked, declarations, name, text.indexOf(after) + after.length);
    return text.slice(0, offset).split('\n').length;
  };
  assert.equal(line('index', 'transition '), 3);
  assert.equal(line('limit', 'index + '), 2);
  assert.equal(line('main', 'limit + '), 1);
});

test('use lines and where they point', () => {
  const masked = scan.maskCommentsAndStrings([
    'use calling.omg as abi;',
    'use {omega}/language/core/numeric_conversion.omg;',
    'use omega\\language\\core\\Extent::Granted;',
    'use system\\Console;',
    'use actors\\state\\Mood;',
    'use kit\\main as k;',
    'use {system}/filesystem_host.omg as paths;',
    'use {wav_output}/wav_output/wav_output.omg as output;',
    'use {omega}/build/dependencies/providers/git.omg as remote;',
  ].join('\n'));
  const uses = scan.parseUses(masked);
  const target = (index, name, extra = {}) => scan.importTarget(uses[index], { name, alias: '', qualifier: '', member: false, ...extra });
  assert.deepEqual(target(8, 'pin', { alias: 'remote' }), { library: 'build', file: 'dependencies/providers/git.omg' });
  assert.deepEqual(target(0, 'plan'), { file: 'calling.omg' });
  assert.equal(target(0, 'plan', { alias: 'other' }), null);
  assert.equal(target(0, 'plan', { alias: 'calling' }), null);
  assert.deepEqual(target(1, 'convert', { alias: 'numeric_conversion' }), { library: 'core', file: 'numeric_conversion.omg' });
  assert.deepEqual(target(1, 'convert'), { library: 'core', file: 'numeric_conversion.omg' });
  assert.deepEqual(target(2, 'Granted'), { library: 'core' });
  assert.deepEqual(target(2, 'Vacant', { qualifier: 'Extent' }), { library: 'core' });
  assert.equal(target(2, 'Unrelated'), null);
  assert.deepEqual(target(3, 'Console'), { module: 'system', dependency: 'system', inside: '' });
  assert.deepEqual(target(4, 'Mood'), { module: 'actors/state', dependency: 'actors', inside: 'state' });
  assert.deepEqual(target(5, 'start', { alias: 'k' }), { module: 'kit/main', dependency: 'kit', inside: 'main' });
  assert.deepEqual(target(6, 'Path', { alias: 'paths' }), { dependency: 'system', file: 'filesystem_host.omg' });
  assert.deepEqual(target(7, 'write_wav', { alias: 'output' }), { dependency: 'wav_output', file: 'wav_output/wav_output.omg' });
});

test('qualified selection prefers the qualifier, then falls back to the name', () => {
  const declarations = scan.parseDeclarations([
    'machine Activation::release(claim: u64);',
    'machine ActivationClaim::release(claim: u64);',
    'data Attempt { case Rejected; }',
    'data Split { case Rejected(claim: u64); }',
  ].join('\n'));
  const pick = (name, qualifier, stem = 'activation') => {
    const { matches, qualified } = scan.selectDeclarations(declarations, { name, alias: '', qualifier, member: false }, stem);
    return { owners: matches.map((item) => item.qualifier || item.owner), qualified };
  };
  assert.deepEqual(pick('release', 'ActivationClaim'), { owners: ['ActivationClaim'], qualified: true });
  assert.deepEqual(pick('Rejected', 'Split'), { owners: ['Split'], qualified: true });
  assert.deepEqual(pick('release', 'activation'), { owners: ['Activation', 'ActivationClaim'], qualified: true });
  assert.deepEqual(pick('release', 'Elsewhere'), { owners: ['Activation', 'ActivationClaim'], qualified: false });
});
