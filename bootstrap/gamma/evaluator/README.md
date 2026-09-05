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
Its 32,768 bytes fit beside the rows inside the existing function partition;
the 4,096-function preflight, duplicate precedence, `main` pointer, and
first-declaration application marker are unchanged. The index adds neither an
AST nor a Gamma operation or resource capacity. The addressed Beta source and
selected Beta compiler are the only reconstruction route; no host label
resolver participates.

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `gamma_evaluator.beta` | Readable Beta implementation. | Replace atomically with a checked smaller or more complete evaluator. |
| `gamma_evaluator_bytecode.tape` | Platform-independent Alpha artifact derived from the Beta source. | Regenerate when the source changes. |
