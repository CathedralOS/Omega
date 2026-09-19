/*
We build executable source plans from already-checked ownership, control, and
call facts. A typed body can be legal without fitting a Terminal producer yet;
these catalogs describe implementation coverage, not additional language rules.
The lowerer must receive a complete plan, never a partly recognized body.

Start with build_checked_unit_effect_plans below. We first collect boundary
signatures and ordinary single-state candidates through control.rs. We then
reconcile their implicit receivers, build composed candidates through
composed_control/assembly.rs, and reconcile composed calls against the completed
signatures. The composed builders include specialized control shapes and the
general state_graph.rs route; their current coverage differs, so a failed
candidate is not permission to omit its unsupported statements.

Local construction is only the first gate. Ordinary and composed entries share
one immutable availability roster. We check operations once, then propagate
unavailable targets through reverse call edges. If A calls B and B calls an
unavailable C, rejecting B also rejects A without rechecking A's body. Checking
only A's own statements, or
pruning each catalog independently, would leave a seemingly complete root with
an unlowerable transitive call. Scalar calls can borrow an ordinary scalar-result
body from this same roster; their argument, claim and contract checks still use
the exact scalar signature. Boundary and structural-result calls retain their
own availability checks. Only after
pruning do we retain the referenced structural types and their transitive shapes.

When a downstream error says a checked transitive machine plan is missing,
read the record's `omissions` roster first: every checked-body machine without
a plan carries the stage that dropped it (local construction, receiver
reconciliation, builder overlap, or closure pruning with the direct dependency
that was unavailable). Following the closure rows reaches the machine whose own
body failed local construction; the Option-based builders still do not retain
which local requirement failed there, so that machine's body is where to look
before changing lowering.
*/

/*
Receiver specialization is the non-obvious ordering constraint. An attached
body can use an ambient attachment without retaining borrowed self, but a callee
that retains self needs the caller's actual loan. control::build_checked_machine
retries with self retained when the ambient plan fails; receiver_calls.rs also
propagates that demand through forwarding callers after signatures exist. We
rebuild those callers with the ordinary planner rather than manually shifting
all their parameter, store, claim, and provider coordinates. Inserting the call's
receiver operand then adjusts existing claim-transfer positions; the loan itself
is not an ownership transfer.

This is transitional source planning, not native receiver provisioning. Keeping
self unconditionally depends on the ProgramEntry bridge supplying its loan
(ENTRY-CONTENT-ROOTS in TASKS.md). A field on that provisioned receiver does not
require generic source-local array construction merely because it is an array.
Likewise, publishing one of these plans establishes no native execution claim.
Partial and nominal cleanup retain separate plan owners below because their
residual fields and destructor obligations cannot become trivial root discards.
*/

/*
Existing source examples are the executable companions to this explanation:
src/tests/flow/terminal_unit/calls.rs checks mixed case payload/view edges and
affine return disposal. In the sibling checked-trees-to-lowered-psi crate,
src/tests/composed_unit_transitive_internal_calls.rs follows Root::enter through
its helpers and rejects substituted targets and missing plans;
src/tests/byte_write_loop.rs exercises borrowed buffers and scalar-case returns.
Read those controls when widening a route. The owning source/Terminal coverage
contract is ../../compiler/terminal-production/README.md relative to this crate.
*/

/*
Erased parameters. An `[erased]` binding occurrence
(contracts.md#explicit-erased-bindings) stays in the typed signature and in
proof identity but owns no ABI position. Every signature builder here
(calls/signatures.rs, calls/call_operations.rs, ../scalar/mod.rs)
skips it, and every caller-side argument producer
(values/scalar/computations.rs, values/scalar/call_lowering.rs,
flow/transfers/scalar_values/calls.rs) numbers its dense argument ordinals over
the retained positions only, so the two sides agree without a shared table.
`position` and `source_position` stay the authored typed-signature index and
therefore become sparse after a strip: the Terminal consumer rejoins each
retained parameter to `state_parameters(state)[position]` for its type, symbol
and custody, so renumbering densely would bind the wrong source parameter.
Arity checks count `abi_parameter_count`, not `state_parameters(state).len()`,
and `strips_erased_parameter` decides which bindings may be stripped at all.
The dense scalar value namespace that `CheckedScalarExpression::Parameter`
indexes is the same partition: `values::scalar::occupies_scalar_position`
excludes erased bindings there, so a body reading a retained parameter after
an erased one gets the shifted position and a runtime read of the erased one
finds no position. The consumer, checked-trees-to-lowered-psi, reconstructs
every one of these partitions independently from the typed relevance
(unit/attached_unit/parameters.rs, expression_preparation/qualifications.rs,
source_custody/{parameters,direct_calls,replay_source,storage_reads,
successors,computation_calls}, scalar_graph/scalar_computations/calls.rs)
and never trusts the plan's counts. What remains open is a contract that
names an erased binding (`requires n < bound`): Terminal contract
propositions have no proof-only value for it, so
pass/relevance/erased_parameter_proof_only stays a checked-only canary while
its two RUN siblings execute natively (canary_suite layouts_and_pending
erased_parameter_* tests).
*/

