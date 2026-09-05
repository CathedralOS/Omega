//! Exhaustive Terminal operation-kind dispatch to exact lowering families.

use std::collections::BTreeMap;

use abstract_operations::AbstractOperation;
use terminal_psi::{OperationKind, TerminalDynamicDispatchCatalog, TerminalMachine};

use super::{
    LoweredAffineLocal, ScalarType, StructuralLiteral, arithmetic, boolean, calls, effects,
    ieee_float, integer_bitwise, integer_constants_and_relations, integer_conversion, shifts,
    structural_establishment, structural_scalar_fields,
};
use crate::lowering::LoweringError;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower(
    operation: &terminal_psi::Operation,
    block: &terminal_psi::Block,
    machine: &TerminalMachine,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
    dynamic_dispatch: &TerminalDynamicDispatchCatalog,
    closed_conformance_applications: &[terminal_psi::ClosedConformanceApplication],
    retain_payloadless_for_optimization: bool,
    value_types: &BTreeMap<semantic_vocabulary::ValueId, ScalarType>,
    byte_sequence_literals: &[StructuralLiteral<'_>],
    unit_affine_locals: &[StructuralLiteral<'_>],
    lowered_unit_affine_locals: &mut Vec<LoweredAffineLocal>,
    lowered_byte_sequence_literals: &mut usize,
) -> Result<AbstractOperation, LoweringError> {
    match &operation.kind {
        OperationKind::StoreDynamicDescriptor { descriptor_ordinal } => {
            Ok(AbstractOperation::StoreDynamicDescriptor {
                psi_operation: operation.id,
                stored: calls::lower_stored_descriptor(
                    machine,
                    operation,
                    *descriptor_ordinal,
                    dynamic_dispatch,
                    closed_conformance_applications,
                )?,
            })
        }
        OperationKind::EstablishPayloadlessCase { .. }
        | OperationKind::EstablishByteSequenceLiteral { .. }
        | OperationKind::EstablishTrivialAffineLocal { .. }
        | OperationKind::EstablishAffineScalarRecord { .. } => structural_establishment::lower(
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
        | OperationKind::CallDynamicScalar { .. }
        | OperationKind::CallDynamicParameterScalar { .. }
        | OperationKind::CallDynamicUnit { .. }
        | OperationKind::CallDynamicParameterUnit { .. }
        | OperationKind::CallStructural { .. }
        | OperationKind::CallStructuralWithScalarArguments { .. }
        | OperationKind::BoundaryCall { .. } => calls::lower(
            operation,
            machine,
            dynamic_dispatch,
            closed_conformance_applications,
        ),
        OperationKind::PortWrite { .. } | OperationKind::WriteOnlyPrimitiveStore { .. } => {
            effects::lower(operation, machine, structural_types, value_types)
        }
        OperationKind::StructuralScalarFieldStore { .. }
        | OperationKind::IntegerStructuralField { .. } => {
            structural_scalar_fields::lower(operation, block, machine, structural_types)
        }
        OperationKind::Call { .. } => calls::lower(
            operation,
            machine,
            dynamic_dispatch,
            closed_conformance_applications,
        ),
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
    }
}
