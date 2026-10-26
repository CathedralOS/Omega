//! Optimizer module role: executable entrance. Exhaustive Terminal-operation routing into exact abstract-operation families.

mod arithmetic;
mod atomic_events;
mod boolean;
mod borrowed_windows;
mod calls;
mod effects;
mod ieee_float;
mod integer_bitwise;
mod integer_constants_and_relations;
mod integer_conversion;
mod primitive_projection;
mod routing;
mod shifts;
mod structural_establishment;
mod structural_leaf_copy;
mod structural_scalar_fields;
mod trapping_integer;

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use terminal_psi::TerminalMachine;

use super::{LoweredAffineLocal, StructuralLiteral};
use crate::lowering::LoweringError;
use semantic_vocabulary::ScalarType;

/// The exactly one row among `rows` that `accepts`, when a machine roster
/// names one coordinate once. Zero rows and competing rows both answer `None`:
/// without a unique row the join has no meaning, so ambiguity refuses like
/// absence and no caller has to re-check the roster itself.
pub(super) fn unique_row<T>(
    rows: impl IntoIterator<Item = T>,
    mut accepts: impl FnMut(&T) -> bool,
) -> Option<T> {
    let mut matches = rows.into_iter().filter(|row| accepts(row));
    let row = matches.next()?;
    matches.next().is_none().then_some(row)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &terminal_psi::Operation,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    module: &terminal_psi::TerminalModule,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    placed_view_plans: &[terminal_psi::TerminalPlacedViewPlan],
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
    operations: &mut Vec<AbstractOperation>,
) -> Result<(), LoweringError> {
    let lowered = routing::lower(
        operation,
        block,
        machine,
        module,
        structural_types,
        placed_view_plans,
        value_types,
        byte_sequence_literals,
        unit_affine_locals,
        lowered_unit_affine_locals,
    )?;
    operations.push(lowered);
    Ok(())
}
mod primitive_storage;
