# Checked trees to lowered Psi

Start at [machine_lowering.rs](src/machine_lowering.rs). It selects the checked
machine, lowers its source closure, retains source custody and evidence, checks
the completed module, and attaches debug companions. Its result is unsealed Psi;
optimization and portable publication belong to the following stages.

[Machine selection](src/machine_lowering/machine_dispatch.rs) owns plan precedence. Each selected
producer supplies its source closure and a shared
[result contract](src/producer_result.rs): conformance publication, operand
proof validation or finalization, and checked-plan debug publication or omission.
The coordinator does not classify the source shape again. Producers import this
contract directly, without depending on dispatch.

An included source is not necessarily an exact emitted-machine owner.
The source-mapping choice is entry-only, scalar-closure order, or exact catalog
bindings. Catalog-producing paths retain explicit source-to-machine bindings;
other paths must not invent callee custody from table position. Scalar order can
join projection sources after checking table lengths, but it is not exact-owner
evidence. Reborrow publication keeps rejecting when the required exact owner is
unavailable.

The plan families beneath the coordinator are concept-owned modules beside
`lib.rs`; each root file names the work its directory owns:

- [`unit`](src/unit/mod.rs): attached, dynamic composed, structural-control and
  cleanup Unit machines, plus the runtime requirements they derive.
- [`returns`](src/returns/mod.rs): affine, boundary-scalar, payloadless and
  structural return machines, with the structural types they publish.
- [`scalar_graph`](src/scalar_graph/mod.rs): scalar-graph preparation,
  computation expansion, call closure and module assembly.
- [Expression preparation](src/expression_preparation/prepare_expression.rs):
  shared source-bound expressions, storage bindings, qualifications and
  independent source replay consumed by Unit, return and scalar producers.
- [`emission`](src/emission/mod.rs): operation and store emission shared by Unit
  and scalar bodies, with the call-operand source custody it replays.
- [`retention`](src/retention/mod.rs): checked custody installed on the assembled
  module: reach and conformance applications, reborrow handoffs, retained
  borrows, suspension plans and placed view inputs.
- [`proofs`](src/proofs/mod.rs): propositions, contracts, certificates and
  evidence artifacts.

[`lowering_error`](src/lowering_error.rs) and
[`terminal_identities`](src/terminal_identities.rs) carry the failure and
identity vocabulary every producer shares; [`debug_map`](src/machine_lowering/debug_map.rs)
presents the Terminal debug companion.

The subordinate owners follow the work:

- [Unit closures](src/unit/attached_unit.rs) assemble calls and storage.
- [Scalar closures](src/scalar_graph/scalar_call_closure.rs) assemble scalar graphs.
- [Scalar graph preparation](src/scalar_graph/scalar_graph_lowering.rs) produces
  [prepared states](src/scalar_graph/scalar_graph_lowering/prepared_graph.rs):
  ordered bindings, structural effects, transfers, and their contract.
- [Operation emission](src/emission/operation_emission.rs) dispatches a scalar
  binding to expression, store, selected comparison, or call emission.
- [Operation proofs](src/proofs/operation_proofs.rs) reconstructs obligations
  from the completed module and fills missing evidence in deterministic order.
- [Evidence lowering](src/proofs/evidence_lowering.rs) retains proof subjects.
- [Source custody](src/emission/call_source_custody.rs) preserves call identities.
- [Tests](src/tests.rs) exercise completed artifacts and invalid custody.

Within shared Unit assembly, [admission](src/unit/attached_unit/admission.rs)
retains the ordered body roster and delegates exact
[operation custody](src/unit/attached_unit/admission/operations.rs).
[Signatures](src/unit/attached_unit/signatures.rs) allocates all formals
before deriving requirements, retaining structural/scalar parameters, authored
predicate positions and machine-local claims together. Ordinary and composed
calls borrow the same records; emission consumes admitted bodies in roster order.
Neither step allocates a second module or changes the shared identity namespace.

[Composed callees](src/unit/attached_unit/composed_control/callable.rs)
borrow the closure's type, domain, service and boundary tables; temporary places
and identity counters remain local to emission. Standalone composed roots own
the same tables for publication. Byte-literal stores may allocate their generated
carrier only in an owned table; shared callees must find the prepared carrier.
Emission does not clone a shared type table and compare it after the fact.

Within emission, [calls](src/emission/operation_emission/calls.rs) owns argument
staging and call-requirement allocation;
[expressions](src/emission/operation_emission/expressions.rs) owns scalar leaves,
with [Boolean](src/emission/operation_emission/boolean.rs) and
[integer](src/emission/operation_emission/integer.rs) operations beside it.
The invocation-owned [buffer](src/emission/operation_emission/buffer.rs)
retains operation identities and source-occurrence companions. Short-circuit
expressions become blocks in [Boolean control](src/emission/boolean_control.rs);
they are not eagerly evaluated by the leaf emitter.

Emission also owns [primitive scalar types](src/emission/scalar_types.rs),
[store destinations](src/emission/store_destination.rs),
[selected-comparison metadata](src/emission/selected_comparison.rs), and
[lowered-expression validation](src/emission/expression_validation.rs).
Unit, return and scalar-graph producers consume these shared operations directly.
Graph cycle checks and branch staging remain in scalar-graph preparation; local
place allocation stays there too.

[Expression preparation](src/expression_preparation/prepare_expression.rs)
joins the checked expression to its authored role, prepares its scalar or Boolean
form, and independently replays source custody before returning it. Its
[bindings](src/expression_preparation/bindings/mod.rs) resolve current
storage and structural paths. [Source replay](src/expression_preparation/source_custody/mod.rs)
owns exact call and borrow occurrences, constructor operands, comparisons,
record observations and structural ownership. These operations use shared
[checked-graph queries](src/expression_preparation/computation_graph.rs), not
the producer's block expansion. [Qualifications](src/expression_preparation/qualifications.rs)
prepare the selected closure's declaration namespace. No preparation module
imports a machine producer; producers consume the shared operations directly.

Working plans live with these operations, not in the pipeline root's namespace.
Content result records remain with [content lowering](src/proofs/content_conservation.rs).
Their existing public re-exports are unchanged. Proof completion runs only after
the complete call/storage closure is assembled; it is not a side effect of
emitting one expression.

Shared scalar cleanup collects runtime inputs through
[ordinary operation traversal](src/scalar_graph/shared_runtime_parameters.rs),
rather than a catalogue of arithmetic compositions. This grants no arithmetic
authority: emission retains each operation, proof completion discharges every
canonical pre-result obligation, and independent verification checks source
custody and the completed graph. Unsupported storage or cleanup relationships
remain separate admission limits.
