# Gamma compiler owner

The canonical compiler owned here accepts Gamma, is implemented in Beta, and
emits platform-independent Alpha tape:

```text
gamma_compiler.beta → gamma_compiler_bytecode.tape
```

The source and artifact do not yet exist. Their language-level contract is
design-blocked on the typed executable Gamma entry, byte-stream adapter,
outcome, and resource-profile ruling in `OWNER_QUESTIONS.md`; no placeholder
compiler or interpreter-shaped artifact is accepted in the meantime.

`../interp.beta` and `../typeck.beta` are retained at the Gamma language owner
as bounded semantic oracles and candidate implementation components. The
eventual compiler may reorganize or absorb them, but must type-check Gamma and
emit Alpha tape directly rather than publishing an interpreter plus source AST.
Delete either component when the compiler subsumes its unique failure
detection, or if it cannot be economically adapted to the selected contract.

Any future validation placed here must reconstruct the exact
Beta-source-to-Alpha-tape edge for `gamma_compiler.beta`. Generic evidence,
external interpreter execution, and host-side source lowering do not belong in
this owner.

The implementation order is tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).
