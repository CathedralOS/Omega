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
one availability roster, and we prune both catalogs until no more callers lose
their dependencies. If A calls B and B calls an unavailable C, B disappears on
one pass and A can disappear on the next. Checking only A's own statements, or
pruning each catalog independently, would leave a seemingly complete root with
an unlowerable transitive call. Scalar calls can borrow an ordinary scalar-result
body from this same roster; their argument, claim and contract checks still use
the exact scalar signature. Boundary and structural-result calls retain their
own availability checks. Only after
pruning do we retain the referenced structural types and their transitive shapes.

When a downstream error says a checked transitive machine plan is missing,
look for failure during local construction, receiver reconciliation, or this
closure pruning before changing lowering. The current Option-based builders do
not retain which local requirement failed; absence alone does not identify it.
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
    CheckedStructuralAccess, CheckedStructuralCallPlan, CheckedStructuralCallReturnMachinePlan,
    CheckedStructuralCallReturnPlans, CheckedStructuralControlSuccessorPlan,
    CheckedStructuralControlTransferPlan, CheckedStructuralResultPlan,
    CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
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
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralDomainPlan,
    CheckedUnitStructuralDomainRequirementPlan, CheckedUnitStructuralFieldPlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ContractProofFactKind,
    ContractProofFactOwner,
};
use diagnostics::Diagnostic;
use language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, SemanticDomainId, ServiceReachSummary,
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

