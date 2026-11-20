#!/usr/bin/env python3
"""Check service identity and call ceilings against actual bundled core declarations."""
import argparse
from pathlib import Path
import runpy
import signal
import struct
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tests/bootstrap'))
from compiled import add_invocation_budget, execute, pack, stamp
from conformance.vectors import epsilon_request


def cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    app_import = b'use dep\\read_value;\n'
    core_import = b'use omega\\language\\core\\ProcessOutput;\n'
    helper = core_import + b'public machine read_value()->u8 reaches ProcessOutput {42}'
    packages = [('app', 0), ('provider_free_library', 0), ('omega_language_core', 1)]
    sites = [(0, 1, 'dep')]

    def trial(name, app, expected=None, *, helper_source=helper, edges=sites, roles=packages):
        rows = [(0, app), (1, helper_source), (2, (core / 'process_authority.omg').read_bytes())]
        paths = ['main.omg', 'read.omg', 'process_authority.omg']
        packet, _, _, units = encode(roles, edges, rows, 0, b'main', paths=paths)
        if expected is None:
            return name, packet, None
        unit, token = expected
        coordinate = units[unit][1] + rows[unit][1].rindex(token)
        return name, packet, b'\0' + struct.pack('<II', 1, coordinate)

    relay = b'machine relay()->u8 {read_value()}\n'
    main = b'machine main()->u8 reaches ProcessOutput + ProcessOutput {relay()}'
    app = app_import + core_import + relay + main
    yield trial('same-service', app)
    yield trial('inferred', app.replace(b' reaches ProcessOutput + ProcessOutput', b''))
    yield trial('separate-ceilings', app.replace(b'ProcessOutput + ProcessOutput',
                b'ProcessInput reaches ProcessOutput').replace(relay,
                b'use omega\\language\\core\\ProcessInput;\n' + relay))
    yield trial('pure', app.replace(b'ProcessOutput + ProcessOutput', b'none'),
                helper_source=helper.replace(b'reaches ProcessOutput', b'reaches none'))
    yield trial('empty', app.replace(b'ProcessOutput + ProcessOutput', b'none'), (0, b'relay()'))
    yield trial('wrong-service', app.replace(b'machine main', b'use omega\\language\\core\\ProcessInput;\nmachine main')
                .replace(b'ProcessOutput + ProcessOutput', b'ProcessInput'), (0, b'relay()'))
    yield trial('same-spelling-other-owner', app.replace(core_import, b'boundary trait ProcessOutput {}\n'),
                (0, b'relay()'))
    yield trial('absent-edge', app, (0, b'use dep'), edges=[])
    yield trial('untrusted-core', app, (0, b'use omega'), roles=[*packages[:2], ('omega_language_core', 0)])
    yield trial('ordinary-name', app.replace(b'ProcessOutput + ProcessOutput', b'read_value'),
                (0, b'read_value'))
    yield trial('mixed-empty', app.replace(b'ProcessOutput + ProcessOutput', b'none + ProcessOutput'),
                (0, b'none'))
    yield trial('duplicate-declaration', app.replace(relay, b'boundary trait A {} boundary trait A {} ' + relay),
                (0, b'A {}'))
    yield trial('reserved-type', app.replace(relay, b'boundary trait u97 {} ' + relay), (0, b'u97'))
    yield trial('retired-marker', app.replace(relay, b'boundary trait Block {} ' + relay)
                .replace(b'ProcessOutput + ProcessOutput', b'Block'), (0, b'Block'))
    yield trial('private-import', app, (0, b'use dep'), helper_source=helper.replace(b'public machine', b'machine'))

    # A compiler role and an explicit reach ceiling cannot install ProcessExit.
    terminal = (b'use omega\\language\\core\\ProcessExit; machine main()->u8 reaches ProcessExit '
                b'{ ProcessExit::exit_process(253); }')
    declaration = (core / 'process_exit.omg').read_bytes()
    units = [(0, terminal), (1, declaration), (1, (core / 'never.omg').read_bytes())]
    packet, _, _, rows = encode([('app', 0), ('omega_language_core', 1)], [], units, 0, b'main',
                                paths=['main.omg', 'process_exit.omg', 'never.omg'])
    coordinate = rows[1][1] + declaration.index(b'boundary trait ProcessExit')
    yield 'uninstalled-exit', packet, b'\0' + struct.pack('<II', 2, coordinate)



