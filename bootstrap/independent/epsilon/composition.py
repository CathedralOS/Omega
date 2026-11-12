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


def runtime_declarations():
    return (b''.join((EPSILON / name).read_bytes() for name in RUNTIME_MODULES[:2])
            + (COMPILER / 'runtime/values.delta').read_bytes())


def runtime_implementation():
    return ((EPSILON / RUNTIME_MODULES[2]).read_bytes()
            + (COMPILER / 'runtime/scalars.delta').read_bytes() + b'\n')


def runtime_source():
    return runtime_declarations() + runtime_implementation()


def program_source(generated, driver):
    # The owned module emits its nominal declarations before its functions.
    # Forward function references allow runtime code after that complete module.
    # Composition only concatenates complete owners; it parses no generated text.
    return runtime_declarations() + generated + runtime_implementation() + driver
