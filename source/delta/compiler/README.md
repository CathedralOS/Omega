# Delta compiler owner

The canonical compiler owned here accepts Delta, is implemented in Gamma, and
emits platform-independent Alpha tape:

```text
delta_compiler.gamma → delta_compiler_bytecode.tape
```

That implementation does not yet exist. D17 fixes the language it must accept,
so this is now an implementation gap rather than a design-blocked surface.

The superseded Beta Delta-to-Gamma route, Darwin-native publication tree, and
restricted Delta-written native compiler prototype are deleted rather than
retained as alternate compiler architecture. The prototype implemented neither
this Gamma-written edge nor full Omega `D`; moving it would have preserved the
wrong identity, while adapting its monolithic restricted frontend and Darwin
backend was less economical than authoring the specified direct components.

## Required replacement

- author `delta_compiler.gamma` against D17 and
  [`../LANGUAGE.md`](../LANGUAGE.md);
- expose pure `main : Bytes -> DeltaCompileOutcome`, with only
  `Complete(Bytes)` and `Reject(DeltaRejectReason, Int)` authored outcomes;
- let the generated adapter own `DCOUT`, its explicit reason-code table, and
  outer `Incomplete`/`InternalFailure` outcomes;
- compile it with `gamma_compiler_bytecode.tape`;
- emit one exact Alpha tape without external older-rung semantic tools;
- reconstruct Gamma source and Alpha artifact semantics independently;
- check direct source-to-tape refinement and negative mutations; and
- keep any native execution as transparent Alpha-seed packaging or an optional
  checked general Alpha realization.

Any new validation placed here must reconstruct the Gamma-source-to-Alpha-tape
edge for `delta_compiler.gamma`. Generic custody, repeated-execution, or native
publication machinery does not belong here.

The active migration order lives in
[`TASKS_BOOTSTRAP.md`](../../../TASKS_BOOTSTRAP.md).

## Deletion condition

This currently empty implementation owner is retained because its exact path is
part of the canonical lattice contract. Delete any future child subtree that
does not reconstruct, implement, or test
`delta_compiler.gamma → delta_compiler_bytecode.tape`; replace the owner only
atomically with a changed, explicitly ruled lattice topology.
