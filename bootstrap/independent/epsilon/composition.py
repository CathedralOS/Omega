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
    'control.delta',
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


def runtime_source():
    # Every member is a complete shared leaf; no Epsilon syntax declaration,
    # parsing/checking function, or AST dispatcher enters the generated program.
    declarations = b''.join((EPSILON / name).read_bytes()
                            for name in RUNTIME_MODULES[:2])
    values = (COMPILER / 'runtime/values.delta').read_bytes()
    operators = (EPSILON / RUNTIME_MODULES[2]).read_bytes()
    scalar_adapter = (COMPILER / 'runtime/scalars.delta').read_bytes()
    return declarations + values + operators + scalar_adapter + b'\n'
