# Checked trees to lowered Psi

Start at [machine_lowering.rs](src/machine_lowering.rs). It selects the checked
machine, lowers its source closure, retains source custody and evidence, checks
the completed module, and attaches debug companions. Its result is unsealed Psi;
optimization and portable publication belong to the following stages.

The checker's permission ledger is consumed as checked: lowering does not
re-validate the checker's own fresh output.

Boundary body candidates also arrive as checked operator facts. Publication
assigns portable declaration identities to their retained rows; it does not
search declarations for conformances or reconstruct specialization judgments.

Place types, destination-independent expression results and shell-free type
carriers come from `checked.facts.resolved_types`. Checking publishes the dense
tables after specialization and authored-selection finalization freeze the
arenas. Place and result remain separate: a borrowed place is not a by-value
result, and arithmetic cannot inherit an operand's refinement. Lowering reads
the exact generational handle's decision; absent types have no reconstruction
fallback. Stored facts and their read APIs both use zero handles for absence,
not optional handle wrappers. The source guards in `src/source_guards.rs`
protect this boundary.

[Machine selection](src/machine_lowering/machine_dispatch.rs) owns plan precedence. Each selected
producer supplies its source closure and a shared
[result contract](src/producer_result.rs): conformance publication, operand
proof validation or finalization, and checked-plan debug publication or omission.
The coordinator does not classify the source shape again. Producers import this
contract directly, without depending on dispatch.

An included source is not necessarily an exact emitted-machine owner.
The source-mapping choice is entry-only or exact catalog bindings.
Catalog-producing paths, including selected scalar roots, retain explicit
source-to-machine bindings; other paths must not invent callee custody from
table position. Reborrow publication keeps rejecting when the required exact
owner is unavailable.

The plan families beneath the coordinator are concept-owned modules beside
`lib.rs`; each root file names the work its directory owns:

- [`unit`](src/unit/mod.rs): attached, structural-control and cleanup Unit
  machines, plus the runtime requirements they derive. Every structural or
  scalar result a body returns lowers here; there is no separate return
  family.
- [`scalar_lowering`](src/scalar_lowering/mod.rs): the scalar pieces every
  Unit body embeds: computation expansion, contracts, scalar callee selection
  and prepared branch states.
- [Expression preparation](src/expression_preparation/prepare_expression.rs):
  shared source-bound expressions, storage bindings, qualifications and
  independent source replay consumed by Unit and scalar producers.
- [`emission`](src/emission/mod.rs): operation and store emission shared by Unit
  and scalar bodies, with the call-operand source custody it replays.
- [`retention`](src/retention/mod.rs): checked custody installed on the assembled
  module: reach and conformance applications, reborrow handoffs, retained
  borrows, suspension plans and placed view inputs.
- [`proofs`](src/proofs/mod.rs): propositions, contracts, certificates and
  evidence artifacts.

[`lowering_error`](src/lowering_error.rs) and
[`terminal_identities`](src/terminal_identities.rs) carry the failure and
identity vocabulary every producer shares;
[`structural_types`](src/structural_types.rs) declares checked structural
types in Terminal form for every family; [`debug_map`](src/machine_lowering/debug_map.rs)
presents the Terminal debug companion.

The subordinate owners follow the work:

- [Shared closures](src/unit/attached_unit.rs) assemble every Unit body,
  scalar callees included, in one callee, type, service and qualification
  catalog. Scalar
  root selection retains an exact source-to-machine map; it does not choose a
  second assembler based on whether a body happens to use places or services.
- [Scalar callees](src/scalar_lowering/call_closure.rs) name the Unit body a
  scalar call lowers against that namespace. A selected direct result and a
  result consumed by a caller retain their distinct contract obligations.
- [Branch states](src/scalar_lowering/branch_states.rs) prepare
  [each state's](src/scalar_lowering/branch_states/prepared.rs) ordered
  bindings, structural effects and exit for the scalar-completion emitter.
- [Operation emission](src/emission/operation_emission.rs) dispatches a scalar
  binding to expression, store, selected comparison, or call emission.

[Boundary applications](src/machine_lowering/boundary_applications.rs) joins
those emitted operations to checked applications while both are owned by Psi.
It publishes exact-owner requirements and closed arguments in the semantic
module before optimization. The lowering request supplies exact toolchain source
identities alongside checked input; native realization does not reconstruct this
roster. Temporary checked application indices remain only for the selected
realization migration.
- [Operation proofs](src/proofs/operation_proofs.rs) reconstructs obligations
  from the completed module and fills missing evidence in deterministic order.
- [Evidence lowering](src/proofs/evidence_lowering.rs) retains proof subjects.

Within shared Unit assembly, [admission](src/unit/attached_unit/admission.rs)
retains the ordered body roster and collects every boundary its calls name;
stage 04 already planned each call against its exact target.
[Signatures](src/unit/attached_unit/signatures.rs) allocates all formals
before deriving requirements, retaining structural/scalar parameters, authored
predicate positions and machine-local claims together. Ordinary and composed
calls borrow the same records; emission consumes admitted bodies in roster order.
Neither step allocates a second module or changes the shared identity namespace.
Composed roots enter this same closure directly. Graph preparation computes live
states and parameter aliases before binding invocation claim names; no boolean
custody preflight speculates with an unaliased graph or selects another emitter.
Ordinary state edges consume checked transfers directly, without authenticating
them again against the checker's permission-event roster.
Qualification forwarding is admitted by stage 04's graph producer; lowering
does not repeat that predecessor walk for either owned or borrowed parameters.

[Composed callees](src/unit/attached_unit/composed_control/callable.rs)
borrow the closure's type, domain, service and boundary tables; temporary places
and identity counters remain local to emission. Composed roots and callees use
the same closure-owned tables for publication. Byte-literal stores may allocate their generated
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
form before returning it. Its
[bindings](src/expression_preparation/bindings/mod.rs) resolve current
storage and structural paths. Lowering reads no authored statement or
expression: a state's declared `let` locals come from
`CheckFacts::state_locals`, and every other value from the plan rows stage 04
hands over. [Byte literals](src/expression_preparation/byte_literals.rs)
catalog a constructor literal's byte predicates against its declared carrier.
These operations use shared
[checked-graph queries](src/expression_preparation/computation_graph.rs), not
the producer's block expansion. [Qualifications](src/expression_preparation/qualifications.rs)
prepare the selected closure's declaration namespace. No preparation module
imports a machine producer; producers consume the shared operations directly.

[Scalar return qualification](src/expression_preparation/qualifications/returns.rs)
consumes the checker's exact exit judgments, keeping predicate truth separate
from issuer authorization. [Normal-result completion](src/scalar_lowering/computations/returns.rs)
uses the ordinary operand scheduler and binds qualification only on the fresh
completion parameter. Emission retains that exact edge/source/destination;
continuation specialization remaps the edge before catalog publication.
Final return rows preserve the original forwarding and issuance alternatives,
not a provenance guess from a generated phi's annotation. Empty tag formation
has its own vacuous origin; neither it nor an ordinary cast authorizes routed
membership. These are checked-source decisions, not portable proof certificates
or a second predicate checker in lowering.

Working plans live with these operations, not in the pipeline root's namespace.
Content result records remain with [content lowering](src/proofs/content_conservation.rs).
Their existing public re-exports are unchanged. Proof completion runs only after
the complete call/storage closure is assembled; it is not a side effect of
emitting one expression.