def terminal_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    terminal = (core / 'process_exit.omg').read_bytes()
    bottom = (core / 'never.omg').read_bytes()
    names = [('app', 0), ('omega_language_core', 1)]

    def trial(name, app, outcome, *, service=terminal, result=bottom):
        units = [(0, app), (1, service), (1, result)]
        packet, _, _, rows = encode(names, [], units, 0, b'main',
                                    paths=['main.omg', 'process_exit.omg', 'never.omg'])
        if isinstance(outcome, tuple):
            code, unit, spelling = outcome
            source_coordinate = rows[unit][1] + units[unit][1].rindex(spelling)
            outcome = b'\0' + struct.pack('<II', code, source_coordinate)
        return name, packet, outcome

    import_exit = b'use omega\\language\\core\\ProcessExit;\n'
    import_never = b'use omega\\language\\core\\Never;\n'
    marker = b'machine main()->u8 reaches ProcessExit {42}'
    yield trial('terminal-declaration', import_exit + marker, None)
    yield trial('inhabited-lookalike-result', import_exit + marker,
                (2, 1, b'boundary trait ProcessExit'), result=b'public data Never {}')
    yield trial('wrong-requirement-parameter', import_exit + marker,
                (2, 1, b'boundary trait ProcessExit'), service=terminal.replace(b'return_code: i32', b'return_code: u32'))
    yield trial('missing-requirement-reach', import_exit + marker, (2, 1, b'boundary trait ProcessExit'),
                service=terminal.replace(b'reaches\n        ProcessExit', b'reaches none'))
    stop = b'machine stop()->Never crashes Trap {crash Trap;}\n'
    relay = b'machine relay()->Never crashes Trap {stop();}\n'
    dispatch = (b'machine choose(stop_now:bool)->u8 crashes Trap {'
                b'transition stop_now {true -> stopped(), _ -> ordinary()} '
                b'machine stopped()->u8 crashes Trap {relay();} machine ordinary()->u8{42}}\n')
    body = import_never + stop + relay + dispatch
    yield trial('uninhabited-untaken', body + b'machine main()->u8 crashes Trap {choose(false)}', None)
    yield trial('uninhabited-taken', body + b'machine main()->u8 crashes Trap {choose(true)}', 'trap')
    yield trial('uninhabited-value-context', import_never + stop + b'machine main()->u8 crashes Trap {stop()}', 'trap')
    yield trial('uninhabited-unrelated-name', b'data Void requires false {}\nmachine fail()->Void crashes Trap '
                b'{crash Trap;} machine main()->u8 crashes Trap {fail();}', 'trap')
    yield trial('zero-uninhabited', import_never + b'machine main()->u8 {let mut value:Never;42}', (1, 0, b'let'))
    yield trial('construct-uninhabited', import_never + b'machine wrong()->Never {Never {}} machine main()->u8{42}',
                (1, 0, b'Never {}'))
    yield trial('normal-result-lookalike', b'data Never [copy] {} machine fake()->Never {Never {}} '
                b'machine main()->u8 {fake()}', (1, 0, b'fake()'))


