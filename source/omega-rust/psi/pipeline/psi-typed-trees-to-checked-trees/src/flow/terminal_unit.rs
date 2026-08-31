use std::collections::{BTreeMap, BTreeSet};

use psi_checked_trees::{
    CheckFacts, CheckedAffineConstructionElementPlan, CheckedBoundaryMachinePlan,
    CheckedBoundaryScalarReturnMachinePlan, CheckedBoundaryScalarReturnPlans,
    CheckedIntegerBinaryKind, CheckedNominalAffineUnitCleanupMachinePlan,
    CheckedNominalAffineUnitCleanupPlans, CheckedPartialAffineUnitCleanupMachinePlan,
    CheckedPartialAffineUnitCleanupPlans, CheckedPayloadlessCaseReturnMachinePlan,
    CheckedPayloadlessGuardedCallEvidencePlan, CheckedPayloadlessGuardedCallEvidenceUsePlan,
    CheckedPayloadlessGuardedCallReturnMachinePlan, CheckedProviderAttachmentRequirementPlan,
    CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralAccess, CheckedStructuralCallPlan,
    CheckedStructuralCallReturnMachinePlan, CheckedStructuralCallReturnPlans,
    CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralResultPlan, CheckedStructuralReturnMachinePlan, CheckedStructuralReturnPlans,
    CheckedStructuralScalarArgumentPlan, CheckedStructuralScalarIntegerBoundKind,
    CheckedStructuralScalarIntegerBoundPlan, CheckedStructuralScalarIntegerBoundRequirementPlan,
    CheckedStructuralScalarParameterPlan, CheckedStructuralScalarReturnCleanupAction,
    CheckedStructuralScalarReturnMachinePlan, CheckedStructuralScalarReturnPlans,
    CheckedStructuralUnitControlMachinePlan, CheckedStructuralUnitControlPlans,
    CheckedStructuralUnitControlStatePlan, CheckedStructuralUnitControlTerminatorPlan,
    CheckedTraitOperatorScalarReturnMachinePlan, CheckedTrivialAffineStructuralLocalPlan,
    CheckedUnitCallCoordinate, CheckedUnitClaimTransferPlan, CheckedUnitEffectMachinePlan,
    CheckedUnitEffectOperationPlan, CheckedUnitEffectPlans, CheckedUnitEntryClaimPlan,
    CheckedUnitNominalAffineCallerRequirementPlan, CheckedUnitNominalAffineCleanupPlan,
    CheckedUnitNominalAffineCleanupRequirementPlan, CheckedUnitPartialAffineDiscardPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralDomainPlan, CheckedUnitStructuralDomainRequirementPlan,
    CheckedUnitStructuralFieldPlan, CheckedUnitStructuralFieldType,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, CheckedUnitStructuralTypeShape, ContractProofFactKind,
    ContractProofFactOwner,
};
use psi_diagnostics::Diagnostic;
use psi_language_semantics::{
    CarryPolicy, MachineSupplyMode, Multiplicity, PermissionAccess, PermissionClaimIdentity,
    PermissionEventKind, PermissionEventSource, SemanticDomainId,
};
use psi_symbols::{BuiltinFunction, SymbolHandle};
use psi_typed_trees::{
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
pub(crate) mod control;
pub(crate) mod returns;
mod selected_operator;
pub(crate) mod shared_convergence;
pub(super) mod types;

use calls::*;
use cleanup::*;
use control::*;
use returns::*;
use selected_operator::*;
use shared_convergence::checked_shared_boolean_convergence;
use types::*;

/// Build the first general structural/Unit terminal plan after ownership and
/// carry checking have recorded their authoritative facts. Unsupported shapes
/// are omitted as a closed unit; callers therefore cannot accidentally lower a
/// root whose transitive helper or boundary settlement was only partly known.
pub(crate) fn build_checked_unit_effect_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    selected_operator_applications: &[crate::SelectedOperatorUnitApplication],
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
            )
        })
        .collect::<Vec<_>>();

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
                // Exact realization custody was already joined by selected
                // execution before this plan was minted.
                CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. } => true,
                CheckedUnitEffectOperationPlan::PortWrite { .. }
                | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. }
                | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal { .. }
                | CheckedUnitEffectOperationPlan::ReturnUnit { .. } => true,
            })
        });
        if candidates.len() == old_len {
            break;
        }
    }
    let retained_type_identities = boundary_machines
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
        })
        .chain(candidates.iter().flat_map(|plan| {
            std::iter::once(plan.attachment_type_identity.as_str())
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
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained_type_identities);

    CheckedUnitEffectPlans {
        structural_types: shapes.types.into_values().collect(),
        structural_domains: {
            shapes.domains.sort_by_key(|domain| domain.domain.0);
            shapes.domains
        },
        boundary_machines,
        machines: candidates,
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
            std::iter::once(plan.machine.attachment_type_identity.as_str())
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
            std::iter::once(plan.machine.attachment_type_identity.as_str())
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
