# Downgraded Concatenative Gamma

This directory preserves the former Gamma language implementation while the
typed scalar/effect Gamma bootstrap is validated. It is not a selected source
owner or permanent rung. Paths below are relative to this retained snapshot.

# Gamma compiler machine


```text
audited Alpha VM + admitted Beta compiler tape
  + gamma_evaluator.beta -> gamma_evaluator_bytecode.tape
  + gamma_compiler.gamma -> canonical gamma_compiler.beta
    -> Beta compiler -> gamma_compiler_bytecode.tape
  + Gamma source -> canonical Beta -> Alpha tape
```

The admitted evaluator artifact, complete conformance closure, and full
Gamma-written Delta elaborator are currently open. The executable evaluator core lives at
[`evaluator/gamma_evaluator.beta`](evaluator/gamma_evaluator.beta), with its
focused gate at
[`../../tests/gamma/evaluator-slice.sh`](../../tests/gamma/evaluator-slice.sh).
No host interpreter stands in for either edge.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Exact Gamma syntax and evaluation relation. | Replace only with a versioned contract and synchronized evaluator and customer gates. |
| `EVALUATOR_PROFILE.md` | Exact request, observation, resource, publication, and private-representation contract for the first Beta-authored evaluator. | Replace only with a versioned profile and synchronized evaluator and gates. |
| `evaluator/` | Beta source for the Gamma evaluator and its eventual Alpha tape. | Replace only atomically with the admitted Beta-to-Gamma edge. |
| `compiler/` | Former Gamma-to-Beta compiler, retained Beta self-receipt, and native tape. | Delete after its comparison evidence is superseded. |