pub(in crate::flow) mod calls;
mod cleanup;
mod composed_control;
pub(crate) mod control;
mod dynamic_scalar_calls;
mod primitive_store;
mod providers;
mod receiver_aliases;
mod receiver_calls;
pub(crate) mod returns;
mod scalar_locals;
mod scalar_targets;
mod selected_ieee_float;
pub(super) mod selected_operator;
pub(crate) mod shared_convergence;
mod state_graph;
mod structural_scalar_store;
pub(super) mod types;

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
use structural_scalar_store::build_structural_scalar_field_store;
use types::*;

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
pub(crate) fn build_checked_unit_effect_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    selected_operator_applications: &[crate::SelectedOperatorApplication],
    selected_ieee_float_fma_applications: &[crate::SelectedIeeeFloatFmaUnitApplication],
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
    let mut candidates = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_checked_machine(
                program,
                facts,
                &mut shapes,
                machine,
                selected_operator_applications,
                selected_ieee_float_fma_applications,
            )
        })
        .collect::<Vec<_>>();
    let mut composed_machines = build_checked_composed_unit_control_machines(
        program,
        facts,
        &mut shapes,
        &boundary_machines,
    );
    receiver_calls::reconcile(
        program,
        facts,
        &mut shapes,
        &mut candidates,
        &mut composed_machines,
        selected_operator_applications,
        selected_ieee_float_fma_applications,
    );
    // Prefer a complete general state graph when both builders describe the
    // same structural-value body. Scalar completion still belongs to the
    // ordinary sequence; merely containing a structural value does not make
    // a general graph available or invalidate that checked sequence.
    candidates.retain(|plan| {
        !(composed_machines
            .iter()
            .any(|graph| graph.machine == plan.machine)
            && plan.operations.iter().any(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                )
            }))
    });
    let dynamic_dispatch =
        build_checked_dynamic_dispatch_plans(program, facts, &mut shapes, &boundary_machines);

    // Both catalogs contain complete admitted bodies. Resolve ordinary calls
    // against their joint entry roster, then prune both sides to a fixed point:
    // an invalid composed leaf must also retire its ordinary upstream callers.
    loop {
        let entries = candidates
            .iter()
            .map(|plan| (plan.machine, plan.state))
            .chain(composed_machines.iter().map(|plan| {
                (
                    plan.machine,
                    plan.states
                        .first()
                        .map_or(SymbolHandle::invalid(), |state| state.state),
                )
            }))
            .collect::<Vec<_>>();
        let unique_entries = entries
            .iter()
            .filter(|(machine, state)| {
                machine.is_valid()
                    && state.is_valid()
                    && entries
                        .iter()
                        .filter(|(candidate, _)| candidate == machine)
                        .count()
                        == 1
            })
            .copied()
            .collect::<Vec<_>>();
        let old_lengths = (candidates.len(), composed_machines.len());
        // Every scalar call observes the same candidate roster for this pass.
        // Mutating it during availability checks would make transitive pruning
        // depend on declaration order; no body copies are needed to retain it.
        let retained_candidates =
            candidates
                .iter()
                .map(|plan| {
                    if !unique_entries.contains(&(plan.machine, plan.state)) {
                        return false;
                    }
                    plan.operations.iter().all(|operation| {
                        match operation {
                    CheckedUnitEffectOperationPlan::CallUnit {
                        target_machine,
                        target_state,
                        ..
                    } => unique_entries.contains(&(*target_machine, *target_state)),
                    CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                        boundary_symbols.contains(target_machine)
                    }
                    CheckedUnitEffectOperationPlan::BoundaryScalarCall {
                        target_machine, ..
                    } => boundary_symbols.contains(target_machine),
                    CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        target_machine,
                        ..
                    } => boundary_symbols.contains(target_machine),
                    CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                        scalar_targets::is_available(program, facts, &candidates, plan, operation)
                    }
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        target_machine,
                        target_state,
                        ..
                    } => {
                        unique_entries.contains(&(*target_machine, *target_state))
                            || facts
                                .flow
                                .terminal_structural_returns
                                .claim_free_affine_for_machine(*target_machine)
                                .is_some()
                    }
                    // Exact realization custody was already joined by selected
                    // execution before this plan was minted.
                    CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                    | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
                        ..
                    } => true,
                    CheckedUnitEffectOperationPlan::PortWrite { .. }
                    | CheckedUnitEffectOperationPlan::EstablishScalarArray { .. }
                    | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                    | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                    | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                    | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                    | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                    | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                    | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                    | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal { .. }
                    | CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { .. }
                    | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                    | CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
                    | CheckedUnitEffectOperationPlan::Complete { .. } => true,
                }
                    })
                })
                .collect::<Vec<_>>();
        let mut retained_candidates = retained_candidates.into_iter();
        candidates.retain(|_| retained_candidates.next().unwrap_or(false));
        composed_machines.retain(|plan| {
            unique_entries
                .iter()
                .any(|(machine, _)| *machine == plan.machine)
                && plan
                    .states
                    .iter()
                    .flat_map(|state| &state.operations)
                    .all(|operation| match operation {
                        CheckedUnitEffectOperationPlan::CallUnit {
                            target_machine,
                            target_state,
                            ..
                        }
                        | CheckedUnitEffectOperationPlan::StructuralCall {
                            target_machine,
                            target_state,
                            ..
                        } => unique_entries.contains(&(*target_machine, *target_state)),
                        CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                            boundary_symbols.contains(target_machine)
                        }
                        CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                            target_machine,
                            ..
                        } => boundary_symbols.contains(target_machine),
                        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(_)
                        | CheckedUnitEffectOperationPlan::ByteSequenceWrite(_)
                        | CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(_)
                        | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                        | CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                        | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. } => true,
                        _ => false,
                    })
        });
        if (candidates.len(), composed_machines.len()) == old_lengths {
            break;
        }
    }
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
                        .operations
                        .iter()
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
    for operation in candidates.iter().flat_map(|plan| &plan.operations) {
        match operation {
            CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
                realization_machine,
                realization_state,
                ..
            } => {
                let Some(realization) = facts
                    .flow
                    .terminal_structural_scalar_returns
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
            build_nominal_affine_unit_cleanup_machine(
                program,
                facts,
                unit_effects,
                &mut shapes,
                machine,
                diagnostics,
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
