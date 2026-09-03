# Delta compiler work

`delta_compiler.gamma` is the selected in-progress Gamma-written Delta compiler.
It emits canonical Gamma source; the selected Gamma compiler then emits Beta,
and Beta alone encodes Alpha. Its current executable slice covers `Int`, scalar
operators, lexical `let`, `if`, mutually visible functions, zero through
thirteen parameters, nested calls, and direct recursion.

Its composed scalar gate lives under
[`../../../tests/delta/compiler-slice/`](../../../tests/delta/compiler-slice/).
The admitted tape remains absent because the implementation still needs
algebraic data, exhaustive `match`, `Bytes`, complete checking, proper tail
calls, profiles, and exact resource outcomes before it implements
[`../LANGUAGE.md`](../LANGUAGE.md) and can compile Epsilon. Noncanonical direct
and state-machine comparisons remain test-owned under `tests/delta/`.
