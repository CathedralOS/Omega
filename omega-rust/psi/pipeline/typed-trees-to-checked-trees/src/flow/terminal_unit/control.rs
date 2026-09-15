//! Structural control and boundary-machine construction.
//!
//! This file owns the control plan entry point. `structural_unit_machine.rs`
//! builds a structural unit's control machine, `boundary_machine.rs` builds
//! boundary machines and their static requirements, `checked_machine.rs`
//! builds a checked machine and `call_results.rs` binds call results to unit
//! locals.

mod boundary_machine;
mod call_occurrences;
mod call_results;
mod checked_machine;
mod scalar_arrays;
pub(super) mod statement_sequence;
pub(super) mod structural_operands;
mod structural_unit_machine;

pub(crate) use boundary_machine::{build_boundary_machine, build_static_boundary_requirements};
pub(super) use call_occurrences::{outer_calls, outer_calls_before, tail_call};
pub(crate) use call_results::{
    bind_structural_call_result, checked_structural_result_type,
    checked_unit_structural_result_local,
};
pub(crate) use checked_machine::{build_checked_machine, build_checked_machine_with};
pub(crate) use structural_unit_machine::build_structural_unit_control_machine;

use super::{
    BTreeSet, CheckFacts, CheckedBoundaryMachineResultPlan, CheckedScalarExpression,
    CheckedScalarExpressionRole, CheckedStructuralAccess, CheckedStructuralScalarParameterPlan,
    CheckedStructuralUnitControlPlans, CheckedUnitEffectOperationPlan,
    CheckedUnitScalarResultBindingPlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralResultBindingPlan, ExpressionNode, MachineSupplyMode, Multiplicity,
    PermissionEventSource, StatementNode, SymbolHandle, TypeReferenceHandle, TypeReferenceNode,
    TypedTrees,
};
use crate::flow::terminal_unit::ShapeCollector;

pub(crate) fn build_checked_structural_unit_control_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> CheckedStructuralUnitControlPlans {
    let mut shapes = ShapeCollector::new(program);
    let machines = program
        .machines()
        .iter()
        .filter(|machine| machine.supply_mode == MachineSupplyMode::CheckedBody)
        .filter_map(|machine| {
            build_structural_unit_control_machine(program, facts, &mut shapes, machine)
        })
        .collect::<Vec<_>>();
    let retained = machines
        .iter()
        .flat_map(|machine| {
            std::iter::once(machine.attachment_type_identity.as_str()).chain(
                machine
                    .states
                    .iter()
                    .flat_map(|state| &state.structural_parameters)
                    .map(|parameter| parameter.type_identity.as_str()),
            )
        })
        .collect::<BTreeSet<_>>();
    shapes.retain_transitive(&retained);
    CheckedStructuralUnitControlPlans {
        structural_types: shapes.types.into_values().collect(),
        machines,
    }
}
