//! Residual reconstruction shares the ordinary operation sequencer.

use crate::execution::terminal_unit::cleanup::residuals;
use crate::execution::terminal_unit::types::ShapeCollector;
use crate::execution::terminal_unit::{
    BTreeMap, CheckFacts, CheckedPartialAffineUnitCleanupMachinePlan, CheckedUnitEffectPlans,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralTypePlan, PermissionEventKind,
    ScalarCalleeClosure, control,
};

pub(crate) fn build_partial_affine_unit_cleanup_machine(
    program: &dyn ::validation::CheckingInput,
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
                ScalarCalleeClosure,
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
