# Gamma compiler owner

The canonical compiler owned here accepts Gamma, is implemented in Beta, and
emits platform-independent Alpha tape:

```text
gamma_compiler.beta → gamma_compiler_bytecode.tape
```

The source and artifact do not yet exist. Their language-level contract is
settled in [`../LANGUAGE.md`](../LANGUAGE.md); this owner is now an implementation
gap rather than a design-blocked placeholder. No interpreter-shaped artifact is
accepted in the meantime.

`../interp.beta` and `../typeck.beta` are retained at the Gamma language owner
as bounded semantic oracles and candidate implementation components. The
eventual compiler may reorganize or absorb them, but must type-check Gamma and
emit Alpha tape directly rather than publishing an interpreter plus source AST.
Delete either component when the compiler subsumes its unique failure
detection, or if it cannot be economically adapted to the contract.

The compiler uses a private arbitrary-arity frame ABI and preserves proper tail
calls. Its emitted compiler-application adapter alone supplies sealed input as
Gamma `Bytes`, invokes the typed `main`, and serializes exact success or the
accepted-language edge's failure frame. Fuel and private storage ceilings yield
outer resource outcomes; they never change Gamma meaning.

Any future validation placed here must reconstruct the exact
Beta-source-to-Alpha-tape edge for `gamma_compiler.beta`. Generic evidence,
external interpreter execution, and host-side source lowering do not belong in
this owner.

The implementation order is tracked in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Deletion condition

This currently empty implementation owner is retained because its exact path is
part of the canonical lattice contract. Delete any future child subtree that
does not reconstruct, implement, or test
`gamma_compiler.beta → gamma_compiler_bytecode.tape`; replace the owner only
atomically with a changed, explicitly ruled lattice topology.
