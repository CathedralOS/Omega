# Checked trees to lowered Psi

Start at [psi_lowering.rs](src/psi_lowering.rs). It selects the checked machine,
lowers its source closure, retains source custody and evidence, checks the
completed module, and attaches debug companions. Its result is unsealed Psi;
optimization and portable publication belong to the following stages.

[Machine selection](src/psi_lowering/machine_dispatch.rs) owns plan precedence.
Each selected producer supplies its source closure and the work still required
before returning: conformance publication, operand proof completion, and debug
eligibility. The coordinator does not classify the source shape again.

An included source is not necessarily an exact emitted-machine owner.
Catalog-producing paths retain explicit source-to-machine bindings; other paths
must not invent callee custody from table position. Reborrow publication keeps
rejecting when the required exact owner is unavailable.

The subordinate owners follow the work:

- [Unit closures](src/psi_lowering/attached_unit.rs) assemble calls and storage.
- [Scalar closures](src/psi_lowering/scalar_call_closure.rs) assemble scalar graphs.
- [Scalar graph preparation](src/psi_lowering/scalar_graph_lowering.rs) produces
  [prepared states](src/psi_lowering/scalar_graph_lowering/prepared_graph.rs):
  ordered bindings, structural effects, transfers, and their contract.
- [Operation emission](src/psi_lowering/operation_emission.rs) dispatches a scalar
  binding to expression, store, selected comparison, or call emission.
- [Operation proofs](src/psi_lowering/operation_proofs.rs) reconstructs obligations
  from the completed module and fills missing evidence in deterministic order.
- [Evidence lowering](src/psi_lowering/evidence_lowering.rs) retains proof subjects.
- [Source custody](src/psi_lowering/call_source_custody.rs) preserves call identities.
- [Tests](src/psi_lowering/tests.rs) exercise completed artifacts and invalid custody.

Within shared Unit assembly, [admission](src/psi_lowering/attached_unit/admission.rs)
retains the ordered body roster and delegates exact
[operation custody](src/psi_lowering/attached_unit/admission/operations.rs).
[Signatures](src/psi_lowering/attached_unit/signatures.rs) allocates all formals
before deriving requirements, retaining structural/scalar parameters, authored
predicate positions and machine-local claims together. Ordinary and composed
calls borrow the same records; emission consumes admitted bodies in roster order.
Neither step allocates a second module or changes the shared identity namespace.

[Composed callees](src/psi_lowering/attached_unit/composed_control/callable.rs)
borrow the closure's type, domain, service and boundary tables; temporary places
and identity counters remain local to emission. Standalone composed roots own
the same tables for publication. Byte-literal stores may allocate their generated
carrier only in an owned table; shared callees must find the prepared carrier.
Emission does not clone a shared type table and compare it after the fact.

Within emission, [calls](src/psi_lowering/operation_emission/calls.rs) owns argument
staging and call-requirement allocation;
[expressions](src/psi_lowering/operation_emission/expressions.rs) owns scalar leaves,
with [Boolean](src/psi_lowering/operation_emission/boolean.rs) and
[integer](src/psi_lowering/operation_emission/integer.rs) operations beside it.
The invocation-owned [buffer](src/psi_lowering/operation_emission/buffer.rs)
retains operation identities and source-occurrence companions. Short-circuit
expressions become blocks in [Boolean control](src/psi_lowering/boolean_control.rs);
they are not eagerly evaluated by the leaf emitter.

Working plans live with these operations, not in the pipeline root's namespace.
Content result records remain with [content lowering](src/psi_lowering/content_conservation.rs).
Their existing public re-exports are unchanged. Proof completion runs only after
the complete call/storage closure is assembled; it is not a side effect of
emitting one expression.

Shared scalar cleanup collects runtime inputs through
[ordinary operation traversal](src/psi_lowering/shared_runtime_parameters.rs),
rather than a catalogue of arithmetic compositions. This grants no arithmetic
authority: emission retains each operation, proof completion discharges every
canonical pre-result obligation, and independent verification checks source
custody and the completed graph. Unsupported storage or cleanup relationships
remain separate admission limits.
