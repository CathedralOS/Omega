#!/usr/bin/env python3
"""Run the Omega-written scalar compiler, then its actual Alpha output.

Python frames input, checks observations and invokes the existing seed stamper.
It contains no source compiler, evaluator, Alpha encoder or alternate output.
"""
import hashlib
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[3]
PROGRAM = sys.argv[1]
COUNT = 0


def compile_source(source, entry=0):
    completed = subprocess.run([PROGRAM], input=struct.pack('<I', entry) + source,
                               capture_output=True, timeout=60)
    assert completed.stderr == b'', completed.stderr
    return completed


def accepted(name, source, expected, entry=0):
    global COUNT
    result = compile_source(source, entry)
    assert result.returncode == 0 and result.stdout, (name, result.returncode, result.stdout)
    repeated = compile_source(source, entry)
    assert (repeated.returncode, repeated.stdout) == (0, result.stdout), name + ': nondeterministic tape'
    with tempfile.TemporaryDirectory() as directory:
        tape = Path(directory) / 'program.tape'
        executable = Path(directory) / 'program'
        tape.write_bytes(result.stdout)
        subprocess.run(['sh', '-c', '''
set -eu
OMEGA_REPO_ROOT=$1
. "$OMEGA_REPO_ROOT/tools/bootstrap/paths.sh"
. "$OMEGA_REPO_ROOT/tools/bootstrap/alpha/seed_env.sh"
require_seed_execution_host 'Omega-written Alpha compiler'
stamp_seed "$2" "$OMEGA_PATH_ALPHA/$ALPHA_SEED" "$3"
''', 'stamp', str(ROOT), str(tape), str(executable)], check=True, capture_output=True)
        run = subprocess.run([str(executable)], input=b'', capture_output=True, timeout=10)
        assert (run.returncode, run.stdout, run.stderr) == (expected, b'', b''), (name, run)
    COUNT += 1
    print(f'{name}: exit={expected} tape_bytes={len(result.stdout)} sha256={hashlib.sha256(result.stdout).hexdigest()}')
    return result.stdout


def refused(name, source, status, entry=0):
    global COUNT
    result = compile_source(source, entry)
    assert (result.returncode, result.stdout) == (status, b''), (name, result.returncode, result.stdout)
    COUNT += 1


