# Psi semantics

The source checker, interpreter and native compiler must agree on the meaning of a [Psi program](../psi/psi.rs). The [shared judgments](src/lib.rs) read its operations, types and declarations to answer questions about calls, references, completion and storage.

- The [predicate evaluator](closed-predicates/closed_predicates.rs) evaluates a closed predicate on concrete operands. Its numeric rules come from the [language's numeric laws](../foundation/numerics/README.md).
- [Proof checking](proofs/README.md) checks explicit certificates and their premises.
- [Storage bounds](storage-bounds/) compose activation requirements. [Default integer layouts](default-integer-layouts/integer_defaults.rs) give standalone integers and record fields their conventional storage geometry.
- [Work accounting](work/) assigns costs to executed operations and taken edges.

[Execution inputs](../psi-execution-inputs/) check the concrete layouts, providers, memory and scalar observations supplied for an execution.
