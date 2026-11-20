#!/usr/bin/env python3
"""Check service identity and call ceilings against actual bundled core declarations."""
import argparse
from pathlib import Path
import runpy
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


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path, help='new directory for input, results and Alpha tapes')
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
    for name, packet, expected in cases():
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
        if expected is None:
            if not observed.startswith(bytes(5)) or len(observed) <= 5:
                raise SystemExit(f'{name}: expected compiled tape, got {observed[:32].hex()}')
            tape = work / f'{name}.tape'
            tape.write_bytes(observed[5:])
            program = work / name
            stamp(tape, program)
            if execute(program, timeout=args.seconds) != (42, b''):
                raise SystemExit(f'{name}: expected Alpha exit 42 and empty stdout')
            print(f'{name}: Alpha exit 42', flush=True)
        elif observed != expected:
            raise SystemExit(f'{name}: expected {expected.hex()}, got {observed[:32].hex()}')
        else:
            print(f'{name}: refusal at byte {int.from_bytes(observed[5:], "little")}', flush=True)


if __name__ == '__main__':
    main()
