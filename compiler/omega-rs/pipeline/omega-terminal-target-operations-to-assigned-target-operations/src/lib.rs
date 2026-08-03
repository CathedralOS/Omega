#![forbid(unsafe_code)]

//! Assign concrete register and stack homes to clean terminal-Psi target
//! operations before machine emission.

use std::collections::BTreeMap;

use omega_target::Architecture;
use omega_terminal_assigned_target_operations::{
    TerminalAssignedFunction, TerminalAssignedIntegerExpression, TerminalAssignedOperation,
    TerminalAssignedOperationPlan, TerminalAssignedScalarLocation, TerminalEntryRegisterSpill,
    TerminalExpressionFrame,
};
use omega_terminal_target_operations::{
    MachineRegister, TerminalScalarParameterLocation, TerminalTargetFunction,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{MachineId, OperationId, ValueId};

pub fn assign_registers(
    plan: &TerminalTargetOperationPlan,
) -> Result<TerminalAssignedOperationPlan, AssignmentError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(AssignmentError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalAssignedOperationPlan {
        terminal_psi: plan.terminal_psi,
        target: plan.target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| assign_function(function, plan.target.architecture))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn assign_function(
    function: &TerminalTargetFunction,
    architecture: Architecture,
) -> Result<TerminalAssignedFunction, AssignmentError> {
    let operation = match &function.operation {
        TerminalTargetOperation::ReturnIntegerImmediate {
            psi_edge,
            source_value,
            scalar_type,
            value,
        } => TerminalAssignedOperation::ReturnIntegerImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            value: *value,
        },
        TerminalTargetOperation::ReturnBooleanImmediate {
            psi_edge,
            source_value,
            value,
        } => TerminalAssignedOperation::ReturnBooleanImmediate {
            psi_edge: *psi_edge,
            source_value: *source_value,
            value: *value,
        },
        TerminalTargetOperation::ReturnIntegerParameter {
            psi_edge,
            source_value,
            scalar_type,
            parameter_index,
            location,
        } => TerminalAssignedOperation::ReturnIntegerParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            scalar_type: *scalar_type,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetOperation::ReturnBooleanParameter {
            psi_edge,
            source_value,
            parameter_index,
            location,
        } => TerminalAssignedOperation::ReturnBooleanParameter {
            psi_edge: *psi_edge,
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: assign_direct_location(*source_value, *location, architecture)?,
        },
        TerminalTargetOperation::ReturnIntegerExpression {
            psi_edge,
            source_value,
            scalar_type,
            expression,
        } => {
            let locations = expression_parameter_locations(expression)?;
            let (frame, assigned_locations) =
                assign_expression_locations(architecture, &locations)?;
            TerminalAssignedOperation::ReturnIntegerExpression {
                psi_edge: *psi_edge,
                source_value: *source_value,
                scalar_type: *scalar_type,
                frame,
                expression: assign_expression(expression, &assigned_locations)?,
            }
        }
    };
    Ok(TerminalAssignedFunction {
        machine: function.machine,
        provenance: function.provenance.clone(),
        operation,
    })
}

fn assign_direct_location(
    source_value: ValueId,
    location: TerminalScalarParameterLocation,
    architecture: Architecture,
) -> Result<TerminalAssignedScalarLocation, AssignmentError> {
    Ok(match location {
        TerminalScalarParameterLocation::Register(register) => {
            require_register_architecture(source_value, register, architecture)?;
            TerminalAssignedScalarLocation::Register(register)
        }
        TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
            TerminalAssignedScalarLocation::IncomingStack { byte_offset }
        }
    })
}

fn assign_expression_locations(
    architecture: Architecture,
    locations: &BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
) -> Result<
    (
        TerminalExpressionFrame,
        BTreeMap<usize, TerminalAssignedScalarLocation>,
    ),
    AssignmentError,
> {
    let mut register_spills = Vec::new();
    let mut assigned = BTreeMap::new();
    for (&parameter_index, &(source_value, location)) in locations {
        match location {
            TerminalScalarParameterLocation::Register(register) => {
                require_register_architecture(source_value, register, architecture)?;
                if architecture == Architecture::Aarch64 {
                    let byte_offset = u32::try_from(register_spills.len())
                        .ok()
                        .and_then(|count| count.checked_mul(8))
                        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
                    register_spills.push(TerminalEntryRegisterSpill {
                        source_value,
                        parameter_index,
                        register,
                        byte_offset,
                    });
                    assigned.insert(
                        parameter_index,
                        TerminalAssignedScalarLocation::FrameSpill { byte_offset },
                    );
                } else {
                    assigned.insert(
                        parameter_index,
                        TerminalAssignedScalarLocation::Register(register),
                    );
                }
            }
            TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
                assigned.insert(
                    parameter_index,
                    TerminalAssignedScalarLocation::IncomingStack { byte_offset },
                );
            }
        }
    }
    let used_bytes = u32::try_from(register_spills.len())
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    let byte_size = used_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(AssignmentError::ExpressionStackFrameNotEncodable)?;
    if byte_size > 0xfff {
        return Err(AssignmentError::ExpressionStackFrameNotEncodable);
    }
    Ok((
        TerminalExpressionFrame {
            byte_size,
            register_spills,
        },
        assigned,
    ))
}

