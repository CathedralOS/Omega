# Gamma evaluator

`gamma_evaluator.beta` is the selected immediate-prior-rung implementation of
typed scalar/effect Gamma. The trusted Beta compiler assembles it directly into
`gamma_evaluator_bytecode.tape`; no host compiler or retired concatenative
language participates.

The evaluator censuses function declarations and evaluates reached expressions
from source. It retains no AST and emits no lower-language source. Its request,
observation, memory partition, and current gaps are fixed by
[`../EVALUATOR_PROFILE.md`](../EVALUATOR_PROFILE.md).

Function rows remain in authored order. A separate sorted row-pointer index
supports exact-name binary search for census, validation, and execution.
Its 16 MiB fit beside 2,097,152 rows in the private
`0x04000000..0x0a000000` function partition. The request's source extent bounds
completed declarations below that capacity; see the profile's byte-count
argument. This covers authored definitions and every generated helper without
an independent census refusal. Duplicate precedence, the `main` pointer, and
first-declaration application ownership are unchanged. The index adds neither
an AST nor a Gamma operation. The addressed Beta source and
selected Beta compiler are the only reconstruction route; no host label
resolver participates.

The [Delta-generated census gate](../../../tests/delta/generated-function-census/README.md)
retains the source that compiled successfully but could not execute under the
former census, and executes a receipt at Delta's full authored-function allowance.

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `gamma_evaluator.beta` | Readable Beta implementation. | Replace atomically with a checked smaller or more complete evaluator. |
| `gamma_evaluator_bytecode.tape` | Platform-independent Alpha artifact derived from the Beta source. | Regenerate when the source changes. |
