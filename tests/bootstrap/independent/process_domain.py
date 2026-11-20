#!/usr/bin/env python3
"""Exercise a standalone Alpha process assigned to one exact Build deployment."""
import argparse
import runpy
from pathlib import Path

from service_reach import (ROOT, add_invocation_budget, epsilon_request, execute,
                           pack, rooted_application_cases, stamp)


def epsilon_literal(data):
    return b'"' + b''.join(bytes([byte]) if 32 <= byte < 127 and byte not in (34, 92)
                           else f'\\x{byte:02x}'.encode() for byte in data) + b'"'


def embedding(library, deployment, roots=b''):
    # The immutable argument belongs to the embedding. Guest input has its own
    # transport and cannot select the argument or modify the entry source.
    entry = (ROOT / 'bootstrap/omega/construct_omega.epsilon').read_bytes()
    hook = b'self.construct_standalone("");'
    if entry.count(hook) != 1:
        raise ValueError('standalone embedding hook changed')
    declaration = (b'self.construct_deployment(' + epsilon_literal(deployment) + b', ' + epsilon_literal(roots) + b');'
                   if roots else b'self.construct_standalone(' + epsilon_literal(deployment) + b');')
    return library + entry.replace(hook, declaration)


DEPLOYMENTS = {
    'selected': ('build-selection-grants-no-domain', True),
    'unselected': ('build-missing-provider-no-domain', False),
    'called-helper': ('build-called-helper', True),
    'hosted-helper': ('build-hosted-helper', False),
    'unreached-helper': ('build-unreached-helper', False),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--deployment', choices=DEPLOYMENTS, default='selected')
    route = parser.add_mutually_exclusive_group()
    route.add_argument('--constructor', type=Path, help='native constructor built from the generated embedding')
    route.add_argument('--evaluator', type=Path, help='run the generated embedding under an Epsilon evaluator')
    add_invocation_budget(parser)
    args = parser.parse_args()
    work = args.output.resolve()
    work.mkdir(parents=True, exist_ok=True)
    cases = {name: (wire, outcome) for name, wire, outcome in rooted_application_cases()}
    case, exits = DEPLOYMENTS[args.deployment]
    approved, refusal = cases[case]
    other = cases['build-entry-without-selection'][0]
    library = pack('bootstrap/omega/compiler/omega_compiler.epsilon.sources', work / 'omega.epsilon')
    driver = embedding(library, approved)
    (work / 'constructor.epsilon').write_bytes(driver)
    (work / 'deployment.input').write_bytes(approved)
    (work / 'other.input').write_bytes(other)
    if not (args.constructor or args.evaluator):
        return
    if args.evaluator:
        evaluator = pack('bootstrap/epsilon/interpreter/epsilon_compiler.delta.sources', work / 'evaluator.delta')
    for name, wire in [('deployment', approved), ('other', other)]:
        request = epsilon_request(evaluator, driver, wire) if args.evaluator else wire
        runner = args.evaluator or args.constructor
        code, observed = execute(runner.resolve(), request, args.seconds)
        (work / f'{name}.out').write_bytes(observed)
        if code:
            raise SystemExit(f'{name}: constructor exited {code}')
        if name == 'other' or not exits:
            expected = bytes([0, 3, 0, 0, 0]) if name == 'other' else refusal
            if observed != expected:
                raise SystemExit(f'{name}: expected {expected.hex()}, got {observed[:32].hex()}')
            print(f'{name}: refused, no tape', flush=True)
            continue
        if not observed.startswith(bytes(5)) or len(observed) == 5:
            raise SystemExit(f'{name}: expected tape, got {observed[:32].hex()}')
        tape = work / 'exit.tape'
        tape.write_bytes(observed[5:])
        seed = work / 'exit'
        stamp(tape, seed)
        observe = runpy.run_path(str(ROOT / 'tests/bootstrap/interpreted/alpha/reference/alpha_ref.py'))['observe']
        if observe(observed[5:], b'') != (0xfffffeff, b'') or execute(seed, timeout=args.seconds) != (255, b''):
            raise SystemExit('expected Alpha status -257 and Linux exit 255, both with empty output')
        print('deployment: Alpha HALT 0xfffffeff, Linux exit 255', flush=True)


if __name__ == '__main__':
    main()
