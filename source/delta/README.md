# Delta functional language

Delta is the typed, pure functional rung above Gamma. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small and exists to implement the Epsilon compiler.

[`LANGUAGE.md`](LANGUAGE.md) is normative. The canonical edge is:

```text
delta_compiler.gamma -> canonical Gamma -> canonical Beta
	-> delta_compiler_bytecode.tape
```

The selected Gamma-written compiler source now exists as an in-progress scalar
slice under `compiler/`; its composed scalar gate passes, but the full-language
compiler and admitted Alpha tape remain open. Noncanonical comparison evidence
is owned by `tests/delta/`, outside the source spine. No old interpreter or
differential oracle stands in for the selected edge.

The Delta compiler may know only Gamma, Delta, and the exact Epsilon compiler
application profile. It may not encode Alpha, parse Epsilon source itself,
invoke a host translator, serialize an interpreter as output, or acquire
general-purpose runtime facilities.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Delta source and execution contract. | Replace only with a versioned contract and synchronized compiler/customer gates. |
| `compiler/` | Selected in-progress Gamma-written compiler source and its missing-edge requirements. | Admit artifacts only after the source implements the complete Delta contract. |
