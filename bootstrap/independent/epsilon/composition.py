"""Explicit source composition; predecessor Delta remains the semantic producer.

The target runtime is a closed leaf implementation from the common Epsilon
source authority. It contains neither a parser nor an AST evaluator/dispatcher.
No host extraction, transformation, or dead-code selection builds its text.
"""
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
EPSILON = ROOT / 'bootstrap/interpreted/4_epsilon'
COMPILER = Path(__file__).parent
COMPILER_MODULES = (
    'emission.delta', 'text.delta', 'compilation.delta', 'locals.delta',
    'binary.delta', 'expressions.delta', 'calls.delta', 'machines.delta',
    'control.delta', 'enums.delta', 'enum_values.delta', 'enum_control.delta',
    'storage_places.delta', 'storage_text.delta', 'storage_expressions.delta',
    'storage_calls.delta', 'storage_machines.delta', 'storage_control.delta',
    'storage_blocks.delta',
)
RUNTIME_MODULES = (
    'representations/binary_operators.delta',
    'representations/scalar_outcomes.delta',
    'execution/scalars/operators.delta',
)



def compiler_source(checked_epsilon_closure):
    return ((COMPILER / 'representations.delta').read_bytes()
            + checked_epsilon_closure + b'\n'
            + b'\n'.join((COMPILER / name).read_bytes() for name in COMPILER_MODULES)
            + b'\n' + (COMPILER / 'entry.delta').read_bytes())


STORAGE_MODULES = (
    'checking/catalog/literal_values.delta',
    'execution/storage/values.delta', 'execution/storage/fields.delta',
    'execution/storage/arrays.delta', 'execution/storage/children.delta',
    'execution/storage/places.delta', 'execution/storage/homes.delta',
    'execution/storage/arrays/lookup.delta', 'execution/storage/arrays/updates.delta',
    'execution/scalars/value_outcomes.delta', 'execution/console/input.delta',
    'execution/views/backing.delta',
    'execution/projections/field_values.delta',
    'execution/projections/index_values.delta', 'execution/views/slice_values.delta',
    'execution/storage/liveness.delta',
)


def runtime_declarations(runtime='pure'):
    if runtime == 'pure':
        names = RUNTIME_MODULES[:2]
    elif runtime == 'storage':
        manifest = EPSILON / 'epsilon_compiler.delta.sources'
        names = [line.split()[4] for line in manifest.read_text().splitlines()[1:]
                 if line.split()[4].startswith('representations/')]
    else:
        raise ValueError(f'unknown compiler runtime {runtime!r}')
    result = (b''.join((EPSILON / name).read_bytes() for name in names)
              + (COMPILER / 'runtime/values.delta').read_bytes())
    if runtime == 'storage':
        result += (COMPILER / 'runtime/storage_values.delta').read_bytes()
    return result


def runtime_implementation(runtime='pure'):
    result = ((EPSILON / RUNTIME_MODULES[2]).read_bytes()
              + (COMPILER / 'runtime/scalars.delta').read_bytes() + b'\n')
    if runtime == 'storage':
        result += b''.join((EPSILON / name).read_bytes() for name in STORAGE_MODULES)
        result += (COMPILER / 'runtime/storage.delta').read_bytes()
    elif runtime != 'pure':
        raise ValueError(f'unknown compiler runtime {runtime!r}')
    return result


def runtime_source(runtime='pure'):
    return runtime_declarations(runtime) + runtime_implementation(runtime)


def program_source(generated, driver, runtime='pure'):
    # The module owns nominal declarations before functions. Forward function
    # references allow complete runtime implementation owners after the module.
    return runtime_declarations(runtime) + generated + runtime_implementation(runtime) + driver


def decode_module(observation):
    for prefix, runtime in ((b'compiled\n', 'pure'), (b'compiled-storage\n', 'storage')):
        if observation.startswith(prefix):
            return observation[len(prefix):], runtime
    raise ValueError(f'compiler refused source: {observation[:80]!r}')
