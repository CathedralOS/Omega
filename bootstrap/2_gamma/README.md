# Gamma

Gamma is the typed scalar/effect functional bootstrap language previously used
experimentally under the name Delta0. Its selected implementation is a direct
Beta evaluator; there is no selected Gamma compiler or concatenative machine.

```text
Beta compiler
  -> gamma_evaluator.beta
  -> gamma_evaluator_bytecode.tape
  -> Gamma-authored source transformers
  -> richer Gamma or Delta source
```

The first executable customer is the test-owned 85-line `const` augmenter under
`tests/gamma/self-augmentation-experiment/`. The direct evaluator executes that
source, produces an exact richer-language receipt, and evaluates the expanded
program to byte 42.

The selected evaluator is complete for the current Gamma contract. Proper-tail
execution, whole-program static validation, provenance-tagged immutable pairs,
profile-owned arithmetic traps, bounded output, and exact resource outcomes are
implemented. Beta-root audit and the
[Gamma-written derivation checker](../proofs/README.md) are separate chain
obligations.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative source contract for typed scalar/effect Gamma. | Replace only with an explicit rung decision. |
| `EVALUATOR_PROFILE.md` | Request, observation, and private resource profile. | Replace atomically with the evaluator and gates. |
| `COMPOSED_ARTIFACT.md` | Exact evaluator-tape plus Gamma-source executable identity and atomic publication rule. | Replace only with an equally explicit executable composition. |
| `gamma_evaluator.beta` | Readable immediate-prior-rung implementation. | Replace atomically with a checked smaller or more complete Beta implementation. |
| `gamma_evaluator_bytecode.tape` | Platform-independent executable derived from the Beta source. | Regenerate atomically when the evaluator source changes. |

## Gamma evaluator

`gamma_evaluator.beta` is the selected immediate-prior-rung implementation of
typed scalar/effect Gamma. The trusted Beta compiler assembles it directly into
`gamma_evaluator_bytecode.tape`; no host compiler or retired concatenative
language participates.

The evaluator censuses function declarations and evaluates reached expressions
from source. It retains no AST and emits no lower-language source. Its request,
observation, memory partition, and current gaps are fixed by
[`EVALUATOR_PROFILE.md`](EVALUATOR_PROFILE.md).

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

The [Delta-generated census gate](../../tests/delta/generated-function-census/README.md)
retains the source that compiled successfully but could not execute under the
former census, and executes a receipt at Delta's full authored-function allowance.
