# Delta functional language

Delta is the typed, pure functional rung above Gamma. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small and exists to implement the Epsilon compiler.

[`LANGUAGE.md`](LANGUAGE.md) is normative. The canonical edge is:

```text
Gamma evaluator + delta_compiler.gamma -> delta_compiler_bytecode.tape
```

The Gamma-written compiler source and its Alpha tape do not exist yet. The
former imperative Gamma compiler and the incomplete compiler previously written
in that language were deleted rather than preserved as alternate authority.
No old interpreter or differential oracle stands in for the direct edge.

The Delta compiler may know only Gamma, Delta, Alpha tape, and the exact Epsilon
compiler application profile. It may not parse Epsilon source itself, invoke a
host translator, serialize an interpreter as output, or acquire general-purpose
runtime facilities.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Delta source and execution contract. | Replace only with a versioned contract and synchronized compiler/customer gates. |
