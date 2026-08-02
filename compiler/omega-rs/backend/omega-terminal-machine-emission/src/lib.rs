#![forbid(unsafe_code)]

//! Machine-code emission for the first source-independent terminal-Psi target
//! operation slice.

use omega_target::Architecture;
use omega_terminal_machine_code::{TerminalMachineCodeFunction, TerminalMachineCodePlan};
use omega_terminal_target_operations::{
    MachineRegister, TerminalScalarParameterLocation, TerminalTargetFunction,
    TerminalTargetOperation, TerminalTargetOperationPlan,
};
use psi_core::{IntegerSign, IntegerType, IntegerValue, MachineId, ValueId};

pub fn emit_machine_code(
    plan: &TerminalTargetOperationPlan,
) -> Result<TerminalMachineCodePlan, EmissionError> {
    if !plan
        .functions
        .iter()
        .any(|function| function.machine == plan.entry)
    {
        return Err(EmissionError::EntryFunctionMissing(plan.entry));
    }
    Ok(TerminalMachineCodePlan {
        terminal_psi: plan.terminal_psi,
        target: plan.target,
        entry: plan.entry,
        functions: plan
            .functions
            .iter()
            .map(|function| emit_function(function, plan.target.architecture))
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn emit_function(
    function: &TerminalTargetFunction,
    architecture: Architecture,
) -> Result<TerminalMachineCodeFunction, EmissionError> {
    let bytes = match function.operation {
        TerminalTargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type,
            value,
            ..
        } => {
            let bits = integer_bits(source_value, scalar_type, value)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_return(scalar_type, bits),
                Architecture::X86_64 => emit_x86_64_return(scalar_type, bits),
            }
        }
        TerminalTargetOperation::ReturnBooleanImmediate { value, .. } => match architecture {
            Architecture::Aarch64 => emit_aarch64_boolean_return(value),
            Architecture::X86_64 => emit_x86_64_boolean_return(value),
        },
        TerminalTargetOperation::ReturnIntegerParameter {
            source_value,
            scalar_type,
            location,
            ..
        } => {
            require_native_integer_width(source_value, scalar_type)?;
            match architecture {
                Architecture::Aarch64 => {
                    emit_aarch64_parameter_return(source_value, scalar_type.bits() > 32, location)?
                }
                Architecture::X86_64 => {
                    emit_x86_64_parameter_return(source_value, scalar_type.bits() > 32, location)?
                }
            }
        }
        TerminalTargetOperation::ReturnBooleanParameter {
            source_value,
            location,
            ..
        } => match architecture {
            Architecture::Aarch64 => emit_aarch64_parameter_return(source_value, false, location)?,
            Architecture::X86_64 => emit_x86_64_parameter_return(source_value, false, location)?,
        },
    };
    Ok(TerminalMachineCodeFunction {
        machine: function.machine,
        provenance: function.provenance.clone(),
        bytes,
    })
}

fn emit_x86_64_boolean_return(value: bool) -> Vec<u8> {
    vec![0xb8, u8::from(value), 0, 0, 0, 0xc3] // mov eax, 0/1; ret
}

fn emit_aarch64_boolean_return(value: bool) -> Vec<u8> {
    let mov_w0 = 0x5280_0000_u32 | (u32::from(value) << 5);
    [mov_w0, 0xd65f_03c0]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

fn integer_bits(
    source: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
) -> Result<u64, EmissionError> {
    let width = require_native_integer_width(source, scalar_type)?;
    if !scalar_type.admits(value) {
        return Err(EmissionError::IntegerOutsideType(source));
    }
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    };
    let bits = match (scalar_type.sign(), value) {
        (IntegerSign::Signed, IntegerValue::Signed(value)) => value as u128 as u64,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => value as u64,
        _ => return Err(EmissionError::IntegerSignMismatch(source)),
    };
    Ok(bits & mask)
}

fn require_native_integer_width(
    source: ValueId,
    scalar_type: IntegerType,
) -> Result<u16, EmissionError> {
    let width = scalar_type.bits();
    if !matches!(width, 8 | 16 | 32 | 64) {
        return Err(EmissionError::IntegerWidthNotNativelySupported {
            value: source,
            bits: width,
        });
    }
    Ok(width)
}

fn emit_x86_64_parameter_return(
    source: ValueId,
    is_64: bool,
    location: TerminalScalarParameterLocation,
) -> Result<Vec<u8>, EmissionError> {
    let mut bytes = Vec::new();
    match location {
        TerminalScalarParameterLocation::Register(register) => {
            let register = x86_gpr_code(source, register)?;
            let rex = 0x40 | (u8::from(is_64) << 3) | (((register >> 3) & 1) << 2);
            if rex != 0x40 {
                bytes.push(rex);
            }
            bytes.push(0x89); // mov eax/rax, selected argument register
            bytes.push(0xc0 | ((register & 7) << 3));
        }
        TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
            let displacement = byte_offset.checked_add(8).ok_or(
                EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                },
            )?;
            if is_64 {
                bytes.push(0x48);
            }
            bytes.push(0x8b); // mov eax/rax, [rsp + displacement]
            if displacement <= i8::MAX as u32 {
                bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
            } else {
                bytes.extend_from_slice(&[0x84, 0x24]);
                bytes.extend_from_slice(&displacement.to_le_bytes());
            }
        }
    }
    bytes.push(0xc3);
    Ok(bytes)
}

