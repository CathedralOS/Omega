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

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &terminal_psi::Operation,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    module: &terminal_psi::TerminalModule,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
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
        value_types,
        byte_sequence_literals,
        unit_affine_locals,
        lowered_unit_affine_locals,
    )?;
    operations.push(lowered);
    Ok(())
}
mod primitive_storage;
