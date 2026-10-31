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
mod scalar_applications;
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
use semantic_vocabulary::{IntegerSign, IntegerType, OperationId, ScalarType};

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

/// The operation's scalar result, accepted only as an unsigned integer of
/// `bits` bits. An absent carrier and a mismatched width answer the caller's
/// own invalid-operation error, so the two spellings cannot drift apart.
fn unsigned_scalar(
    operation: &terminal_psi::Operation,
    bits: u16,
    invalid: fn(OperationId) -> LoweringError,
) -> Result<terminal_psi::ValueDeclaration, LoweringError> {
    let result = operation.result.scalar().ok_or(invalid(operation.id))?;
    if result.scalar_type.as_integer() != IntegerType::new(IntegerSign::Unsigned, bits).ok() {
        return Err(invalid(operation.id));
    }
    Ok(result)
}

/// The operation's structural result, cloned so each caller routes onward the
/// place and claim frontier it established.
fn structural_result(
    operation: &terminal_psi::Operation,
    invalid: fn(OperationId) -> LoweringError,
) -> Result<terminal_psi::StructuralOperationResult, LoweringError> {
    let result = operation.result.structural().cloned();
    result.ok_or(invalid(operation.id))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &terminal_psi::Operation,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    module: &terminal_psi::TerminalModule,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    scalar_applications: Option<&terminal_installation::AdmittedScalarApplications>,
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
        scalar_applications,
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
