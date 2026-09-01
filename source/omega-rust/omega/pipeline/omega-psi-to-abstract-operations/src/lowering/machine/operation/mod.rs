//! Optimizer module role: executable entrance. Exhaustive Terminal-operation routing into exact abstract-operation families.

mod arithmetic;
mod boolean;
mod calls;
mod effects;
mod ieee_float;
mod integer_bitwise;
mod integer_constants_and_relations;
mod integer_conversion;
mod shifts;
mod structural_establishment;
mod structural_scalar_fields;

use std::collections::BTreeMap;

use omega_abstract_operations::AbstractOperation;
use psi_terminal::{OperationKind, TerminalMachine};

use super::{LoweredAffineLocal, ScalarType, StructuralLiteral};
use crate::lowering::LoweringError;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_operation(
    operation: &psi_terminal::Operation,
    block: &psi_terminal::Block,
    machine: &TerminalMachine,
    structural_types: &[psi_terminal::StructuralTypeDeclaration],
    retain_payloadless_for_optimization: bool,
    value_types: &BTreeMap<psi_core::ValueId, ScalarType>,
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
    lowered_byte_sequence_literals: &mut usize,
    operations: &mut Vec<AbstractOperation>,
) -> Result<(), LoweringError> {
    let lowered = match &operation.kind {
        OperationKind::EstablishPayloadlessCase { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. } => structural_establishment::lower(
            operation,
            block,
            machine,
            structural_types,
            retain_payloadless_for_optimization,
            byte_sequence_literals,
            unit_affine_locals,
            lowered_unit_affine_locals,
            lowered_byte_sequence_literals,
        ),
        OperationKind::CallUnit { .. }
        | OperationKind::CallStructuralScalar { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::BoundaryCall { .. } => calls::lower(operation, machine),
        OperationKind::PortWrite { .. } | OperationKind::WriteOnlyPrimitiveStore { .. } => {
            effects::lower(operation, machine, structural_types, value_types)
        }
        OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::IntegerStructuralField { .. } => {
            structural_scalar_fields::lower(operation, block, machine, structural_types)
        }
        OperationKind::Call { .. } => calls::lower(operation, machine),
        OperationKind::IntegerConstant { .. } => integer_constants_and_relations::lower(operation),
        OperationKind::IeeeFloatConstant { .. }
        | OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. } => ieee_float::lower(operation),
        OperationKind::BooleanConstant { .. }
        | OperationKind::BooleanStructuralField { .. }
        | OperationKind::BooleanNot { .. }
        | OperationKind::BooleanEqual { .. } => boolean::lower(operation),
        OperationKind::IntegerEqual { .. }
        | OperationKind::IntegerLessThan { .. }
        | OperationKind::IntegerLessOrEqual { .. } => {
            integer_constants_and_relations::lower(operation)
        }
        OperationKind::IntegerBitwiseNot { .. } => integer_bitwise::lower(operation),
        OperationKind::IntegerWiden { .. } | OperationKind::IntegerExactCast { .. } => {
            integer_conversion::lower(operation, value_types)
        }
        OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. } => integer_bitwise::lower(operation),
        OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. }
        | OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. } => shifts::lower(operation, value_types),
        OperationKind::ExactIntegerAdd { .. }
        | OperationKind::WrappingIntegerAdd { .. }
        | OperationKind::ExactIntegerSubtract { .. }
        | OperationKind::WrappingIntegerSubtract { .. }
        | OperationKind::ExactIntegerMultiply { .. }
        | OperationKind::WrappingIntegerMultiply { .. }
        | OperationKind::ExactIntegerDivide { .. }
        | OperationKind::ExactIntegerRemainder { .. }
        | OperationKind::WrappingIntegerDivide { .. }
        | OperationKind::WrappingIntegerRemainder { .. }
        | OperationKind::SaturatingIntegerDivide { .. }
        | OperationKind::SaturatingIntegerRemainder { .. }
        | OperationKind::SaturatingIntegerAdd { .. }
        | OperationKind::SaturatingIntegerSubtract { .. }
        | OperationKind::SaturatingIntegerMultiply { .. } => arithmetic::lower(operation),
    }?;
    operations.push(lowered);
    Ok(())
}
