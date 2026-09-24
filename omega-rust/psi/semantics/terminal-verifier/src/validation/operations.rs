//! Validates terminal operation operands against exact SSA value types.
//!
//! `validate_operation_operands` checks the owned array payloads a call
//! passes, then dispatches the operation kind to its family:
//! `storage_operands` (primitive and byte-sequence reads and stores),
//! `call_operands`, `scalar_operands` (casts, comparisons, bitwise logic
//! and shifts) and `arithmetic_operands`.

mod arithmetic_operands;
mod call_operands;
mod scalar_operands;
mod storage_operands;

use super::{
    BTreeMap, BTreeSet, BoundaryMachineDeclaration, MachineId, ModuleError, OperationId,
    OperationKind, ScalarType, StructuralAccess, TerminalMachine, TerminalModule, ValueId,
};
pub(super) fn validate_operation_operands(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    boundary_machines: &[BoundaryMachineDeclaration],
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    // Ordinary calls carry complete owned array values. Boundary presentation
    // and borrowed/projected uses cannot substitute opaque-place metadata for
    // the initialized payload. Existing borrowed fixed-array backing is separate.
    let structural_arguments = match &operation.kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    for argument in structural_arguments {
        if super::scalar::array::owned_payload_source(module, machine, argument.place)
            && (matches!(operation.kind, OperationKind::BoundaryCall { .. })
                || argument.access != StructuralAccess::Owned
                || !argument.path.is_empty()
                || !super::scalar::array::plain_return_source(module, machine, argument.place))
        {
            return Err(ModuleError::ScalarArrayResultMismatch(operation.id));
        }
    }
    match &operation.kind {
        OperationKind::EstablishScalarArray { .. } => {
            super::scalar::array::operands(module, machine, operation, value_types, defined)
        }
        OperationKind::EstablishScalarCase { .. } => {
            super::scalar::case::operands(module, machine, operation, value_types, defined)
        }
        OperationKind::EstablishRecord { .. } => {
            super::record::operands(module, machine, operation, value_types, defined)
        }
        OperationKind::EstablishStructuralCase { .. } => {
            super::structural::case::operands(module, machine, operation, value_types, defined)
        }
        OperationKind::StructuralByteSequenceFieldByteStore { .. } => {
            storage_operands::validate_structural_byte_sequence_field_byte_store(
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::ByteSequenceSubslice { .. } => {
            storage_operands::validate_byte_sequence_subslice(operation, value_types, defined)
        }
        OperationKind::StructuralByteSequenceFieldStore { .. } => {
            storage_operands::validate_structural_byte_sequence_field_store(
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::ByteSequenceWrite { .. } => {
            storage_operands::validate_byte_sequence_write(operation, value_types, defined)
        }
        OperationKind::ByteSequenceRead { .. } => {
            storage_operands::validate_byte_sequence_read(operation, value_types, defined)
        }
        OperationKind::ElementViewRead { .. } => {
            storage_operands::validate_element_view_read(operation, value_types, defined)
        }
        OperationKind::ElementViewSubslice { .. } => {
            storage_operands::validate_element_view_subslice(operation, value_types, defined)
        }
        OperationKind::IeeeFloatCompare { .. } => {
            scalar_operands::validate_ieee_float_compare(operation, value_types, defined)
        }
        OperationKind::NearestIeeeFloatFusedMultiplyAdd { .. } => {
            scalar_operands::validate_nearest_ieee_float_fused_multiply_add(
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::EstablishPrimitiveLocal { .. } => {
            storage_operands::validate_establish_primitive_local(
                module,
                machine,
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::PrimitiveScalarRead { .. } => {
            storage_operands::validate_primitive_scalar_read(module, machine, operation)
        }
        OperationKind::WriteOnlyPrimitiveStore { .. } => {
            storage_operands::validate_write_only_primitive_store(
                module,
                machine,
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::StructuralScalarFieldStore { .. } => {
            storage_operands::validate_structural_scalar_field_store(
                module,
                machine,
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::Call { .. } => {
            call_operands::validate_call(operation, machines, value_types, defined)
        }
        OperationKind::CallUnit { .. } => {
            call_operands::validate_call_unit(operation, machines, value_types, defined)
        }
        OperationKind::CallStructuralScalar { .. } => {
            call_operands::validate_call_structural_scalar(
                operation,
                machines,
                value_types,
                defined,
            )
        }
        OperationKind::CallStructuralWithScalarArguments { .. } => {
            call_operands::validate_call_structural_with_scalar_arguments(
                operation,
                machines,
                value_types,
                defined,
            )
        }
        OperationKind::BoundaryCall { .. } => call_operands::validate_boundary_call(
            machine,
            operation,
            boundary_machines,
            value_types,
            defined,
        ),
        OperationKind::IntegerExactCast { .. } => {
            scalar_operands::validate_integer_exact_cast(operation, value_types, defined)
        }
        OperationKind::IntegerWiden { .. } => {
            scalar_operands::validate_integer_widen(operation, value_types, defined)
        }
        OperationKind::IntegerBitwiseNot { .. } => {
            scalar_operands::validate_integer_bitwise_not(operation, value_types, defined)
        }
        OperationKind::BooleanNot { .. } => {
            scalar_operands::validate_boolean_not(operation, value_types, defined)
        }
        OperationKind::BooleanEqual { .. } => {
            scalar_operands::validate_boolean_equal(operation, value_types, defined)
        }
        OperationKind::IntegerEqual { .. } => {
            scalar_operands::validate_integer_equal(operation, value_types, defined)
        }
        OperationKind::IntegerLessThan { .. } | OperationKind::IntegerLessOrEqual { .. } => {
            scalar_operands::validate_integer_comparison(operation, value_types, defined)
        }
        OperationKind::IntegerBitwiseAnd { .. }
        | OperationKind::IntegerBitwiseOr { .. }
        | OperationKind::IntegerBitwiseXor { .. } => {
            scalar_operands::validate_integer_bitwise(operation, value_types, defined)
        }
        OperationKind::WrappingIntegerShiftLeft { .. }
        | OperationKind::WrappingIntegerShiftRight { .. } => {
            scalar_operands::validate_wrapping_shift(operation, value_types, defined)
        }
        OperationKind::ExactIntegerShiftLeft { .. }
        | OperationKind::ExactIntegerShiftRight { .. } => {
            scalar_operands::validate_exact_shift(operation, value_types, defined)
        }
        OperationKind::ExactIntegerAdd { .. } => {
            arithmetic_operands::validate_exact_integer_add(operation, value_types, defined)
        }
        OperationKind::ExactIntegerSubtract { .. } => {
            arithmetic_operands::validate_exact_integer_subtract(operation, value_types, defined)
        }
        OperationKind::ExactIntegerMultiply { .. } => {
            arithmetic_operands::validate_exact_integer_multiply(operation, value_types, defined)
        }
        OperationKind::ExactIntegerDivide { .. } => {
            arithmetic_operands::validate_exact_integer_divide(operation, value_types, defined)
        }
        OperationKind::ExactIntegerRemainder { .. } => {
            arithmetic_operands::validate_exact_integer_remainder(operation, value_types, defined)
        }
        OperationKind::WrappingIntegerDivide { .. } => {
            arithmetic_operands::validate_wrapping_integer_divide(operation, value_types, defined)
        }
        OperationKind::WrappingIntegerRemainder { .. } => {
            arithmetic_operands::validate_wrapping_integer_remainder(
                operation,
                value_types,
                defined,
            )
        }
        OperationKind::SaturatingIntegerDivide { .. } => {
            arithmetic_operands::validate_saturating_integer_divide(operation, value_types, defined)
        }
        OperationKind::SaturatingIntegerRemainder { .. } => {
            arithmetic_operands::validate_saturating_integer_remainder(
                operation,
                value_types,
                defined,
            )
        }
        _ => arithmetic_operands::validate_binary_arithmetic(operation, value_types, defined),
    }
}

#[derive(Clone, Copy)]
enum ScalarCallKind {
    Ordinary,
    Boundary,
}

fn validate_call_arguments(
    operation: OperationId,
    arguments: &[ValueId],
    parameter_types: &[ScalarType],
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
    kind: ScalarCallKind,
) -> Result<(), ModuleError> {
    if arguments.len() != parameter_types.len() {
        return Err(match kind {
            ScalarCallKind::Ordinary => ModuleError::CallArgumentArityMismatch {
                operation,
                expected: parameter_types.len(),
                actual: arguments.len(),
            },
            ScalarCallKind::Boundary => ModuleError::BoundaryCallArgumentArityMismatch {
                operation,
                expected: parameter_types.len(),
                actual: arguments.len(),
            },
        });
    }
    for (argument, expected) in arguments.iter().zip(parameter_types) {
        if let Err(error) = require_defined(*argument, value_types, defined) {
            return Err(match (kind, error) {
                (ScalarCallKind::Boundary, ModuleError::UnknownValue(_)) => {
                    ModuleError::UnknownBoundaryCallArgument {
                        operation,
                        argument: *argument,
                    }
                }
                (ScalarCallKind::Boundary, ModuleError::ValueUsedBeforeDefinition(_))
                    if !value_types.contains_key(argument) =>
                {
                    ModuleError::UnknownBoundaryCallArgument {
                        operation,
                        argument: *argument,
                    }
                }
                (ScalarCallKind::Boundary, ModuleError::ValueUsedBeforeDefinition(_)) => {
                    ModuleError::BoundaryCallArgumentUsedBeforeDefinition {
                        operation,
                        argument: *argument,
                    }
                }
                (_, error) => error,
            });
        }
        let actual = value_types[argument];
        if actual != *expected {
            return Err(match kind {
                ScalarCallKind::Ordinary => ModuleError::CallArgumentTypeMismatch {
                    operation,
                    argument: *argument,
                    expected: *expected,
                    actual,
                },
                ScalarCallKind::Boundary => ModuleError::BoundaryCallArgumentTypeMismatch {
                    operation,
                    argument: *argument,
                    expected: *expected,
                    actual,
                },
            });
        }
    }
    Ok(())
}

pub(super) fn require_defined(
    value: ValueId,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    if !defined.contains(&value) {
        return Err(ModuleError::ValueUsedBeforeDefinition(value));
    }
    if !value_types.contains_key(&value) {
        return Err(ModuleError::UnknownValue(value));
    }
    Ok(())
}

enum ArithmeticOperandKind {
    WrappingAdd,
    SaturatingAdd,
    WrappingSubtract,
    SaturatingSubtract,
    WrappingMultiply,
    SaturatingMultiply,
}