use std::collections::{BTreeMap, BTreeSet};

use checked_trees::{
    CheckFacts, CheckedAffineConstructionElementPlan, CheckedBooleanExpression,
    CheckedBoundaryMachinePlan, CheckedBoundaryMachineResultPlan,
    CheckedBoundaryScalarReturnMachinePlan, CheckedBoundaryScalarReturnPlans,
    CheckedClaimFreeAffineStructuralReturnMachinePlan, CheckedClosedSumCaseSuccessorPlan,
    CheckedClosedSumPayloadTransferPlan, CheckedComposedUnitControlMachinePlan,
    CheckedComposedUnitControlStatePlan, CheckedComposedUnitControlTerminatorPlan,
    CheckedIntegerBinaryKind, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupPlans, CheckedPayloadlessCaseReturnMachinePlan,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedSelectedOperatorStructuralScalarReturnMachinePlan,
    CheckedStructuralAccess, CheckedStructuralCallReturnPlans,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarFieldStorePlan,
    CheckedStructuralScalarIntegerBoundKind, CheckedStructuralScalarIntegerBoundPlan,
    CheckedStructuralScalarIntegerBoundRequirementPlan, CheckedStructuralScalarParameterPlan,
    CheckedStructuralScalarReturnCleanupAction, CheckedStructuralScalarReturnMachinePlan,
    CheckedStructuralScalarReturnPlans, CheckedStructuralUnitControlMachinePlan,
    CheckedStructuralUnitControlPlans, CheckedStructuralUnitControlStatePlan,
    CheckedStructuralUnitControlTerminatorPlan, CheckedTraitOperatorScalarReturnMachinePlan,
    CheckedTrivialAffineStructuralLocalPlan, CheckedUnitCallCoordinate,
    CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan,
    CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitPlanOmission, CheckedUnitPlanOmissionStage, CheckedUnitScalarResultBindingPlan,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralResultBindingPlan, CheckedUnitStructuralTypePlan,
    CheckedUnitStructuralTypeShape, ContractProofFactKind, ContractProofFactOwner,
};
use diagnostics::Diagnostic;
use language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, PermissionProvenance, SemanticDomainId,
    ServiceReachSummary,
};
use symbols::{BuiltinFunction, SymbolHandle};
use typed_trees::{
    TypedTrees,
    data::{DataMember, DataShapeKind},
    domain::ProofFact,
    expression::ExpressionNode,
    signature::{SignatureContractKind, StateParameter},
    statement::{StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode},
    types::{PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode},
};

pub(crate) mod calls;
mod candidate_closure;
mod cleanup;
mod composed_control;
pub(crate) mod control;
mod dynamic_scalar_calls;
mod primitive_store;
mod providers;
mod receiver_aliases;
mod receiver_calls;
use validation::reference_result_custody as reference_results;
pub(crate) mod returns;
mod scalar_locals;
mod scalar_targets;
mod selected_ieee_float;
pub(super) mod selected_operator;
pub(crate) mod shared_convergence;
mod state_graph;
mod structural_scalar_store;
pub(crate) mod types;

use crate::execution::terminal_unit::cleanup::build_partial_affine_unit_cleanup_machine;
pub(crate) use calls::structural_computation_argument;
use calls::*;
use cleanup::*;
use composed_control::*;
use control::*;
use dynamic_scalar_calls::*;
use primitive_store::build_write_only_primitive_store;
use providers::*;
use returns::*;
use scalar_locals::*;
use selected_ieee_float::*;
use selected_operator::*;
use shared_convergence::checked_shared_boolean_convergence;
pub(super) use structural_scalar_store::build_local_scalar_field_store;
use structural_scalar_store::build_structural_scalar_field_store;
pub(crate) use types::strips_erased_parameter;
use types::*;