fn assign_expression(
    expression: &TerminalTargetIntegerExpression,
    locations: &BTreeMap<usize, TerminalAssignedScalarLocation>,
) -> Result<TerminalAssignedIntegerExpression, AssignmentError> {
    fn binary(
        psi_operation: OperationId,
        left: &TerminalTargetIntegerExpression,
        right: &TerminalTargetIntegerExpression,
        locations: &BTreeMap<usize, TerminalAssignedScalarLocation>,
        constructor: fn(
            OperationId,
            Box<TerminalAssignedIntegerExpression>,
            Box<TerminalAssignedIntegerExpression>,
        ) -> TerminalAssignedIntegerExpression,
    ) -> Result<TerminalAssignedIntegerExpression, AssignmentError> {
        Ok(constructor(
            psi_operation,
            Box::new(assign_expression(left, locations)?),
            Box::new(assign_expression(right, locations)?),
        ))
    }
    match expression {
        TerminalTargetIntegerExpression::Immediate {
            source_value,
            value,
        } => Ok(TerminalAssignedIntegerExpression::Immediate {
            source_value: *source_value,
            value: *value,
        }),
        TerminalTargetIntegerExpression::Parameter {
            source_value,
            parameter_index,
            ..
        } => Ok(TerminalAssignedIntegerExpression::Parameter {
            source_value: *source_value,
            parameter_index: *parameter_index,
            location: *locations.get(parameter_index).ok_or(
                AssignmentError::ExpressionParameterAssignmentMissing {
                    value: *source_value,
                    parameter_index: *parameter_index,
                },
            )?,
        }),
        TerminalTargetIntegerExpression::WrappingAdd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingAdd {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingAdd {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingSubtract {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingSubtract {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingSubtract {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::WrappingMultiply {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            |psi_operation, left, right| TerminalAssignedIntegerExpression::WrappingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
        TerminalTargetIntegerExpression::SaturatingMultiply {
            psi_operation,
            left,
            right,
        } => binary(
            *psi_operation,
            left,
            right,
            locations,
            |psi_operation, left, right| TerminalAssignedIntegerExpression::SaturatingMultiply {
                psi_operation,
                left,
                right,
            },
        ),
    }
}

fn expression_parameter_locations(
    expression: &TerminalTargetIntegerExpression,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, AssignmentError> {
    fn collect(
        expression: &TerminalTargetIntegerExpression,
        locations: &mut BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
    ) -> Result<(), AssignmentError> {
        match expression {
            TerminalTargetIntegerExpression::Immediate { .. } => {}
            TerminalTargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            } => {
                if let Some((_, established)) = locations.get(parameter_index) {
                    if established != location {
                        return Err(AssignmentError::ExpressionParameterLocationConflict {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        });
                    }
                } else {
                    locations.insert(*parameter_index, (*source_value, *location));
                }
            }
            TerminalTargetIntegerExpression::WrappingAdd { left, right, .. }
            | TerminalTargetIntegerExpression::SaturatingAdd { left, right, .. }
            | TerminalTargetIntegerExpression::WrappingSubtract { left, right, .. }
            | TerminalTargetIntegerExpression::SaturatingSubtract { left, right, .. }
            | TerminalTargetIntegerExpression::WrappingMultiply { left, right, .. }
            | TerminalTargetIntegerExpression::SaturatingMultiply { left, right, .. } => {
                collect(left, locations)?;
                collect(right, locations)?;
            }
        }
        Ok(())
    }
    let mut locations = BTreeMap::new();
    collect(expression, &mut locations)?;
    Ok(locations)
}

fn require_register_architecture(
    value: ValueId,
    register: MachineRegister,
    architecture: Architecture,
) -> Result<(), AssignmentError> {
    let matches = match architecture {
        Architecture::Aarch64 => matches!(register, MachineRegister::Aarch64X(0..=30)),
        Architecture::X86_64 => matches!(
            register,
            MachineRegister::X86Rax
                | MachineRegister::X86Rcx
                | MachineRegister::X86Rdx
                | MachineRegister::X86Rbx
                | MachineRegister::X86Rsp
                | MachineRegister::X86Rbp
                | MachineRegister::X86Rsi
                | MachineRegister::X86Rdi
                | MachineRegister::X86R8
                | MachineRegister::X86R9
                | MachineRegister::X86R10
                | MachineRegister::X86R11
                | MachineRegister::X86R12
                | MachineRegister::X86R13
                | MachineRegister::X86R14
                | MachineRegister::X86R15
        ),
    };
    if matches {
        Ok(())
    } else {
        Err(AssignmentError::ParameterRegisterArchitectureMismatch {
            value,
            register,
            architecture,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignmentError {
    EntryFunctionMissing(MachineId),
    ParameterRegisterArchitectureMismatch {
        value: ValueId,
        register: MachineRegister,
        architecture: Architecture,
    },
    ExpressionParameterLocationConflict {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionParameterAssignmentMissing {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionStackFrameNotEncodable,
}

impl std::fmt::Display for AssignmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for AssignmentError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::NativeTarget;
    use omega_terminal_assigned_target_operations::{
        TerminalAssignedIntegerExpression, TerminalAssignedOperation,
        TerminalAssignedScalarLocation,
    };
    use omega_terminal_target_operations::{
        TerminalPsiProvenance, TerminalTargetFunction, TerminalTargetIntegerExpression,
        TerminalTargetOperation,
    };
    use psi_core::{EdgeId, IntegerSign, IntegerType, OperationId};
    use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};

    #[test]
    fn aarch64_expression_registers_receive_stable_frame_spills() {
        let plan = expression_plan(
            NativeTarget::linux_arm64(),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        );
        let assigned = assign_registers(&plan).expect("assign AArch64 homes");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 16);
        assert_eq!(frame.register_spills.len(), 2);
        assert_eq!(frame.register_spills[0].byte_offset, 0);
        assert_eq!(frame.register_spills[1].byte_offset, 8);
        let TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 0 },
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::FrameSpill { byte_offset: 8 },
                ..
            }
        ));
    }

    #[test]
    fn x86_expression_registers_remain_explicit_without_a_frame() {
        let plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
        );
        let assigned = assign_registers(&plan).expect("assign x86-64 homes");
        let TerminalAssignedOperation::ReturnIntegerExpression {
            frame, expression, ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must remain an expression")
        };
        assert_eq!(frame.byte_size, 0);
        assert!(frame.register_spills.is_empty());
        let TerminalAssignedIntegerExpression::WrappingAdd { left, right, .. } = expression else {
            panic!("fixture must remain wrapping addition")
        };
        assert!(matches!(
            left.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::Register(MachineRegister::X86Rdi),
                ..
            }
        ));
        assert!(matches!(
            right.as_ref(),
            TerminalAssignedIntegerExpression::Parameter {
                location: TerminalAssignedScalarLocation::IncomingStack { byte_offset: 16 },
                ..
            }
        ));
    }

    #[test]
    fn repeated_parameter_location_drift_rejects_before_emission() {
        let mut plan = expression_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
        );
        let TerminalTargetOperation::ReturnIntegerExpression { expression, .. } =
            &mut plan.functions[0].operation
        else {
            panic!("fixture must contain an expression")
        };
        let TerminalTargetIntegerExpression::WrappingAdd { right, .. } = expression else {
            panic!("fixture must contain wrapping addition")
        };
        let TerminalTargetIntegerExpression::Parameter {
            parameter_index, ..
        } = right.as_mut()
        else {
            panic!("right operand must be a parameter")
        };
        *parameter_index = 0;
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::ExpressionParameterLocationConflict {
                parameter_index: 0,
                ..
            })
        ));
    }

    #[test]
    fn cross_architecture_register_rejects_during_assignment() {
        let plan = expression_plan(
            NativeTarget::linux_arm64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
        );
        assert!(matches!(
            assign_registers(&plan),
            Err(AssignmentError::ParameterRegisterArchitectureMismatch {
                architecture: Architecture::Aarch64,
                ..
            })
        ));
    }

    fn expression_plan(
        target: NativeTarget,
        left_location: TerminalScalarParameterLocation,
        right_location: TerminalScalarParameterLocation,
    ) -> TerminalTargetOperationPlan {
        TerminalTargetOperationPlan {
            terminal_psi: TerminalPsiIdentity {
                semantic_version: SemanticVersion::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([3; 32]),
            },
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    expression: TerminalTargetIntegerExpression::WrappingAdd {
                        psi_operation: OperationId::new(1).expect("operation"),
                        left: Box::new(TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(1).expect("left"),
                            parameter_index: 0,
                            location: left_location,
                        }),
                        right: Box::new(TerminalTargetIntegerExpression::Parameter {
                            source_value: ValueId::new(2).expect("right"),
                            parameter_index: 1,
                            location: right_location,
                        }),
                    },
                },
            }],
        }
    }
}
