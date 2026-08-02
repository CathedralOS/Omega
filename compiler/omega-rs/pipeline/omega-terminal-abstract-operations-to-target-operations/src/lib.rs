#![forbid(unsafe_code)]

//! Resolve source-independent terminal Omega requirements into the first
//! target operation slice.

use std::collections::BTreeMap;

use omega_calling_conventions::{
    CallSignature, CallingPolicy, PlanDiagnostic, ValueLocation, ValuePlacement, ValueShape,
    evaluate_call_plan,
};
use omega_target::NativeTarget;
use omega_terminal_abstract_operations::{
    TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
    TerminalAbstractParameter,
};
use omega_terminal_target_operations::{
    TerminalPsiProvenance, TerminalScalarParameterLocation, TerminalTargetFunction,
    TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{IntegerType, IntegerValue, MachineId, ScalarType, ValueId};

pub fn lower_to_target_operations(
    plan: &TerminalAbstractOperationPlan,
    target: NativeTarget,
) -> Result<TerminalTargetOperationPlan, LoweringError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(LoweringError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalTargetOperationPlan {
        terminal_psi: plan.terminal_psi,
        target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| lower_function(function, target))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn lower_function(
    function: &TerminalAbstractFunction,
    target: NativeTarget,
) -> Result<TerminalTargetFunction, LoweringError> {
    let mut values = BTreeMap::new();
    let mut provenance = TerminalPsiProvenance::default();
    let mut returned = None;
    let signature = CallSignature {
        parameters: function
            .parameters
            .iter()
            .map(|parameter| scalar_shape(parameter.value, parameter.scalar_type, true))
            .collect::<Result<Vec<_>, _>>()?,
        result: Some(scalar_shape(
            function.result.value,
            function.result.scalar_type,
            false,
        )?),
    };
    let call_plan = evaluate_call_plan(CallingPolicy::native_for_target(target), &signature)
        .map_err(LoweringError::AbiPlan)?;
    if call_plan.parameters.len() != function.parameters.len() {
        return Err(LoweringError::AbiParameterCountMismatch {
            expected: function.parameters.len(),
            actual: call_plan.parameters.len(),
        });
    }
    for (parameter_index, (parameter, placement)) in function
        .parameters
        .iter()
        .zip(&call_plan.parameters)
        .enumerate()
    {
        insert_value(
            &mut values,
            parameter.value,
            KnownScalar::Parameter {
                scalar_type: parameter.scalar_type,
                parameter_index,
                location: scalar_parameter_location(parameter, placement)?,
            },
        )?;
    }

    for operation in &function.operations {
        if returned.is_some() {
            return Err(LoweringError::OperationAfterReturn(function.machine));
        }
        match operation {
            TerminalAbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } => {
                let ScalarType::Integer(integer_type) = scalar_type else {
                    return Err(LoweringError::IntegerConstantHasNonIntegerType(*result));
                };
                if !integer_type.admits(*value) {
                    return Err(LoweringError::IntegerConstantOutsideType(*result));
                }
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *integer_type,
                        value: *value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            } => {
                insert_value(&mut values, *result, KnownScalar::Boolean(*value))?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::WrappingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left = values
                    .get(left)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    if left.is_parameter() || right.is_parameter() {
                        return Err(LoweringError::RuntimeArithmeticNotYetSupported(*result));
                    }
                    return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::WrappingAddOperandTypeMismatch(*result));
                }
                let value = scalar_type
                    .wrapping_add(left, right)
                    .ok_or(LoweringError::WrappingAddOperandTypeMismatch(*result))?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::SaturatingIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
            } => {
                let left = values
                    .get(left)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*left))?;
                let right = values
                    .get(right)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*right))?;
                let (
                    KnownScalar::Integer {
                        scalar_type: left_type,
                        value: left,
                    },
                    KnownScalar::Integer {
                        scalar_type: right_type,
                        value: right,
                    },
                ) = (left, right)
                else {
                    if left.is_parameter() || right.is_parameter() {
                        return Err(LoweringError::RuntimeArithmeticNotYetSupported(*result));
                    }
                    return Err(LoweringError::SaturatingAddOperandTypeMismatch(*result));
                };
                if left_type != *scalar_type || right_type != *scalar_type {
                    return Err(LoweringError::SaturatingAddOperandTypeMismatch(*result));
                }
                let value = scalar_type
                    .saturating_add(left, right)
                    .ok_or(LoweringError::SaturatingAddOperandTypeMismatch(*result))?;
                insert_value(
                    &mut values,
                    *result,
                    KnownScalar::Integer {
                        scalar_type: *scalar_type,
                        value,
                    },
                )?;
                provenance.operations.push(*psi_operation);
            }
            TerminalAbstractOperation::Jump {
                psi_edge, bindings, ..
            } => {
                let transferred = bindings
                    .iter()
                    .map(|binding| {
                        let value = values
                            .get(&binding.argument)
                            .copied()
                            .ok_or(LoweringError::UnknownValue(binding.argument))?;
                        if binding.scalar_type != value.scalar_type() {
                            return Err(LoweringError::ValueTypeMismatch(binding.parameter));
                        }
                        Ok((binding.parameter, value))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                for (parameter, value) in transferred {
                    insert_value(&mut values, parameter, value)?;
                }
                provenance.edges.push(*psi_edge);
            }
            TerminalAbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
            } => {
                if *result != function.result.value || *scalar_type != function.result.scalar_type {
                    return Err(LoweringError::FunctionResultMismatch(function.machine));
                }
                let returned_value = values
                    .get(value)
                    .copied()
                    .ok_or(LoweringError::UnknownValue(*value))?;
                if *scalar_type != returned_value.scalar_type() {
                    return Err(LoweringError::ValueTypeMismatch(*result));
                }
                provenance.edges.push(*psi_edge);
                returned = Some(match returned_value {
                    KnownScalar::Boolean(boolean) => {
                        TerminalTargetOperation::ReturnBooleanImmediate {
                            psi_edge: *psi_edge,
                            source_value: *value,
                            value: boolean,
                        }
                    }
                    KnownScalar::Integer {
                        scalar_type,
                        value: integer,
                    } => TerminalTargetOperation::ReturnIntegerImmediate {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        value: integer,
                    },
                    KnownScalar::Parameter {
                        scalar_type: ScalarType::Boolean,
                        parameter_index,
                        location,
                    } => TerminalTargetOperation::ReturnBooleanParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        parameter_index,
                        location,
                    },
                    KnownScalar::Parameter {
                        scalar_type: ScalarType::Integer(scalar_type),
                        parameter_index,
                        location,
                    } => TerminalTargetOperation::ReturnIntegerParameter {
                        psi_edge: *psi_edge,
                        source_value: *value,
                        scalar_type,
                        parameter_index,
                        location,
                    },
                });
            }
        }
    }

    Ok(TerminalTargetFunction {
        machine: function.machine,
        provenance,
        operation: returned.ok_or(LoweringError::FunctionHasNoReturn(function.machine))?,
    })
}

