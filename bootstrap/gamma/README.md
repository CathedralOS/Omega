# Gamma

Gamma is the typed scalar/effect functional bootstrap language previously used
experimentally under the name Delta0. Its selected implementation is a direct
Beta evaluator; there is no selected Gamma compiler or concatenative machine.

```text
Beta compiler
  -> evaluator/gamma_evaluator.beta
  -> gamma_evaluator_bytecode.tape
  -> Gamma-authored source transformers
  -> richer Gamma or Delta source
```

The first executable customer is the test-owned 85-line `const` augmenter under
`tests/gamma/self-augmentation-experiment/`. The direct evaluator executes that
source, produces an exact richer-language receipt, and evaluates the expanded
program to byte 42.

The former concatenative Gamma evaluator/compiler is preserved under
[`bootstrap/concatenative/`](bootstrap/concatenative/). It is bootstrap
comparison evidence, not the selected Gamma language.

The selected evaluator is complete for the current Gamma contract. Proper-tail
execution, whole-program static validation, provenance-tagged immutable pairs,
profile-owned arithmetic traps, bounded output, and exact resource outcomes are
implemented. Beta-root audit and the Gamma derivation checker are separate
chain obligations.

## Retention inventory

| Retained child | Direct role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Normative source contract for typed scalar/effect Gamma. | Replace only with an explicit rung decision. |
| `EVALUATOR_PROFILE.md` | Request, observation, and private resource profile. | Replace atomically with the evaluator and gates. |
| `COMPOSED_ARTIFACT.md` | Exact evaluator-tape plus Gamma-source executable identity and atomic publication rule. | Replace only with an equally explicit executable composition. |
| `evaluator/gamma_evaluator.beta` | Readable immediate-prior-rung implementation. | Replace with a smaller or more complete Beta implementation. |
| `evaluator/gamma_evaluator_bytecode.tape` | Platform-independent executable derived from the Beta source. | Regenerate atomically when the evaluator source changes. |
| `derivation_checker/` | Source-owned outer request admission for the planned Gamma derivation checker; no complete checker or proof-accepting entry yet. | Replace only with a complete checker that retains the same bounded input custody. |
| `bootstrap/concatenative/` | Downgraded former Gamma implementation and receipts. | Delete after the new Gamma-to-Delta bootstrap edge supersedes its remaining evidence. |
