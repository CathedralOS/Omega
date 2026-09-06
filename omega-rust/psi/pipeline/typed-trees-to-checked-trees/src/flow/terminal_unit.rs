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

mod calls;
mod cleanup;
mod composed_control;
pub(crate) mod control;
mod dynamic_scalar_calls;
mod providers;
pub(crate) mod returns;
mod scalar_locals;
mod scalar_targets;
mod selected_ieee_float;
pub(super) mod selected_operator;
pub(crate) mod shared_convergence;
mod structural_scalar_store;
pub(super) mod types;

use calls::*;
use cleanup::*;
use composed_control::*;
use control::*;
use dynamic_scalar_calls::*;
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
    let dynamic_dispatch =
        build_checked_dynamic_dispatch_plans(program, facts, &mut shapes, &boundary_machines);

    loop {
        let checked_symbols = candidates
            .iter()
            .map(|plan| plan.machine)
            .collect::<Vec<_>>();
        let old_len = candidates.len();
        candidates.retain(|plan| {
            plan.operations.iter().all(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    checked_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                    boundary_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryScalarCall { target_machine, .. } => {
                    boundary_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    target_machine, ..
                } => boundary_symbols.contains(target_machine),
                CheckedUnitEffectOperationPlan::ScalarCall { .. } => {
                    scalar_targets::is_available(program, facts, plan, operation)
                }
                CheckedUnitEffectOperationPlan::StructuralCall { target_machine, .. } => facts
                    .flow
                    .terminal_structural_returns
                    .claim_free_affine_for_machine(*target_machine)
                    .is_some(),
                // Exact realization custody was already joined by selected
                // execution before this plan was minted.
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall { .. }
                | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { .. }
                | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd { .. } => true,
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(_)
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal { .. }
                | CheckedUnitEffectOperationPlan::EstablishScalarLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => true,
            })
        });
        if candidates.len() == old_len {
            break;
        }
    }
    let checked_symbols = candidates
        .iter()
        .map(|plan| plan.machine)
        .collect::<Vec<_>>();
    composed_machines.retain(|plan| {
        plan.states
            .iter()
            .flat_map(|state| &state.operations)
            .all(|operation| match operation {
                CheckedUnitEffectOperationPlan::CallUnit { target_machine, .. } => {
                    checked_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryCall { target_machine, .. } => {
                    boundary_symbols.contains(target_machine)
                }
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    target_machine, ..
                } => boundary_symbols.contains(target_machine),
                _ => false,
            })
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
        }))
        .chain(composed_machines.iter().flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
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
                            CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
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
                retained_type_identities.insert(realization.attachment_type_identity.as_str());
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
            CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. } => {
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
/// terminal Psi still has a root-only affine frontier, so publishing the
/// machine through that older lane would silently erase its live sibling.
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
