# Delta compiler owner

The canonical compiler owned here accepts Delta, is implemented in Gamma, and
emits platform-independent Alpha tape:

```text
delta_compiler.gamma → delta_compiler_bytecode.tape
```

The source now exists as an incomplete implementation. Its retained milestones
own the exact D17 rejection/outcome sums, complete lexical phase, native syntax
representation, and transient syntax-token scanner. It validates every source
byte before scanning all tokens and literals, returns the exact lexical reason
and packed offset, and retains no host-generated token ledger. Syntax nodes are
recursive Gamma values with exact source spans rather than byte-rope records or
numeric arena references. The scanner rescans one token at a time only after
the complete lexical phase succeeds. Its token start, code, end, and literal
value are immediate `Int` results: lookahead may repeat bounded scanning work,
but it authors no transient token objects into the generated program's fixed
immutable heap. This foundation type-checks through the current Gamma frontend
gate.

It deliberately has no `main`, emitted placeholder, or canonical tape. The
native values can represent every D17 grammar form, but parsing, whole-closure
collection, type/control checking, direct Alpha lowering, and final publication
remain implementation gaps. The existing source is therefore not yet a
compiler edge and no validation may describe it as one.

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

This implementation owner is retained because its exact path is part of the
canonical lattice contract. Delete any child subtree that does not reconstruct,
implement, or test
`delta_compiler.gamma → delta_compiler_bytecode.tape`; replace the owner only
atomically with a changed, explicitly ruled lattice topology.