/// Scalar callees available to this planning pass, independent of published facts.
#[derive(Clone, Copy)]
pub(crate) struct ScalarCalleePlans<'plans> {
    pub(crate) boundary_returns: &'plans CheckedBoundaryScalarReturnPlans,
    pub(crate) structural_returns: &'plans CheckedStructuralScalarReturnPlans,
}

pub(super) fn cleanup_type_is_unit(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> bool {
    is_unit(program, type_reference)
}

/// Reuse ordinary shape ownership for borrowed and no-code owned graph inputs.
pub(super) fn structural_scalar_graph_signature(
    program: &TypedTrees,
    state: &typed_trees::state::State,
) -> Option<(
    Vec<CheckedUnitStructuralParameterPlan>,
    Vec<CheckedStructuralScalarParameterPlan>,
    Vec<CheckedUnitStructuralTypePlan>,
)> {
    if program.state_parameters(state).iter().any(|parameter| {
        if parameter.relevance.is_erased() {
            return false;
        }
        // Numeric constraints retain their separate scalar contract owner;
        // they do not qualify an owned structural carrier.
        if program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
        {
            return !validation::has_plain_owned_contents_with_numeric_constraints(
                program,
                parameter.type_reference,
            );
        }
        // Whole array payloads include empty dimensions. Their exact recursive
        // type, not the nominal owned-storage classifier below, admits copying.
        if validation::is_closed_primitive_array_type(program, parameter.type_reference) {
            return parameter.is_mutable;
        }
        let reference = match program
            .type_reference_table
            .type_reference(parameter.type_reference)
        {
            TypeReferenceNode::Reference { referee, .. } => {
                if program.primitive_type_reference(*referee).is_none() {
                    return true;
                }
                *referee
            }
            _ => {
                if parameter.is_mutable
                    && program
                        .primitive_type_reference(parameter.type_reference)
                        .is_none()
                {
                    return true;
                }
                if !validation::has_plain_owned_contents_with_numeric_constraints(
                    program,
                    parameter.type_reference,
                ) {
                    return true;
                }
                parameter.type_reference
            }
        };
        !matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        )
    }) {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    let (structural, scalar) = free_structural_scalar_signature(program, &mut shapes, state, &[])?;
    if structural.iter().any(|parameter| {
        parameter.is_self
            || !matches!(
                (parameter.access, parameter.multiplicity),
                (_, Multiplicity::Unrestricted)
                    | (CheckedStructuralAccess::Owned, Multiplicity::Affine)
            )
            || !parameter.qualifications.is_empty()
    }) {
        return None;
    }
    Some((structural, scalar, shapes.types.into_values().collect()))
}

/// Construction and structural results share the same closed sum admission.
/// The namespace includes real value types even when no parameter names them.
pub(super) fn scalar_case_value_shapes(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    let mut shapes = ShapeCollector::new(program);
    state_graph::returns::signature(program, &mut shapes, reference)?;
    Some(shapes.types.into_values().collect())
}

/// Scalar control retains complete fresh-record storage in the same declaration
/// namespace as the ordinary structural value emitter, including nested bounds.
pub(super) fn scalar_graph_record_shapes(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<Vec<CheckedUnitStructuralTypePlan>> {
    if !validation::has_plain_owned_contents_with_numeric_constraints(program, reference)
        || !matches!(
            program.type_multiplicity(reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
        || !matches!(
            program.type_reference_table.type_reference(reference),
            TypeReferenceNode::Named { .. }
        )
    {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    shapes.add_type(reference, &[], &[])?;
    if !shapes.domains.is_empty()
        || !shapes.types.values().all(|declaration| {
            matches!(&declaration.shape, CheckedUnitStructuralTypeShape::Record { fields }
            if fields.iter().all(|field| !field.relevance.is_erased()
                && matches!(field.field_type, CheckedUnitStructuralFieldType::Scalar(_)
                    | CheckedUnitStructuralFieldType::BoundedInteger(_)
                    | CheckedUnitStructuralFieldType::Structural { .. })))
        })
    {
        return None;
    }
    Some(shapes.types.into_values().collect())
}

/// Reconstruct the exact direct-record shape admitted by the first checked
/// projected-transition cleanup rung. Keeping this next to `ShapeCollector`
/// makes the result use the same normalized field/type identities as the
/// established partial-return residual walker.
pub(super) fn exact_two_field_record_projection(
    program: &TypedTrees,
    root_type: TypeReferenceHandle,
    moved_field: SymbolHandle,
    target_type: TypeReferenceHandle,
) -> Option<(String, String, String, String)> {
    let TypeReferenceNode::Named {
        symbol: root_symbol,
        ..
    } = program.type_reference_table.type_reference(root_type)
    else {
        return None;
    };
    let root = program
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *root_symbol)?;
    if root.properties.multiplicity != Multiplicity::Affine
        || root.properties.carry.is_some()
        || !root.lifetime_parameters.is_empty()
        || !program.data_type_parameters(root).is_empty()
        || type_graph_requires_nominal_drop(program, root_type)
    {
        return None;
    }
    let members = program.data_members(root);
    let [DataMember::Field(left), DataMember::Field(right)] = members else {
        return None;
    };
    let source_fields = [left, right];
    if source_fields.iter().any(|field| {
        field.relevance.is_erased()
            || crate::checks::type_multiplicity(program, field.type_reference)
                != Multiplicity::Affine
            || type_graph_requires_nominal_drop(program, field.type_reference)
    }) {
        return None;
    }

    let mut shapes = ShapeCollector::new(program);
    let root_identity = shapes.add_type(root_type, &[], &[])?;
    let target_identity = shapes.add_type(target_type, &[], &[])?;
    let root_plan = shapes.types.get(&root_identity)?;
    let CheckedUnitStructuralTypeShape::Record { fields } = &root_plan.shape else {
        return None;
    };
    let [left_plan, right_plan] = fields.as_slice() else {
        return None;
    };
    let plans = [left_plan, right_plan];
    let moved_index = source_fields
        .iter()
        .position(|field| field.symbol == moved_field)?;
    let residual_index = 1_usize.checked_sub(moved_index)?;
    let CheckedUnitStructuralFieldType::Structural {
        type_identity: moved_type_identity,
    } = &plans[moved_index].field_type
    else {
        return None;
    };
    let CheckedUnitStructuralFieldType::Structural {
        type_identity: residual_type_identity,
    } = &plans[residual_index].field_type
    else {
        return None;
    };
    if moved_type_identity != &target_identity {
        return None;
    }
    Some((
        plans[moved_index].identity.clone(),
        moved_type_identity.clone(),
        plans[residual_index].identity.clone(),
        residual_type_identity.clone(),
    ))
}

/// Build the first general structural/Unit terminal plan after ownership and
/// carry checking have recorded their authoritative facts. Unsupported shapes
/// are omitted as a closed unit; callers therefore cannot accidentally lower a
/// root whose transitive helper or boundary settlement was only partly known.
#[cfg(test)]
pub(crate) fn build_checked_unit_effect_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
) -> CheckedUnitEffectPlans {
    build_checked_unit_effect_plans_with_call_frames(
        program,
        facts,
        scalar_callees,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        None,
    )
}

pub(crate) fn build_checked_unit_effect_plans_with_call_frames(
    program: &TypedTrees,
    facts: &CheckFacts,
    scalar_callees: ScalarCalleePlans<'_>,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> CheckedUnitEffectPlans {
    let mut shapes = ShapeCollector::new(program);
    let mut boundary_machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode.is_boundary_declaration())
        .filter_map(|machine| build_boundary_machine(program, facts, &mut shapes, machine))
        .collect::<Vec<_>>();
    boundary_machines.extend(build_static_boundary_requirements(
        program,
        facts,
        &mut shapes,
    ));
    let boundary_symbols = boundary_machines
        .iter()
        .map(|plan| plan.machine)
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let mut local_construction = BTreeMap::new();
    for machine in program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
    {
        let trace = LocalConstructionTrace::default();
        match build_checked_machine_traced(
            program,
            facts,
            scalar_callees,
            &mut shapes,
            machine,
            selected_operator_applications,
            selected_ieee_float_fma_applications,
            call_frames,
            &trace,
        ) {
            Some(plan) => candidates.push(plan),
            None => {
                local_construction.insert(omission_key(machine.symbol), trace.stage());
            }
        }
    }
    // A free consuming machine (a `drop<T>` specialization) whose parameters
    // carry an exact owner-attached `::drop` hook has no authored body, so the
    // ordinary builder omits it at completion. It is still a real call target:
    // seed its synthetic complete-only plan before closure pruning so callers
    // transfer custody into it, and the exact hook edge rides the nominal
    // cleanup roster into lowering. Requirement diagnostics stay owned by the
    // finalize-time pass over the retained roster.
    let mut nominal_diagnostics = Vec::new();
    let nominal_candidates = program
        .machines()
        .iter()
        .filter(|machine| {
            machine.supply_mode == MachineSupplyMode::CheckedBody
                && machine.attached_data.is_none()
                && !candidates.iter().any(|plan| plan.machine == machine.symbol)
        })
        .filter_map(|machine| {
            build_nominal_affine_unit_cleanup_machine(
                program,
                facts,
                &candidates,
                &mut shapes,
                machine,
                &mut nominal_diagnostics,
            )
        })
        .collect::<Vec<_>>();
    candidates.extend(
        nominal_candidates
            .iter()
            .map(|nominal| nominal.machine.clone()),
    );
    let nominal_cleanup_edges = nominal_candidates
        .iter()
        .flat_map(|nominal| {
            nominal
                .cleanups
                .iter()
                .map(|cleanup| (nominal.machine.machine, cleanup.cleanup_machine))
        })
        .collect::<Vec<_>>();

    let mut composed_construction = BTreeMap::new();
    let mut composed_machines = build_checked_composed_unit_control_machines_traced(
        program,
        facts,
        scalar_callees,
        &mut shapes,
        &boundary_machines,
        call_frames,
        &mut composed_construction,
    );
    let mut omissions = OmissionLedger::new(
        program,
        &local_construction,
        &composed_construction,
        &candidates,
        &composed_machines,
    );
    receiver_calls::reconcile(
        program,
        facts,
        scalar_callees,
        &mut shapes,
        &mut candidates,
        &mut composed_machines,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
        call_frames,
    );
    // Prefer a complete general state graph when both builders describe the
    // same structural-result body, including a returned parameter or call.
    // Otherwise both catalogs appear to define the same entry and closure
    // pruning removes the callee and every caller as ambiguous. Unit/scalar
    // completion still belongs to the ordinary sequence when that complete
    // body exists. Resolve builder overlap before closure, never retry another
    // body after a selected candidate loses a required dependency.
    candidates.retain(|plan| {
        !(composed_machines
            .iter()
            .any(|graph| graph.machine == plan.machine)
            && plan.structural_result.is_some())
    });
    composed_machines.retain(|graph| {
        !matches!(graph.result, checked_trees::CheckedControlResultPlan::Unit)
            || !candidates.iter().any(|plan| {
                plan.machine == graph.machine
                    && graph
                        .states
                        .first()
                        .is_some_and(|state| state.state == plan.state)
                    && plan.structural_result.is_none()
                    && plan.scalar_result.is_none()
                    && plan.scalar_control.is_none()
            })
    });
    omissions.record_dropped(
        &candidates,
        &composed_machines,
        CheckedUnitPlanOmissionStage::ReceiverReconciliation,
    );
    let dynamic_dispatch =
        build_checked_dynamic_dispatch_plans(program, facts, &mut shapes, &boundary_machines);

    let closure_omissions = candidate_closure::retain_available(
        program,
        facts,
        scalar_callees,
        &boundary_symbols,
        &mut candidates,
        &mut composed_machines,
        &nominal_cleanup_edges,
    );
    omissions.record_closure(&candidates, &composed_machines, closure_omissions);
    // A claim-free affine structural-return machine already owns its checked
    // plan and dedicated emitter; an ordinary or composed Unit plan for the
    // same machine is a competing body description, not a second admission.
    // Ordinary callers and provider discovery already route such a machine
    // through the structural-result catalog, but a retained composed body is
    // different: its internal calls admit only Unit-roster targets, so a
    // callee a composed plan still invokes keeps its Unit entry. Run this
    // exclusion after closure pruning so only surviving callers count.
    let mut composed_call_targets = composed_machines
        .iter()
        .flat_map(|graph| graph.states.iter())
        .flat_map(|state| state.operation_dependencies())
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. }
            | CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. } => {
                Some(omission_key(*target_machine))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    composed_call_targets.sort_unstable();
    composed_call_targets.dedup();
    candidates.retain(|plan| {
        facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_for_machine(plan.machine)
            .is_none()
            || composed_call_targets
                .binary_search(&omission_key(plan.machine))
                .is_ok()
    });
    composed_machines.retain(|graph| {
        facts
            .flow
            .terminal_structural_returns
            .claim_free_affine_for_machine(graph.machine)
            .is_none()
            || composed_call_targets
                .binary_search(&omission_key(graph.machine))
                .is_ok()
    });
    let mut retained_type_identities = boundary_machines
        .iter()
        .flat_map(|plan| {
            plan.attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(plan.result.structural_identity())
        })
        .chain(candidates.iter().flat_map(|plan| {
            plan.attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    plan.structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .map(|local| local.type_identity.as_str()),
                )
                .chain(
                    plan.trivial_affine_locals
                        .iter()
                        .filter_map(|local| local.construction.as_ref())
                        .map(|element| element.root_type_identity.as_str()),
                )
                .chain(
                    plan.operations
                        .iter()
                        .filter_map(|operation| match operation {
                            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                                type_identity,
                                ..
                            } => Some(type_identity.as_str()),
                            _ => None,
                        }),
                )
        }))
        .chain(composed_machines.iter().flat_map(|plan| {
            plan.attachment_type_identity
                .as_deref()
                .into_iter()
                .chain(match &plan.result {
                    checked_trees::CheckedControlResultPlan::Unit => None,
                    checked_trees::CheckedControlResultPlan::Structural(result) => {
                        Some(result.type_identity.as_str())
                    }
                })
                .chain(
                    plan.states
                        .iter()
                        .flat_map(|state| &state.structural_parameters)
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(plan.states.iter().flat_map(|state| {
                    state
                        .operation_dependencies()
                        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
                        .filter_map(|operation| match operation {
                            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
                                type_identity,
                                ..
                            } => Some(type_identity.as_str()),
                            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                                result,
                                ..
                            }
                            | CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                            | CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                                result,
                                ..
                            } => Some(result.type_identity.as_str()),
                            _ => None,
                        })
                }))
        }))
        .chain(
            dynamic_dispatch
                .direct_scalar_calls
                .iter()
                .flat_map(|plan| {
                    [
                        plan.caller_attachment_type_identity.as_str(),
                        plan.source_type_identity.as_str(),
                    ]
                }),
        )
        .chain(
            dynamic_dispatch
                .rebound_scalar_calls
                .iter()
                .flat_map(|plan| {
                    [
                        plan.latest.caller_attachment_type_identity.as_str(),
                        plan.initial.type_identity.as_str(),
                        plan.latest.source_type_identity.as_str(),
                    ]
                }),
        )
        .chain(
            dynamic_dispatch
                .joined_scalar_calls
                .iter()
                .flat_map(|plan| {
                    [
                        plan.caller_attachment_type_identity.as_str(),
                        plan.when_true.call.source_type_identity.as_str(),
                        plan.when_false.call.source_type_identity.as_str(),
                    ]
                }),
        )
        .chain(
            dynamic_dispatch
                .stored_scalar_calls
                .iter()
                .flat_map(|plan| {
                    [
                        plan.call.caller_attachment_type_identity.as_str(),
                        plan.call.source_type_identity.as_str(),
                    ]
                }),
        )
        .chain(dynamic_dispatch.direct_unit_calls.iter().flat_map(|plan| {
            [
                plan.caller_attachment_type_identity.as_str(),
                plan.source_type_identity.as_str(),
            ]
        }))
        .chain(dynamic_dispatch.rebound_unit_calls.iter().flat_map(|plan| {
            [
                plan.latest.caller_attachment_type_identity.as_str(),
                plan.initial.type_identity.as_str(),
                plan.latest.source_type_identity.as_str(),
            ]
        }))
        .chain(dynamic_dispatch.joined_unit_calls.iter().flat_map(|plan| {
            [
                plan.caller_attachment_type_identity.as_str(),
                plan.when_true.call.source_type_identity.as_str(),
                plan.when_false.call.source_type_identity.as_str(),
            ]
        }))
        .collect::<BTreeSet<_>>();
    for operation in candidates
        .iter()
        .flat_map(|plan| &plan.operations)
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        match operation {
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                realization_machine,
                realization_state,
                ..
            } => {
                let Some(realization) =
                    scalar_callees
                        .structural_returns
                        .machines
                        .iter()
                        .find(|plan| {
                            plan.machine == *realization_machine && plan.state == *realization_state
                        })
                else {
                    continue;
                };
                retained_type_identities.extend(realization.attachment_type_identity.as_deref());
                retained_type_identities.extend(
                    realization
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                );
            }
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall {
                realization_machine,
                realization_state,
                result,
                ..
            }
            | CheckedUnitEffectOperationPlan::StructuralCall {
                target_machine: realization_machine,
                target_state: realization_state,
                result,
                ..
            } => {
                let Some(realization) = facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_machines
                    .iter()
                    .find(|plan| {
                        plan.machine == *realization_machine && plan.state == *realization_state
                    })
                else {
                    continue;
                };
                retained_type_identities.extend(realization.attachment_type_identity.as_deref());
                retained_type_identities
                    .insert(realization.structural_parameter.type_identity.as_str());
                retained_type_identities.insert(result.type_identity.as_str());
            }
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishReference { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. }
            | CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. } => {
                retained_type_identities.insert(result.type_identity.as_str());
            }
            _ => {}
        }
    }
    shapes.retain_transitive(&retained_type_identities);

    CheckedUnitEffectPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines: candidates,
        dynamic_dispatch,
        composed_machines,
        omissions: omissions.into_rows(),
    }
}