def domain_cast_cases():
    """Canonical explicit accounting, through real checking and Alpha output."""
    for policy in (b'Trapping', b'Wrapping', b'Saturating'):
        source = (b'machine chosen() -> u8 { let x: u8 in ' + policy
                  + b' = 42 as u8 +' + policy + b'; (x as =' + policy
                  + b') as -' + policy + b' }')
        accepted('explicit-policy-' + policy.decode(), source, 42)
        refused('policy-declaration-cannot-shadow-' + policy.decode(),
                b'domain u8::' + policy + b' requires self==0; '
                b'machine chosen()->u8{(42 as u8 +' + policy + b') as -' + policy + b'}', 248)
    accepted('policy-change-keeps-value',
             b'machine chosen()->u8 { ((42 as u8 +Trapping) as -Trapping as u8 +Wrapping) as -Wrapping }', 42)
    accepted('cast-does-not-wrap',
             b'machine chosen()->u8 { (255 as u8 +Wrapping) as -Wrapping }', 255)
    refused('cast-out-of-range-before-policy',
            b'machine chosen()->u8 { (256 as u8 +Wrapping) as -Wrapping }', 249)
    for name, source in (
        ('implicit-local', b'machine chosen()->u8 { let x:u8 in Trapping=42; x as -Trapping }'),
        ('implicit-field', b'data Box { value:u8 in Trapping; } machine chosen()->u8 { let x:Box=Box{value:42}; x.value as -Trapping }'),
        ('implicit-argument', b'machine take(x:u8 in Trapping)->u8{x as -Trapping} machine chosen()->u8{take(42)}'),
        ('implicit-result', b'machine result()->u8 in Trapping{42} machine chosen()->u8{result() as -Trapping}'),
        ('implicit-array-element', b'machine chosen()->u8 { let x:[u8 in Trapping;1]=[42]; x[0] as -Trapping }'),
        ('implicit-array-shell', b'machine chosen()->u8 { let x:[u8;1] in Trapping=[42]; 0 }'),
        ('implicit-nested-array-shell', b'machine chosen()->u8 { let x:[[u8;1] in Trapping;1]=[[42]]; 0 }'),
        ('implicit-match-arm', b'machine chosen()->u8 { let x:u8 in Trapping=match true {true->42 false->43}; x as -Trapping }'),
        ('implicit-policy-removal', b'machine chosen()->u8 { let x:u8 in Trapping=42 as u8 +Trapping; x }'),
        ('carrier-cast-cannot-drop-policy', b'machine chosen()->u8 { let x:u8 in Trapping=42 as u8 +Trapping; x as u8 }'),
        ('duplicate-addition', b'machine chosen()->u8 { ((42 as u8 +Trapping) as +Trapping) as -Trapping }'),
        ('missing-removal-domain', b'machine chosen()->u8 { 42 as u8 as -Trapping }'),
        ('missing-retention-domain', b'machine chosen()->u8 { 42 as u8 as =Trapping }'),
        ('wrong-retention-domain', b'machine chosen()->u8 { ((42 as u8 +Trapping) as =Wrapping) as -Wrapping }'),
        ('anonymous-policy-needs-carrier', b'machine chosen()->u8 { (42 as +Trapping) as -Trapping }'),
        ('unsupported-declared-domain', b'domain u8::Small requires self<100; machine chosen()->u8{(42 as u8 +Small) as -Small}'),
        ('implicit-declared-erasure', b'domain u8::Small requires self<100; machine chosen(x:u8 in Small)->u8{x}'),
        ('qualified-collection-needs-selected-operator', b'machine chosen()->u8 { let mut x:[u8;1] in Trapping; x[0]=42; 0 }'),
        ('qualified-index-needs-selected-operator', b'machine chosen()->u8 { let mut x:[u8;1]; let i:u8 in Trapping=0 as u8 +Trapping; x[i]=42; 0 }'),
        ('qualified-range-start-needs-selected-operator', b'machine chosen()->u8 { let x:[u8;1]; let i:u8 in Trapping=0 as u8 +Trapping; let view:&[u8]=&x[i..1]; 0 }'),
        ('qualified-range-end-needs-selected-operator', b'machine chosen()->u8 { let x:[u8;1]; let i:u8 in Trapping=1 as u8 +Trapping; let view:&[u8]=&x[0..i]; 0 }'),
    ):
        refused(name, source, 249)
    for name, source in (
        ('retired-whole-type-cast', b'machine chosen()->u8{42 as u8 in Trapping}'),
        ('carrier-removal-entry', b'machine chosen()->u8{42 as u8 -Trapping}'),
        ('carrier-retention-entry', b'machine chosen()->u8{42 as u8 =Trapping}'),
        ('simultaneous-entries-not-sequential-casts', b'machine chosen()->u8{42 as u8 +Trapping -Trapping}'),
    ):
        refused(name, source, 250)
    # Pack the actual source declarations unchanged. The wrapper supplies a
    # canonical caller; Python implements no part of their behavior. Accepted
    # checking must reach the scalar lowerer's explicit storage refusal, which
    # does not claim that this backend emitted a SourceUnit program.
    owned = (ROOT / 'source/psi/source/source.omg').read_bytes()
    source_id = owned[owned.index(b'public data SourceId [copy]'):
                      owned.index(b'public data Span [copy]')]
    source_unit = owned[owned.index(b'public data SourceUnit'):
                        owned.index(b'machine source_unit_append(')]
    source_unit = source_id + source_unit + b"""machine chosen()->u8 {
 let mut unit:SourceUnit;
 let id:SourceId=SourceId{value:77 as u32 +Trapping};
 unit.length=19; unit.last_retained=false;
 (&mut unit.bytes as -Trapping)[0]=42;
 source_unit_clear(&mut unit,id);
 transition (unit.id.value as -Trapping)==(id.value as -Trapping)
  && unit.length==0 && unit.last_retained
  && (&unit.bytes as -Trapping)[0]==42 { true->42 false->1 }
}"""
    refused('actual-SourceUnit-checked-before-storage-frontier', source_unit, 246)
    refused('SourceUnit-selector-still-owes-bounds',
            source_unit.replace(b'[0]=42', b'[131072]=42'), 247)
    refused('SourceUnit-view-keeps-shared-access',
            source_unit.replace(b'(&mut unit.bytes as -Trapping)[0]=42',
                                b'(&unit.bytes as -Trapping)[0]=42'), 249)
    refused('shared-reference-binding-does-not-make-referent-mutable',
            b'machine chosen()->u8{let mut bytes:[u8;1];let mut view:&[u8;1]=&bytes;view[0]=42;0}', 249)
    refused('chained-view-keeps-call-argument-loan', b"""
machine write(a:&mut [u8;1],b:&mut [u8;1]) { a[0]=1; b[0]=2; }
machine chosen()->u8 {
 let mut bytes:[u8;1] in Trapping;
 write(((&mut bytes as -Trapping) as +Trapping) as -Trapping, &mut bytes as -Trapping);
 0
}""", 247)
    refused('chained-view-keeps-held-local-loan', b"""
machine chosen()->u8 {
 let mut bytes:[u8;1] in Trapping;
 let view:&mut [u8;1]=((&mut bytes as -Trapping) as +Trapping) as -Trapping;
 (&mut bytes as -Trapping)[0]=1;
 view[0]=2;
 0
}""", 247)
    refused('chained-view-cannot-escape-owned-local', b"""
machine escaped()->&[u8;1] {
 let bytes:[u8;1] in Trapping;
 ((&bytes as -Trapping) as +Trapping) as -Trapping
}
machine chosen()->u8 { 42 }
""", 247)
    # Domain-only nodes have no type-reference row. They must not rebind
    # the unrelated first row (the data's generic T) under this machine.
    refused('domain-cast-keeps-unrelated-type-scope',
            b'data Box<T>{value:T;} machine chosen()->u8{(42 as u8 +Trapping) as -Trapping}', 246)
    print(f'Omega-written canonical casts: {COUNT} actual source/output cases passed')


