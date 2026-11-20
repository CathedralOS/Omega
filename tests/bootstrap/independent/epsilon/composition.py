"""Materialize the Delta sources declared by the Epsilon compiler."""
from argparse import ArgumentParser
from hashlib import sha256
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
EPSILON = ROOT / 'bootstrap/epsilon/interpreter'
COMPILER = ROOT / 'bootstrap/epsilon/compiler'


def interpreter_members(names, packed=None):
    members = {}
    offset = 0
    manifest = EPSILON / 'epsilon_compiler.delta.sources'
    for row in manifest.read_text().splitlines()[1:]:
        _, _, size, digest, name = row.split()
        path = "../interpreter/" + name
        if packed is None and path not in names:
            continue
        end = offset + int(size)
        source = (EPSILON / name).read_bytes() if packed is None else packed[offset:end]
        if len(source) != int(size) or sha256(source).hexdigest() != digest:
            raise ValueError(f'evaluator member differs from its manifest: {name}')
        if path in names:
            members[path] = source
        offset = end
    if packed is not None and len(packed) != offset:
        raise ValueError('packed evaluator length differs from its manifest')
    return members


def compose(parts, *, packed=None, generated=None, driver=None):
    names = parts.read_text().splitlines()
    members = interpreter_members(set(names), packed)
    chunks = []
    for name in names:
        if not name:
            chunks.append(b'\n')
        elif name == '@module':
            chunks.append(generated)
        elif name == '@entry':
            if driver is not None:
                chunks.append(driver)
                break
        elif name.startswith('../interpreter/'):
            chunks.append(members[name])
        else:
            chunks.append((COMPILER / name).read_bytes())
    return b''.join(chunks)


def compiler_source(packed_evaluator=None):
    return compose(COMPILER / 'compiler.parts', packed=packed_evaluator)


def program_source(generated, driver=None, runtime='pure'):
    if runtime not in ('pure', 'storage'):
        raise ValueError(f'unknown compiler runtime {runtime!r}')
    return compose(COMPILER / 'runtime' / f'{runtime}.parts', generated=generated, driver=driver)


def runtime_source(runtime='pure'):
    return program_source(b'', b'', runtime)


class CompileRefusal(ValueError):
    """The Epsilon compiler explicitly rejected the source or identified an unsupported construct."""


def decode_module(observation):
    for prefix, runtime in ((b'compiled\n', 'pure'), (b'compiled-storage\n', 'storage')):
        if observation.startswith(prefix):
            return observation[len(prefix):], runtime
    if observation.startswith((b'rejected ', b'unsupported ')):
        raise CompileRefusal(f'compiler refused source: {observation[:80]!r}')
    raise ValueError(f'unexpected compiler output: {observation[:80]!r}')


def linked_program(observation):
    module, runtime = decode_module(observation)
    return program_source(module, runtime=runtime)


def main():
    parser = ArgumentParser(description=__doc__)
    command = parser.add_subparsers(dest='command', required=True)
    command.add_parser('compiler', help='write the Delta compiler source to stdout')
    command.add_parser('program', help='read a compiler observation from stdin and write runnable Delta')
    args = parser.parse_args()
    if args.command == 'compiler':
        source = compiler_source()
    else:
        try:
            source = linked_program(sys.stdin.buffer.read())
        except ValueError as error:
            parser.exit(1, str(error) + '\n')
    sys.stdout.buffer.write(source)


if __name__ == '__main__':
    main()
