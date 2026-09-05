# Gamma evaluator

`gamma_evaluator.beta` is the selected immediate-prior-rung implementation of
typed scalar/effect Gamma. The trusted Beta compiler assembles it directly into
`gamma_evaluator_bytecode.tape`; no host compiler or retired concatenative
language participates.

The evaluator censuses function declarations and evaluates reached expressions
from source. It retains no AST and emits no lower-language source. Its request,
observation, memory partition, and current gaps are fixed by
[`../EVALUATOR_PROFILE.md`](../EVALUATOR_PROFILE.md).

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `gamma_evaluator.beta` | Readable Beta implementation. | Replace atomically with a checked smaller or more complete evaluator. |
| `gamma_evaluator_bytecode.tape` | Platform-independent Alpha artifact derived from the Beta source. | Regenerate when the source changes. |