/// Omission evidence gathered while the roster is built: which checked-body
/// machines never received a body, and which admitted bodies each later stage
/// dropped. Rows are diagnostic; they never reinstate a body.
struct OmissionLedger {
    rows: Vec<CheckedUnitPlanOmission>,
    named: BTreeSet<(u32, u32)>,
    admitted: BTreeSet<(u32, u32)>,
}

fn omission_key(symbol: SymbolHandle) -> (u32, u32) {
    (symbol.arena_index(), symbol.generation())
}

impl OmissionLedger {
    fn new(
        program: &TypedTrees,
        local_construction: &BTreeMap<(u32, u32), CheckedUnitPlanOmissionStage>,
        composed_construction: &BTreeMap<(u32, u32), CheckedUnitPlanOmissionStage>,
        candidates: &[CheckedUnitEffectMachinePlan],
        composed_machines: &[CheckedComposedUnitControlMachinePlan],
    ) -> Self {
        let admitted = Self::planned(candidates, composed_machines);
        let mut ledger = Self {
            rows: Vec::new(),
            named: BTreeSet::new(),
            admitted,
        };
        let unplanned = program
            .machines()
            .iter()
            .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
            .filter(|machine| !ledger.admitted.contains(&omission_key(machine.symbol)))
            .map(|machine| machine.symbol)
            .collect::<Vec<_>>();
        for machine in unplanned {
            // A multi-state body never reaches the ordinary builder's later
            // phases; the general state-graph route's trace explains it.
            let key = omission_key(machine);
            let stage = match local_construction.get(&key) {
                Some(CheckedUnitPlanOmissionStage::LocalConstruction {
                    phase: "single-state body",
                    ..
                })
                | None => composed_construction.get(&key).copied().unwrap_or(
                    CheckedUnitPlanOmissionStage::LocalConstruction {
                        phase: "composed control",
                        state_index: None,
                        statement_index: None,
                    },
                ),
                Some(stage) => *stage,
            };
            ledger.name(CheckedUnitPlanOmission { machine, stage });
        }
        ledger
    }

