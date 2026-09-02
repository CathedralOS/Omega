# Gamma compiler machine

Gamma is the bounded concatenative rung above Beta. It provides named words,
an explicit value stack, fixed cells, sealed input, append-only output, ordinary
calls, and tail CFG transfers for writing the Delta compiler and small bootstrap
tools.

Its evaluator is written in trusted Beta and compiled to Alpha tape by the
admitted Beta compiler. Its obligation is the exact Gamma evaluation relation
in [`LANGUAGE.md`](LANGUAGE.md) under the implementation contract in
[`EVALUATOR_PROFILE.md`](EVALUATOR_PROFILE.md).

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
| `compiler/` | Selected Gamma-to-Beta compiler, retained Beta self-receipt, and native tape. | Replace only atomically with its reconstruction and conformance gates. |