fn scalar_shape(
    value: ValueId,
    scalar_type: ScalarType,
    require_native_parameter: bool,
) -> Result<ValueShape, LoweringError> {
    let bytes = match scalar_type {
        ScalarType::Boolean => 1,
        ScalarType::Integer(integer_type) => {
            let bits = integer_type.bits();
            if require_native_parameter && !matches!(bits, 8 | 16 | 32 | 64) {
                return Err(LoweringError::ParameterWidthNotNativelySupported { value, bits });
            }
            bits.div_ceil(8)
        }
    };
    Ok(ValueShape::integer(bytes, bytes.next_power_of_two().min(8)))
}

fn scalar_parameter_location(
    parameter: &TerminalAbstractParameter,
    placement: &ValuePlacement,
) -> Result<TerminalScalarParameterLocation, LoweringError> {
    let expected_bytes = scalar_shape(parameter.value, parameter.scalar_type, true)?.byte_size;
    match placement.locations.as_slice() {
        [
            ValueLocation::Register {
                register,
                value_byte_offset: 0,
                byte_size,
            },
        ] if *byte_size == expected_bytes => {
            Ok(TerminalScalarParameterLocation::Register(*register))
        }
        [
            ValueLocation::Stack {
                stack_byte_offset,
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] if *byte_size == expected_bytes => Ok(TerminalScalarParameterLocation::IncomingStack {
            byte_offset: *stack_byte_offset,
        }),
        _ => Err(LoweringError::UnsupportedScalarParameterPlacement(
            parameter.value,
        )),
    }
}

fn insert_value(
    values: &mut BTreeMap<ValueId, KnownScalar>,
    id: ValueId,
    value: KnownScalar,
) -> Result<(), LoweringError> {
    if values.insert(id, value).is_some() {
        return Err(LoweringError::DuplicateValue(id));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KnownScalar {
    Boolean(bool),
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    Parameter {
        scalar_type: ScalarType,
        parameter_index: usize,
        location: TerminalScalarParameterLocation,
    },
}

impl KnownScalar {
    const fn scalar_type(self) -> ScalarType {
        match self {
            Self::Boolean(_) => ScalarType::Boolean,
            Self::Integer { scalar_type, .. } => ScalarType::Integer(scalar_type),
            Self::Parameter { scalar_type, .. } => scalar_type,
        }
    }

    const fn is_parameter(self) -> bool {
        matches!(self, Self::Parameter { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoweringError {
    EntryFunctionMissing(MachineId),
    OperationAfterReturn(MachineId),
    FunctionHasNoReturn(MachineId),
    FunctionResultMismatch(MachineId),
    DuplicateValue(ValueId),
    UnknownValue(ValueId),
    ValueTypeMismatch(ValueId),
    IntegerConstantHasNonIntegerType(ValueId),
    IntegerConstantOutsideType(ValueId),
    WrappingAddOperandTypeMismatch(ValueId),
    SaturatingAddOperandTypeMismatch(ValueId),
    RuntimeArithmeticNotYetSupported(ValueId),
    ParameterWidthNotNativelySupported { value: ValueId, bits: u16 },
    UnsupportedScalarParameterPlacement(ValueId),
    AbiPlan(PlanDiagnostic),
    AbiParameterCountMismatch { expected: usize, actual: usize },
}

impl std::fmt::Display for LoweringError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for LoweringError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_terminal_abstract_operations::{
        TerminalAbstractFunction, TerminalAbstractOperation, TerminalAbstractOperationPlan,
        TerminalAbstractParameter, TerminalAbstractResult,
    };
    use omega_terminal_target_operations::MachineRegister;
    use psi_core::{BlockId, EdgeId};
    use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};

    #[test]
    fn refuses_a_return_whose_value_was_never_materialized() {
        let machine = MachineId::new(1).expect("machine");
        let unknown = ValueId::new(1).expect("unknown value");
        let result = ValueId::new(2).expect("result");
        let i32_type = IntegerType::new(psi_core::IntegerSign::Signed, 32).expect("i32");
        let plan = TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            functions: vec![TerminalAbstractFunction {
                machine,
                entry: BlockId::new(1).expect("block"),
                parameters: Vec::new(),
                result: TerminalAbstractResult {
                    value: result,
                    scalar_type: ScalarType::Integer(i32_type),
                },
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    result,
                    value: unknown,
                    scalar_type: ScalarType::Integer(i32_type),
                }],
            }],
        };

        assert_eq!(
            lower_to_target_operations(&plan, NativeTarget::linux_x64()),
            Err(LoweringError::UnknownValue(unknown))
        );
    }

    #[test]
    fn selects_native_register_and_stack_locations_for_runtime_parameters() {
        let register_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rcx),
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            ),
        ];
        for (target, expected) in register_cases {
            let lowered = lower_to_target_operations(&parameter_return_plan(1), target).unwrap();
            assert!(matches!(
                lowered.functions[0].operation,
                TerminalTargetOperation::ReturnIntegerParameter {
                    parameter_index: 0,
                    location,
                    ..
                } if location == expected
            ));
        }

        let stack_cases = [
            (
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
            ),
            (
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 64 },
            ),
            (
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ];
        for (target, expected) in stack_cases {
            let lowered = lower_to_target_operations(&parameter_return_plan(9), target).unwrap();
            assert!(matches!(
                lowered.functions[0].operation,
                TerminalTargetOperation::ReturnIntegerParameter {
                    parameter_index: 8,
                    location,
                    ..
                } if location == expected
            ));
        }
    }

    #[test]
    fn runtime_parameter_arithmetic_refuses_until_its_target_slice_exists() {
        let mut plan = parameter_return_plan(2);
        let function = &mut plan.functions[0];
        let sum = ValueId::new(50).expect("sum");
        let scalar_type = match function.result.scalar_type {
            ScalarType::Integer(integer) => integer,
            ScalarType::Boolean => unreachable!("fixture is integer"),
        };
        function.operations.insert(
            0,
            TerminalAbstractOperation::WrappingIntegerAdd {
                psi_operation: psi_core::OperationId::new(50).expect("operation"),
                result: sum,
                scalar_type,
                left: function.parameters[0].value,
                right: function.parameters[1].value,
            },
        );
        let TerminalAbstractOperation::Return { value, .. } = &mut function.operations[1] else {
            unreachable!("fixture ends in return")
        };
        *value = sum;

        assert_eq!(
            lower_to_target_operations(&plan, NativeTarget::host()),
            Err(LoweringError::RuntimeArithmeticNotYetSupported(sum))
        );
    }

    #[test]
    fn lowers_a_boolean_runtime_parameter_with_its_selected_abi_location() {
        let mut plan = parameter_return_plan(1);
        let function = &mut plan.functions[0];
        function.parameters[0].scalar_type = ScalarType::Boolean;
        function.result.scalar_type = ScalarType::Boolean;
        let TerminalAbstractOperation::Return { scalar_type, .. } = &mut function.operations[0]
        else {
            unreachable!("fixture ends in return")
        };
        *scalar_type = ScalarType::Boolean;

        let lowered = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
        assert!(matches!(
            lowered.functions[0].operation,
            TerminalTargetOperation::ReturnBooleanParameter {
                parameter_index: 0,
                location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                ..
            }
        ));
    }

    fn parameter_return_plan(parameter_count: usize) -> TerminalAbstractOperationPlan {
        let machine = MachineId::new(10).expect("machine");
        let result = ValueId::new(100).expect("result");
        let integer = IntegerType::new(psi_core::IntegerSign::Unsigned, 8).expect("u8");
        let scalar_type = ScalarType::Integer(integer);
        let parameters = (0..parameter_count)
            .map(|index| TerminalAbstractParameter {
                value: ValueId::new(10 + index as u64).expect("parameter"),
                scalar_type,
            })
            .collect::<Vec<_>>();
        let returned = parameters.last().expect("fixture has parameters").value;
        TerminalAbstractOperationPlan {
            terminal_psi: identity(),
            entry: machine,
            functions: vec![TerminalAbstractFunction {
                machine,
                entry: BlockId::new(10).expect("block"),
                parameters,
                result: TerminalAbstractResult {
                    value: result,
                    scalar_type,
                },
                operations: vec![TerminalAbstractOperation::Return {
                    psi_edge: EdgeId::new(10).expect("edge"),
                    result,
                    value: returned,
                    scalar_type,
                }],
            }],
        }
    }

    fn identity() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            semantic_version: SemanticVersion::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        }
    }
}