    fn planned(
        candidates: &[CheckedUnitEffectMachinePlan],
        composed_machines: &[CheckedComposedUnitControlMachinePlan],
    ) -> BTreeSet<(u32, u32)> {
        candidates
            .iter()
            .map(|plan| omission_key(plan.machine))
            .chain(
                composed_machines
                    .iter()
                    .map(|plan| omission_key(plan.machine)),
            )
            .collect()
    }

    /// The first row for a machine wins; later stages cannot rename it.
    fn name(&mut self, row: CheckedUnitPlanOmission) {
        if self.named.insert(omission_key(row.machine)) {
            self.rows.push(row);
        }
    }

    /// Every previously admitted machine that no longer has a body left at
    /// `stage`.
    fn record_dropped(
        &mut self,
        candidates: &[CheckedUnitEffectMachinePlan],
        composed_machines: &[CheckedComposedUnitControlMachinePlan],
        stage: CheckedUnitPlanOmissionStage,
    ) {
        let planned = Self::planned(candidates, composed_machines);
        let dropped = self
            .admitted
            .difference(&planned)
            .copied()
            .collect::<Vec<_>>();
        for (arena_index, generation) in dropped {
            self.name(CheckedUnitPlanOmission {
                machine: SymbolHandle::from_parts(arena_index, generation),
                stage,
            });
        }
        self.admitted = planned;
    }

