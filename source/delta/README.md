# Delta functional language

Delta is the typed, pure functional rung above Gamma. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small and exists to implement the Epsilon compiler.

[`LANGUAGE.md`](LANGUAGE.md) is normative. The canonical edge is:

```text
Gamma-authored staged Delta implementation -> canonical Gamma/Delta receipts
	-> complete Delta edge
```

The selected compiler now begins under `compiler/` with a Gamma-authored stage
for finite arbitrary-field data, including recursive nominal fields and
exhaustive matching through right-nested immutable pairs. The complete Delta
edge remains open. The former full compiler written in the retired concatenative Gamma
language is preserved under
`bootstrap/concatenative-compiler/`; it is expressiveness evidence, not the
selected edge. Current staged-bootstrap experiments remain under `tests/delta/`.

The Delta compiler may know only Gamma, Delta, and the exact Epsilon compiler
application profile. It may not encode Alpha, parse Epsilon source itself,
invoke a host translator, serialize an interpreter as output, or acquire
general-purpose runtime facilities.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Delta source and execution contract. | Replace only with a versioned contract and synchronized compiler/customer gates. |
| `compiler/` | Selected Gamma-authored staged compiler, currently covering arbitrary-field recursive ADTs and exhaustive matches through immutable Gamma pairs. | Replace only with a more complete immediately prior-rung implementation. |
| `bootstrap/concatenative-compiler/` | Downgraded full compiler from the former concatenative Gamma rung. | Delete after a Gamma-authored staged Delta edge supersedes its evidence. |
