# Psi semantics

The source checker, interpreter and native compiler must agree on the meaning of a [Psi program](../psi.rs). The [shared judgments](src/lib.rs) read its operations, types and declarations to answer questions about calls, references, completion and storage.

- The [predicate evaluator](closed-predicates/closed_predicates.rs) evaluates a closed predicate on concrete operands. Its numeric rules come from the [language's numeric laws](../../foundation/numerics/README.md).
- [Proof checking](proofs/README.md) checks explicit certificates and their premises.
- [Storage bounds](storage-bounds/) compose activation requirements. [Default integer layouts](default-integer-layouts/integer_defaults.rs) give standalone integers and record fields their conventional storage geometry.
- [Work accounting](work/) assigns costs to executed operations and taken edges.
- [Module analyses](src/module_analyses.rs) build a module's machine lookup, type index and descriptor rows once, so later closures over the same module reuse them. A closure that reaches a dynamic call still gets rows adapted against that closure alone. The free `terminal_graph*` functions build a bundle for one closure and remain until their callers move to the shared one.

[Execution inputs](../execution-inputs/) check the concrete layouts, providers, memory and scalar observations supplied for an execution.