fn x86_gpr_code(source: ValueId, register: MachineRegister) -> Result<u8, EmissionError> {
    Ok(match register {
        MachineRegister::X86Rax => 0,
        MachineRegister::X86Rcx => 1,
        MachineRegister::X86Rdx => 2,
        MachineRegister::X86Rbx => 3,
        MachineRegister::X86Rsp => 4,
        MachineRegister::X86Rbp => 5,
        MachineRegister::X86Rsi => 6,
        MachineRegister::X86Rdi => 7,
        MachineRegister::X86R8 => 8,
        MachineRegister::X86R9 => 9,
        MachineRegister::X86R10 => 10,
        MachineRegister::X86R11 => 11,
        MachineRegister::X86R12 => 12,
        MachineRegister::X86R13 => 13,
        MachineRegister::X86R14 => 14,
        MachineRegister::X86R15 => 15,
        MachineRegister::X86Xmm(_)
        | MachineRegister::Aarch64X(_)
        | MachineRegister::Aarch64V(_) => {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::X86_64,
            });
        }
    })
}

fn emit_aarch64_parameter_return(
    source: ValueId,
    is_64: bool,
    location: TerminalScalarParameterLocation,
) -> Result<Vec<u8>, EmissionError> {
    let instruction = match location {
        TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(register))
            if register < 31 =>
        {
            if register == 0 {
                None
            } else {
                let base = if is_64 { 0xaa00_03e0 } else { 0x2a00_03e0 };
                Some(base | (u32::from(register) << 16))
            }
        }
        TerminalScalarParameterLocation::Register(register) => {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source,
                register,
                architecture: Architecture::Aarch64,
            });
        }
        TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
            let scale = if is_64 { 8 } else { 4 };
            if byte_offset % scale != 0 || byte_offset / scale > 0xfff {
                return Err(EmissionError::IncomingStackOffsetNotEncodable {
                    value: source,
                    byte_offset,
                });
            }
            let base = if is_64 { 0xf940_0000 } else { 0xb940_0000 };
            Some(base | ((byte_offset / scale) << 10) | (31 << 5))
        }
    };
    Ok(instruction
        .into_iter()
        .chain([0xd65f_03c0])
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn emit_x86_64_return(scalar_type: IntegerType, bits: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    if scalar_type.bits() <= 32 {
        bytes.push(0xb8); // mov eax, imm32
        bytes.extend_from_slice(&(bits as u32).to_le_bytes());
    } else {
        bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
        bytes.extend_from_slice(&bits.to_le_bytes());
    }
    bytes.push(0xc3); // ret
    bytes
}

fn emit_aarch64_return(scalar_type: IntegerType, bits: u64) -> Vec<u8> {
    let is_64 = scalar_type.bits() > 32;
    let chunk_count = if is_64 { 4 } else { 2 };
    let movz_base = if is_64 { 0xd280_0000 } else { 0x5280_0000 };
    let movk_base = if is_64 { 0xf280_0000 } else { 0x7280_0000 };
    let mut instructions = Vec::new();
    for chunk in 0..chunk_count {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { movz_base } else { movk_base };
            instructions.push(base | ((chunk as u32) << 21) | (immediate << 5));
        }
    }
    instructions.push(0xd65f_03c0); // ret x30
    instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmissionError {
    IntegerWidthNotNativelySupported {
        value: ValueId,
        bits: u16,
    },
    IntegerOutsideType(ValueId),
    IntegerSignMismatch(ValueId),
    ParameterRegisterArchitectureMismatch {
        value: ValueId,
        register: MachineRegister,
        architecture: Architecture,
    },
    IncomingStackOffsetNotEncodable {
        value: ValueId,
        byte_offset: u32,
    },
    EntryFunctionMissing(MachineId),
}

impl std::fmt::Display for EmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for EmissionError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_target::NativeTarget;
    use omega_terminal_target_operations::{
        TerminalPsiProvenance, TerminalTargetFunction, TerminalTargetOperation,
        TerminalTargetOperationPlan,
    };
    use psi_core::{EdgeId, MachineId};
    use psi_terminal::{SemanticFingerprint, SemanticVersion, TerminalPsiIdentity};

    fn plan(target: NativeTarget) -> TerminalTargetOperationPlan {
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerImmediate {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(1).expect("value"),
                    scalar_type: i32_type,
                    value: IntegerValue::Signed(7),
                },
            }],
        }
    }

    #[test]
    fn emits_x86_64_return_immediate() {
        let emitted = emit_machine_code(&plan(NativeTarget::linux_x64())).expect("emit");
        assert_eq!(emitted.functions[0].bytes, [0xb8, 7, 0, 0, 0, 0xc3]);
    }

    #[test]
    fn emits_aarch64_return_immediate() {
        let emitted = emit_machine_code(&plan(NativeTarget::linux_arm64())).expect("emit");
        assert_eq!(
            emitted.functions[0].bytes,
            [0xe0, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_canonical_boolean_returns_for_both_architectures() {
        let boolean_plan = |target, value| TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnBooleanImmediate {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(1).expect("value"),
                    value,
                },
            }],
        };

        assert_eq!(
            emit_machine_code(&boolean_plan(NativeTarget::linux_x64(), true))
                .unwrap()
                .functions[0]
                .bytes,
            [0xb8, 1, 0, 0, 0, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&boolean_plan(NativeTarget::linux_arm64(), false))
                .unwrap()
                .functions[0]
                .bytes,
            [0x00, 0x00, 0x80, 0x52, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_selected_register_parameter_returns_for_all_native_policies() {
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x89, 0xf8, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::windows_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rcx),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x89, 0xc8, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xc0, 0x03, 0x5f, 0xd6]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x01, 0x2a, 0xc0, 0x03, 0x5f, 0xd6]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::Register(MachineRegister::X86R9),
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x4c, 0x89, 0xc8, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(3)),
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x03, 0xaa, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_selected_incoming_stack_parameter_returns_for_both_architectures() {
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x8b, 0x44, 0x24, 24, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
                false,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x40, 0xb9, 0xc0, 0x03, 0x5f, 0xd6]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_x64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 16 },
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0x48, 0x8b, 0x44, 0x24, 24, 0xc3]
        );
        assert_eq!(
            emit_machine_code(&parameter_plan(
                NativeTarget::linux_arm64(),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
                true,
            ))
            .unwrap()
            .functions[0]
                .bytes,
            [0xe0, 0x03, 0x40, 0xf9, 0xc0, 0x03, 0x5f, 0xd6]
        );
    }

    #[test]
    fn emits_a_canonical_boolean_parameter_return() {
        let mut plan = parameter_plan(
            NativeTarget::linux_x64(),
            TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
            false,
        );
        plan.functions[0].operation = TerminalTargetOperation::ReturnBooleanParameter {
            psi_edge: EdgeId::new(1).expect("edge"),
            source_value: ValueId::new(1).expect("value"),
            parameter_index: 0,
            location: TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
        };
        assert_eq!(
            emit_machine_code(&plan).unwrap().functions[0].bytes,
            [0x89, 0xf8, 0xc3]
        );
    }

    #[test]
    fn rejects_integer_width_without_a_native_scalar_realization() {
        let mut plan = plan(NativeTarget::linux_x64());
        let TerminalTargetOperation::ReturnIntegerImmediate {
            scalar_type, value, ..
        } = &mut plan.functions[0].operation
        else {
            panic!("integer fixture must contain an integer return")
        };
        *scalar_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        *value = IntegerValue::Signed(7);
        assert!(matches!(
            emit_machine_code(&plan),
            Err(EmissionError::IntegerWidthNotNativelySupported { bits: 128, .. })
        ));
    }

    fn parameter_plan(
        target: NativeTarget,
        location: TerminalScalarParameterLocation,
        is_64: bool,
    ) -> TerminalTargetOperationPlan {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, if is_64 { 64 } else { 8 })
            .expect("integer type");
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerParameter {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(1).expect("value"),
                    scalar_type,
                    parameter_index: 0,
                    location,
                },
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
