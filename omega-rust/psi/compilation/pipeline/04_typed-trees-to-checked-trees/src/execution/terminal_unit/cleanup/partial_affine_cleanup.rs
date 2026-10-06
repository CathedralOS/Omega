//! Residual reconstruction shares the ordinary operation sequencer.

use crate::execution::terminal_unit::cleanup::residuals;
use crate::execution::terminal_unit::types::ShapeCollector;
use crate::execution::terminal_unit::{
    BTreeMap, CheckFacts, CheckedPartialAffineUnitCleanupMachinePlan, CheckedUnitEffectPlans,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralFieldType, CheckedUnitStructuralPathSegment,
    CheckedUnitStructuralTypePlan, PermissionEventKind, PrimitiveType, ScalarCalleePlans,
    TypedTrees, control,
};

pub(crate) fn build_partial_affine_unit_cleanup_machine(
    program: &TypedTrees,
    facts: &CheckFacts,
    unit_effects: &CheckedUnitEffectPlans,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
) -> Option<CheckedPartialAffineUnitCleanupMachinePlan> {
    let [state] = program.machine_states(machine) else {
        return None;
    };
    // The general route: an ordinary body the roster declined because its
    // return edge owes the residual complement of one partially moved owned
    // parameter. The carrier exists for exactly this path-sensitive return
    // cleanup, so republish the machine here when the residual rebuild
    // confirms it. Scalar-callee, selected-operator and call-frame inputs
    // are not retained at this stage; a body needing them keeps its earlier
    // admission failure rather than silently dropping custody.
    if unit_effects.for_machine(machine.symbol).is_none()
        && facts.flow.ownership.permissions.iter().any(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.kind == PermissionEventKind::Transfer
                && !event.segments.is_empty()
        })
        && let Some((plan, residual_affine_discards, has_projected_parameter_moves)) =
            control::build_checked_machine_residual_parts(
                program,
                facts,
                ScalarCalleePlans {
                    boundary_returns: &facts.flow.terminal_boundary_scalar_returns,
                    structural_returns: &facts.flow.terminal_structural_scalar_returns,
                },
                shapes,
                machine,
                None,
                &control::LocalConstructionTrace::default(),
            )
        && has_projected_parameter_moves
    {
        return Some(CheckedPartialAffineUnitCleanupMachinePlan {
            machine: plan,
            residual_affine_discards,
        });
    }
    None
}

pub(crate) fn partial_affine_residuals(
    types: &BTreeMap<String, CheckedUnitStructuralTypePlan>,
    source: &CheckedUnitStructuralArgumentSourcePlan,
    root_type: &str,
    moved_paths: &[(Vec<CheckedUnitStructuralPathSegment>, String)],
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let borrowed = moved_paths
        .iter()
        .map(|(path, moved_type)| (path.as_slice(), moved_type.as_str()))
        .collect::<Vec<_>>();
    residuals::reconstruct(types, source, root_type, &borrowed, usize::MAX)
}

pub(crate) fn is_partial_affine_field_type(field_type: &CheckedUnitStructuralFieldType) -> bool {
    // Numeric restrictions remain on the retained field type for arithmetic
    // proofs, but do not introduce custody or executable cleanup. A bounded
    // integer is the same no-cleanup leaf as its raw scalar carrier when
    // reconstructing the untouched complement of a partial move.
    matches!(
        field_type,
        CheckedUnitStructuralFieldType::Structural { .. }
            | CheckedUnitStructuralFieldType::BoundedInteger(_)
            | CheckedUnitStructuralFieldType::RefinedScalar { .. }
            | CheckedUnitStructuralFieldType::ByteSequence(
                checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
            )
            | CheckedUnitStructuralFieldType::PredicateByteSequence {
                carrier: checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. },
                ..
            }
            | CheckedUnitStructuralFieldType::Scalar(
                PrimitiveType::Bool
                    | PrimitiveType::I8
                    | PrimitiveType::I16
                    | PrimitiveType::I32
                    | PrimitiveType::I64
                    | PrimitiveType::U8
                    | PrimitiveType::U16
                    | PrimitiveType::U32
                    | PrimitiveType::U64
                    | PrimitiveType::Addr
                    | PrimitiveType::F32
                    | PrimitiveType::F64
            )
    )
}