    /// Closure pruning reports its own rows; a machine it did not name but
    /// that still vanished was one of two competing bodies for one entry.
    fn record_closure(
        &mut self,
        candidates: &[CheckedUnitEffectMachinePlan],
        composed_machines: &[CheckedComposedUnitControlMachinePlan],
        closure_omissions: Vec<CheckedUnitPlanOmission>,
    ) {
        let planned = Self::planned(candidates, composed_machines);
        for row in closure_omissions {
            if !planned.contains(&omission_key(row.machine)) {
                self.name(row);
            }
        }
        self.record_dropped(
            candidates,
            composed_machines,
            CheckedUnitPlanOmissionStage::CompetingCandidates,
        );
    }

    fn into_rows(mut self) -> Vec<CheckedUnitPlanOmission> {
        self.rows
            .sort_by_key(|row| (row.machine.arena_index(), row.machine.generation()));
        self.rows
    }
}

/// Build the checked front of direct-record path-sensitive affine cleanup.
///
/// This plan is deliberately parallel to `CheckedUnitEffectPlans`: current
/// ordinary Unit production has root-only cleanup, so publishing partial
/// moves through that lane would silently erase the live sibling.
pub(crate) fn build_checked_partial_affine_unit_cleanup_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
) -> CheckedPartialAffineUnitCleanupPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_partial_affine_unit_cleanup_machine(
                program,
                facts,
                unit_effects,
                &mut shapes,
                machine,
            )
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            plan.machine
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    plan.machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.residual_affine_discards
                        .iter()
                        .map(|discard| discard.type_identity.as_str()),
                )
                .chain(
                    plan.machine
                        .operations
                        .iter()
                        .filter_map(|operation| match operation {
                            CheckedUnitEffectOperationPlan::StructuralCall { result, .. }
                            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                                result,
                                ..
                            } => Some(result.type_identity.as_str()),
                            _ => None,
                        }),
                )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedPartialAffineUnitCleanupPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}

