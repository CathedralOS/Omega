//! Optimizer module role: executable entrance. Exhaustive Terminal-operation routing into exact abstract-operation families.

mod arithmetic;
mod boolean;
mod calls;
mod effects;
mod ieee_float;
mod integer_bitwise;
mod integer_constants_and_relations;
mod integer_conversion;
mod routing;
mod shifts;
mod structural_establishment;
mod structural_scalar_fields;

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use terminal_psi::TerminalMachine;

use super::{LoweredAffineLocal, ScalarType, StructuralLiteral};
use crate::lowering::LoweringError;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &terminal_psi::Operation,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[terminal_psi::ClosedConformanceApplication],
    retain_payloadless_for_optimization: bool,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
    lowered_byte_sequence_literals: &mut usize,
    operations: &mut Vec<AbstractOperation>,
) -> Result<(), LoweringError> {
    let lowered = routing::lower(
        operation,
        block,
        machine,
        structural_types,
        dynamic_dispatch,
        closed_conformance_applications,
        retain_payloadless_for_optimization,
        value_types,
        byte_sequence_literals,
        unit_affine_locals,
        lowered_unit_affine_locals,
        lowered_byte_sequence_literals,
    )?;
    operations.push(lowered);
    Ok(())
}
