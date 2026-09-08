# Delta functional language

Delta is the typed, pure functional rung above Gamma. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small and exists to implement the Epsilon evaluator.

[`LANGUAGE.md`](LANGUAGE.md) is normative. The canonical edge is:

```text
Gamma-authored staged Delta implementation -> canonical Gamma/Delta receipts
	-> complete Delta edge
```

The selected compiler now begins under `compiler/` with a Gamma-authored stage
for finite arbitrary-field data, including recursive nominal fields and
exhaustive matching through right-nested immutable pairs. The complete Delta
edge remains open. [Selected validation](../../tests/delta/README.md) owns
staged compiler behavior and conformance controls; there is no alternate
concatenative compiler.

The Delta compiler may know only Gamma, Delta, and its exact application
profiles. It may not encode Alpha, parse Epsilon source itself,
invoke a host translator, serialize an interpreter as output, or acquire
general-purpose runtime facilities.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Delta source and execution contract. | Replace only with a versioned contract and synchronized compiler/customer gates. |
| `compiler/` | Selected Gamma-authored staged compiler, currently covering arbitrary-field recursive ADTs, exhaustive matches, checked arithmetic, and immutable length-bearing Bytes ropes through Gamma pairs. | Replace only with a more complete immediately prior-rung implementation. |