/// Build the checked front of the first executable nominal-cleanup slice.
///
/// The admitted caller is deliberately tiny: one state, one whole claim-free
/// unqualified affine parameter of a finite flat record whose fields are all
/// relevant terminal-supported primitive scalars, an empty Unit body, and one
/// exact checked empty `Type::drop(&mut self)` attached to that type. Nested,
/// erased, floating-point, and aggregate fields are omitted atomically. In
/// particular, the return operation publishes no trivial discard for the
/// parameter; the separate cleanup row is the only disposal authority.
pub(crate) fn build_checked_nominal_affine_unit_cleanup_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    diagnostics: &mut Vec<Diagnostic>,
) -> CheckedNominalAffineUnitCleanupPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            let plan = build_nominal_affine_unit_cleanup_machine(
                program,
                facts,
                &unit_effects.machines,
                &mut shapes,
                machine,
                diagnostics,
            )?;
            // A free consuming machine — the `drop<T>` specialization — is a
            // real ordinary roster member: the cleanup roster covers it only
            // when closure pruning retained that exact body. Missing cleanup
            // requirements still diagnose above even when the caller-side
            // pruning dropped the consuming machine.
            if machine.attached_data.is_none() && unit_effects.for_machine(machine.symbol).is_none()
            {
                return None;
            }
            Some(plan)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|plan| {
            plan.machine
                .attachment_type_identity
                .iter()
                .map(String::as_str)
                .chain(
                    plan.machine
                        .structural_parameters
                        .iter()
                        .map(|parameter| parameter.type_identity.as_str()),
                )
                .chain(
                    plan.cleanups
                        .iter()
                        .map(|cleanup| cleanup.type_identity.as_str()),
                )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedNominalAffineUnitCleanupPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}
