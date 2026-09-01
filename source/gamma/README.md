# Gamma functional language

Gamma is the typed, pure functional rung above Beta. It supplies nominal
algebraic data, exhaustive pattern matching, checked signed integers, immutable
bytes, forward and mutual recursion, and proper tail calls. It is deliberately
small and exists to implement the Delta compiler.

[`LANGUAGE.md`](LANGUAGE.md) is normative. The canonical edge is:

```text
Beta evaluator + gamma_compiler.beta -> gamma_compiler_bytecode.tape
```

The Beta-written compiler source and its Alpha tape do not exist yet. The
former imperative Gamma compiler and the incomplete compiler previously written
in that language were deleted rather than preserved as alternate authority.
No old interpreter or differential oracle stands in for the direct edge.

The Gamma compiler may know only Beta, Gamma, Alpha tape, and the exact Delta
compiler application profile. It may not parse Delta source itself, invoke a
host translator, serialize an interpreter as output, or acquire general-purpose
runtime facilities.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative Gamma source and execution contract. | Replace only with a versioned contract and synchronized compiler/customer gates. |