def main():
    first = accepted('literal', b'machine chosen() -> u8 { 42 }', 42)
    changed = accepted('changed-source', b'machine chosen() -> u8 { 43 }', 43)
    assert first != changed
    accepted('renamed-entry', b'machine different() -> u8 { 42 }', 42)
    accepted('anonymous-rational', b'machine f() -> u8 { ((5 / 2) * 2) as u8 }', 5)
    accepted('negated-rational', b'machine f() -> u8 { (-(5 / 2) * (-2)) as u8 }', 5)
    refused('implicit-fractional-history', b'machine f() -> u8 { (5 / 2) * 2 }', 249)
    accepted('anonymous-negative-intermediate', b'machine f() -> u8 { (1 - 2) + 43 }', 42)
    accepted('typed-division', b'machine f() -> u8 { let a: u8 = 5; a / 2 }', 2)
    accepted('typed-remainder', b'machine f() -> u8 { let a: u8 = 5; a % 2 }', 1)
    accepted('local-arithmetic', b'machine f() -> u8 { let a: u8 = 6; let b: u8 = a * 7; b - 1 }', 41)
    accepted('forward-call', b'machine start() -> u8 { later() } machine later() -> u8 { 37 }', 37)
    accepted('selected-callable', b'machine first() -> u8 { 37 } machine second() -> u8 { 38 }', 38, 1)
    accepted('caller-values', b'machine start() -> u8 { let a: u8 = left(); let b: u8 = right(); a + b } '
             b'machine left() -> u8 { 17 } machine right() -> u8 { inner() } machine inner() -> u8 { 25 }', 42)
    refused('fractional-landing', b'machine f() -> u8 { 5 / 2 }', 249)
    refused('out-of-range', b'machine f() -> u8 { 256 }', 249)
    refused('unchecked-overflow', b'machine f() -> u8 { let a: u8 = 255; a + 1 }', 247)
    refused('typed-intermediate-underflow', b'machine f() -> u8 { let a: u8 = 1; (a - 2) + 43 }', 247)
    refused('typed-multiply-overflow', b'machine f() -> u8 { let a: u8 = 128; a * 2 }', 247)
    accepted('inferred-call-sum', b'machine f() -> u8 { g() + g() } machine g() -> u8 { 1 }', 2)
    refused('callee-result-overflow', b'machine f() -> u8 { g() + g() } machine g() -> u8 { 128 }', 247)
    refused('zero-divisor', b'machine f() -> u8 { let a: u8 = 5; a / 0 }', 247)
    refused('unsupported-width', b'machine f() -> u16 { 42 }', 246)
    refused('unsupported-mutation', b'machine f() -> u8 { let mut a: u8 = 1; a = 2; a }', 246)
    accepted('nested-call', b'machine start() -> u8 { nested() machine nested() -> u8 { 42 } }', 42)
    refused('nested-capture', b'machine start() -> u8 { let a: u8 = 42; nested() machine nested() -> u8 { a } }', 248)
    refused('qualified-owner', b'data Box {} machine Box::f() -> u8 { 42 }', 246)
    refused('trait-default-entry', b'trait T { machine f() -> u8 { 42 } }', 246)
    refused('typed-underflow-before-cast', b'machine f() -> u8 { let a: u8 = 1; ((a - 2) + 43) as u8 }', 247)
    refused('peer-landing-before-cast', b'machine f() -> u8 { ((5 / 2) * 2 as u8) as u8 }', 249)
    # Named-domain accounting is still outside this bounded policy slice.
    # Parsing accepts canonical syntax; typing explicitly refuses its coverage.
    refused('unsupported-domain-cast', b'domain u8::Small requires self < 4; '
            b'machine f() -> u8 { let x: u8 in Small = (((5 / 2) * 2) as u8) as +Small; 0 }', 249)
    # Accepted checking reaches the explicit contract-coverage refusal; the
    # matched false guarantee must fail before lowering. This exercises the
    # fact-expression evaluator without claiming contract backend support.
    refused('landed-fact-checked', b'machine f() -> u8 ensures result == (((5 / 2) * 2) as u8) { 5 }', 246)
    refused('landed-fact-false', b'machine f() -> u8 ensures result == (((5 / 2) * 2) as u8) { 4 }', 247)
    refused('entry-precondition', b'machine f() -> u8 requires false { 42 }', 246)
    refused('recursive-cycle', b'machine f() -> u8 { f() }', 246)
    refused('mutual-cycle', b'machine f() -> u8 { g() } machine g() -> u8 { f() }', 246)
    refused('uncalled-cycle', b'machine f() -> u8 { 42 } machine g() -> u8 { g() }', 246)
    refused('missing-selected-entry', b'machine f() -> u8 { 1 }', 246, 1)
    refused('unreachable-bad-body', b'machine f() -> u8 { 1 } machine bad() -> u8 { nope }', 248)
    refused('invalid-source', b'machine f( {', 250)
    refused('unsupported-unused-constant', b'const X: u8 = 1; machine f() -> u8 { 42 }', 246)
    refused('unsupported-domain-root', b'domain u8::Small requires self < 4; '
            b'machine f() -> u8 { 42 }', 246)
    refused('erased-runtime-local', b'machine f() -> u8 { let x [erased]: u8 = 42; x }', 246)
    refused('nested-selected-entry', b'machine outer() -> u8 { 42 machine hidden() -> u8 { 7 } }', 246, 1)
    print(f'Omega-written Alpha compiler: {COUNT} actual source/output cases passed')


if __name__ == '__main__':
    # Both suites retain the exact host-built compiler for additional inputs
    # without repeating native construction. This is outside captured source.
    retained = ROOT / 'build/product-alpha/build/compiler-under-test'
    retained.parent.mkdir(parents=True, exist_ok=True)
    if Path(PROGRAM).resolve() != retained.resolve():
        shutil.copy2(PROGRAM, retained)
    if len(sys.argv) == 3 and sys.argv[2] == '--domain-casts':
        domain_cast_cases()
    else:
        main()