def provider_cases():
    encode = runpy.run_path(str(ROOT / 'source/application/construction/construct.py'))['encode']
    core = ROOT / 'source/library/core'
    service = (core / 'process_exit.omg').read_bytes()
    bottom = (core / 'never.omg').read_bytes()
    primitive = (core / 'process_exit_native.omg').read_bytes()
    provider = (ROOT / 'source/library/std/process_exit.omg').read_bytes()
    packages = [('app', 0), ('omega_language_core', 1), ('omega_language_std', 2)]
    paths = ['main.omg', 'process_exit.omg', 'never.omg', 'process_exit_native.omg', 'process_exit.omg']
    marker = b'use omega\\language\\core\\ProcessExit; machine main()->u8 reaches ProcessExit {42}'

    def trial(name, app=marker, outcome=None, *, boundary=primitive, wrapper=provider, result=bottom):
        units = [(0, app), (1, service), (1, result), (1, boundary), (2, wrapper)]
        packet, _, _, rows = encode(packages, [(0, 2, 'stdlib')], units, 0, b'main', paths=paths)
        if outcome:
            code, unit, spelling = outcome
            coordinate = rows[unit][1] + units[unit][1].rindex(spelling)
            outcome = b'\0' + struct.pack('<II', code, coordinate)
        return name, packet, outcome

    yield trial('unselected-provider')
    yield trial('forwarded-name', wrapper=provider.replace(b'return_code', b'status'))
    body = b'{ProcessExit::exit_process(253);}'
    yield trial('provider-does-not-install', marker.replace(b'{42}', body), (2, 1, b'boundary trait ProcessExit'))
    direct = b'use omega\\language\\core\\ProcessExitPrimitive; machine main()->u8 {ProcessExitPrimitive(253);}'
    yield trial('primitive-does-not-install', direct, (2, 3, b'public boundary machine'))
    selected = b'use stdlib\\NativeProcessExit; machine main()->u8 {NativeProcessExit::exit_process(253);}'
    yield trial('implementation-does-not-install', selected, (2, 4, b'NativeProcessExit satisfies'))
    yield trial('member-is-owned', b'machine main()->u8 {exit_process(253);}', (1, 0, b'exit_process'))
    yield trial('wrong-forwarded-value', outcome=(2, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b'ProcessExitPrimitive(return_code)', b'ProcessExitPrimitive(7)'))
    yield trial('shadowed-forwarding-call', outcome=(2, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b'public NativeProcessExit satisfies', b'machine ProcessExitPrimitive(x:i32)->i32{x}\npublic NativeProcessExit satisfies'))
    yield trial('wrong-forwarded-place', outcome=(2, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b'ProcessExitPrimitive(return_code)', b'ProcessExitPrimitive(other)'))
    yield trial('wrong-primitive-requirement', outcome=(1, 3, b'ProcessExitPrimitive('),
                boundary=primitive.replace(b'satisfies ProcessExit::exit_process', b'satisfies ProcessExit::other'))
    yield trial('wrong-provider-result', outcome=(1, 4, b'exit_process(return_code'),
                wrapper=provider.replace(b') -> Never', b') -> u8'))
    yield trial('wrong-provider-owner', outcome=(2, 4, b'NativeProcessExit satisfies'),
                wrapper=provider.replace(b'public NativeProcessExit satisfies ProcessExit',
                                         b'boundary trait Other {} public NativeProcessExit satisfies Other'))
    # An ordinary local machine with the primitive's spelling has no physical identity.
    yield trial('ordinary-primitive-spelling',
                b'machine ProcessExitPrimitive(x:u8)->u8{x} machine main()->u8{ProcessExitPrimitive(42)}')


    relay = b'machine status(code:i32)->i32 {code}\n'
    compare = (relay + b'machine main()->u8 {let lo:i32=status(-2147483648i32); '
               b'let hi:i32=status(2147483647i32); let neg:i32=status(-1); '
               b'let zero:u32=0; transition lo < neg && neg < zero && zero < hi && '
               b'status(-0) == 0 && hi < 4294967295u32 && -1i32 != 255u8 '
               b'{true -> good(), _ -> bad()} machine good()->u8{42} machine bad()->u8{9}}')
    yield trial('signed-exit-status', compare)
    for name, value, spelling in [
        ('positive-status-overflow', b'2147483648i32', b'2147483648i32'),
        ('negative-status-overflow', b'-2147483649i32', b'-2147483649i32'),
        ('unsigned-status-suffix', b'-1u32', b'1u32'),
    ]:
        yield trial(name, relay + b'machine main()->u8{let bad:i32=status(' + value + b');42}',
                    (1, 0, spelling))

    # The construction writer passes its output byte to Console::write_byte as i32.
    yield trial('widen-output-byte', b'machine status(byte:u8)->i32 {byte as i32} '
                b'machine id(code:i32)->i32{code as i32} machine main()->u8 {'
                b'transition id(status(255)) == 255i32 && id(-1) < status(0) '
                b'{true -> yes(), _ -> no()} machine yes()->u8{42} machine no()->u8{9}}')
    yield trial('unsigned-to-signed-unproved', b'machine status(x:u32)->i32{x as i32} machine main()->u8{42}',
                (2, 0, b'x as i32'))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path, help='new directory for input, results and Alpha tapes')
    parser.add_argument("--case", action="append", help="run the named case (repeatable; default: all)")
    route = parser.add_mutually_exclusive_group()
    route.add_argument('--constructor', type=Path, help='compiled Epsilon construction executable')
    route.add_argument('--evaluator', type=Path, help='source-built Epsilon evaluator')
    add_invocation_budget(parser)
    args = parser.parse_args()
    work = args.output.resolve()
    work.mkdir(parents=True)
    if args.evaluator:
        evaluator = pack('bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources', work / 'evaluator.delta')
        compiler = pack('bootstrap/omega/compiler/omega_compiler.epsilon.sources', work / 'omega.epsilon')
        compiler += (ROOT / 'bootstrap/omega/construct_omega.epsilon').read_bytes()
    subjects = [*cases(), *terminal_cases(), *provider_cases()]
    unknown = set(args.case or ()) - {name for name, _, _ in subjects}
    if unknown:
        parser.error(f"unknown cases {sorted(unknown)}")
    for name, packet, expected in subjects:
        if args.case and name not in args.case:
            continue
        (work / f'{name}.input').write_bytes(packet)
        if args.evaluator:
            request = epsilon_request(evaluator, compiler, packet)
            (work / f'{name}.eereq').write_bytes(request)
            runner = args.evaluator
        elif args.constructor:
            runner, request = args.constructor, packet
        else:
            continue
        code, observed = execute(runner.resolve(), request, args.seconds)
        (work / f'{name}.output').write_bytes(observed)
        if code:
            raise SystemExit(f'{name}: host exit {code}')
        if expected is None or expected == 'trap':
            if not observed.startswith(bytes(5)) or len(observed) <= 5:
                raise SystemExit(f'{name}: expected compiled tape, got {observed[:32].hex()}')
            tape = work / f'{name}.tape'
            tape.write_bytes(observed[5:])
            program = work / name
            stamp(tape, program)
            code, output = execute(program, timeout=args.seconds)
            if expected == 'trap':
                if output or (code != -signal.SIGILL and code & 0xffffffff != 0xc000001d):
                    raise SystemExit(f'{name}: expected Alpha Trap, got {code} with {output!r}')
                print(f'{name}: Alpha Trap', flush=True)
            elif (code, output) == (42, b''):
                print(f'{name}: Alpha exit 42', flush=True)
            else:
                raise SystemExit(f'{name}: expected Alpha exit 42 and empty stdout')
        elif observed != expected:
            raise SystemExit(f'{name}: expected {expected.hex()}, got {observed[:32].hex()}')
        else:
            print(f'{name}: refusal at byte {int.from_bytes(observed[5:], "little")}', flush=True)


if __name__ == '__main__':
    main()
