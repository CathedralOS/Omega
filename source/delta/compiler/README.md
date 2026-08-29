# Delta compiler owner

The canonical compiler owned here accepts Delta, is implemented in Gamma, and
emits platform-independent Alpha tape:

```text
delta_compiler.gamma → delta_compiler_bytecode.tape
```

That implementation does not yet exist.

## Current misplaced implementation

`main.delta` is not a Gamma-written Delta compiler. It is Delta-written compiler
work intended to accept the source used for the first Omega build. Under D6 and
D11 that role is the first Omega compiler implementation source closure `D`, so
it belongs under `source/omega/` once its accepted language and full-Omega
contract are reconciled.

The superseded Beta Delta-to-Gamma route and Darwin-native publication tree are
deleted rather than retained as an alternate compiler architecture.

## Required replacement

- author `delta_compiler.gamma` against the independent Delta contract;
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
