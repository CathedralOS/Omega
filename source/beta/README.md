# Beta bootstrap calculus

Beta is the first language above Alpha. It is a strict, first-order,
S-expression functional calculus for writing the Gamma compiler and small
bootstrap tools such as the derivation checker.

The cold-start implementation will be one directly admitted Alpha evaluator
tape. That tape is part of the audited root: it is not justified by an
assembly-language rung or by self-hosting. Its obligation is the exact Beta
evaluation relation in [`LANGUAGE.md`](LANGUAGE.md) under the exact first
implementation contract in [`EVALUATOR_PROFILE.md`](EVALUATOR_PROFILE.md).

```text
audited Alpha VM + audited Beta evaluator tape
  + Beta source + sealed input
    -> returned Beta value
```

The canonical evaluator artifact and Beta-written Gamma compiler are currently
open. A non-canonical executable development slice lives at
[`../../tools/alpha/beta-evaluator/`](../../tools/alpha/beta-evaluator/) with
its focused gate at
[`../../tests/beta/evaluator-slice.sh`](../../tests/beta/evaluator-slice.sh).
No former Gamma artifact or host interpreter stands in for either edge.

## Retention inventory

| Retained child | Canonical role | Deletion condition |
| --- | --- | --- |
| `LANGUAGE.md` | Exact Beta syntax and evaluation relation. | Replace only with a versioned contract and synchronized evaluator and customer gates. |
| `EVALUATOR_PROFILE.md` | Exact request, observation, resource, publication, and private-representation contract for the first audited evaluator tape. | Replace only with a versioned profile and synchronized tape audit and gates. |
