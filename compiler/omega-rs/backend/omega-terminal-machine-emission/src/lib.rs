#![forbid(unsafe_code)]

//! Machine-code emission for the first source-independent terminal-Psi target
//! operation slice.

use std::collections::BTreeMap;

use omega_target::Architecture;
use omega_terminal_machine_code::{TerminalMachineCodeFunction, TerminalMachineCodePlan};
use omega_terminal_target_operations::{
    MachineRegister, TerminalScalarParameterLocation, TerminalTargetFunction,
    TerminalTargetIntegerExpression, TerminalTargetOperation, TerminalTargetOperationPlan,
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
    let bytes = match &function.operation {
        TerminalTargetOperation::ReturnIntegerImmediate {
            source_value,
            scalar_type,
            value,
            ..
        } => {
            let bits = integer_bits(*source_value, *scalar_type, *value)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_return(*scalar_type, bits),
                Architecture::X86_64 => emit_x86_64_return(*scalar_type, bits),
            }
        }
        TerminalTargetOperation::ReturnBooleanImmediate { value, .. } => match architecture {
            Architecture::Aarch64 => emit_aarch64_boolean_return(*value),
            Architecture::X86_64 => emit_x86_64_boolean_return(*value),
        },
        TerminalTargetOperation::ReturnIntegerParameter {
            source_value,
            scalar_type,
            location,
            ..
        } => {
            require_native_integer_width(*source_value, *scalar_type)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_parameter_return(
                    *source_value,
                    scalar_type.bits() > 32,
                    *location,
                )?,
                Architecture::X86_64 => {
                    emit_x86_64_parameter_return(*source_value, scalar_type.bits() > 32, *location)?
                }
            }
        }
        TerminalTargetOperation::ReturnBooleanParameter {
            source_value,
            location,
            ..
        } => match architecture {
            Architecture::Aarch64 => {
                emit_aarch64_parameter_return(*source_value, false, *location)?
            }
            Architecture::X86_64 => emit_x86_64_parameter_return(*source_value, false, *location)?,
        },
        TerminalTargetOperation::ReturnIntegerExpression {
            source_value,
            scalar_type,
            expression,
            ..
        } => {
            require_native_integer_width(*source_value, *scalar_type)?;
            match architecture {
                Architecture::Aarch64 => emit_aarch64_integer_expression(*scalar_type, expression)?,
                Architecture::X86_64 => emit_x86_64_integer_expression(*scalar_type, expression)?,
            }
        }
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

fn emit_x86_64_integer_expression(
    scalar_type: IntegerType,
    expression: &TerminalTargetIntegerExpression,
) -> Result<Vec<u8>, EmissionError> {
    expression_parameter_locations(expression)?;
    let mut bytes = Vec::new();
    emit_x86_64_expression_node(&mut bytes, scalar_type, expression, 0)?;
    bytes.push(0xc3); // ret
    Ok(bytes)
}

fn emit_x86_64_expression_node(
    bytes: &mut Vec<u8>,
    scalar_type: IntegerType,
    expression: &TerminalTargetIntegerExpression,
    stack_depth: u32,
) -> Result<(), EmissionError> {
    match expression {
        TerminalTargetIntegerExpression::Immediate {
            source_value,
            value,
        } => {
            let bits = integer_bits(*source_value, scalar_type, *value)?;
            bytes.extend_from_slice(&[0x48, 0xb8]); // mov rax, imm64
            bytes.extend_from_slice(&bits.to_le_bytes());
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalTargetIntegerExpression::Parameter {
            source_value,
            location,
            ..
        } => {
            match location {
                TerminalScalarParameterLocation::Register(register) => {
                    let register_code = x86_gpr_code(*source_value, *register)?;
                    if matches!(register_code, 0 | 4 | 10 | 11) {
                        return Err(EmissionError::ExpressionScratchRegisterConflict {
                            value: *source_value,
                            register: *register,
                        });
                    }
                    let rex = 0x48 | (((register_code >> 3) & 1) << 2);
                    bytes.extend_from_slice(&[rex, 0x89, 0xc0 | ((register_code & 7) << 3)]); // mov rax, selected argument register
                }
                TerminalScalarParameterLocation::IncomingStack { byte_offset } => {
                    let displacement = byte_offset
                        .checked_add(8)
                        .and_then(|offset| offset.checked_add(stack_depth))
                        .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                            value: *source_value,
                            byte_offset: *byte_offset,
                        })?;
                    bytes.extend_from_slice(&[0x48, 0x8b]); // mov rax, [rsp + displacement]
                    if displacement <= i8::MAX as u32 {
                        bytes.extend_from_slice(&[0x44, 0x24, displacement as u8]);
                    } else {
                        bytes.extend_from_slice(&[0x84, 0x24]);
                        bytes.extend_from_slice(&displacement.to_le_bytes());
                    }
                }
            }
            emit_x86_64_normalize(bytes, scalar_type);
        }
        TerminalTargetIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingSubtract { left, right, .. } => {
            emit_x86_64_expression_node(bytes, scalar_type, left, stack_depth)?;
            bytes.push(0x50); // push rax
            let nested_depth = stack_depth.checked_add(8).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_x86_64_expression_node(bytes, scalar_type, right, nested_depth)?;
            bytes.extend_from_slice(&[0x41, 0x5a]); // pop r10
            match expression {
                TerminalTargetIntegerExpression::WrappingAdd { .. } => {
                    bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalTargetIntegerExpression::SaturatingAdd { .. } => {
                    emit_x86_64_saturating_add(bytes, scalar_type);
                }
                TerminalTargetIntegerExpression::WrappingSubtract { .. } => {
                    bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
                    bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
                    emit_x86_64_normalize(bytes, scalar_type);
                }
                TerminalTargetIntegerExpression::SaturatingSubtract { .. } => {
                    emit_x86_64_saturating_subtract(bytes, scalar_type);
                }
                _ => unreachable!("outer match admits only binary arithmetic nodes"),
            }
        }
    }
    Ok(())
}

fn emit_x86_64_normalize(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    match (scalar_type.sign(), scalar_type.bits()) {
        (_, 64) => {}
        (IntegerSign::Unsigned, 8) => bytes.extend_from_slice(&[0x25, 0xff, 0, 0, 0]),
        (IntegerSign::Unsigned, 16) => bytes.extend_from_slice(&[0x25, 0xff, 0xff, 0, 0]),
        (IntegerSign::Unsigned, 32) => bytes.extend_from_slice(&[0x89, 0xc0]),
        (IntegerSign::Signed, 8) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbe, 0xc0]),
        (IntegerSign::Signed, 16) => bytes.extend_from_slice(&[0x48, 0x0f, 0xbf, 0xc0]),
        (IntegerSign::Signed, 32) => bytes.extend_from_slice(&[0x48, 0x63, 0xc0]),
        _ => unreachable!("native integer width was checked before expression emission"),
    }
}

fn emit_x86_64_saturating_add(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            bytes.extend_from_slice(&[0x4d, 0x19, 0xd2]); // sbb r10, r10
            bytes.extend_from_slice(&[0x4c, 0x09, 0xd0]); // or rax, r10
        }
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x47, 0xc2]); // cmova rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.extend_from_slice(&[0x4d, 0x89, 0xd3]); // mov r11, r10
            bytes.extend_from_slice(&[0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
            bytes.extend_from_slice(&[0x49, 0xf7, 0xd3]); // not r11
            bytes.extend_from_slice(&[0x49, 0x0f, 0xba, 0xfb, 0x3f]); // btc r11, 63
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x4c, 0x01, 0xd0]); // add rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
}

fn emit_x86_64_saturating_subtract(bytes: &mut Vec<u8>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, _) => {
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0xb8, 0, 0, 0, 0]); // mov eax, 0 (flags unchanged)
            bytes.extend_from_slice(&[0x49, 0x0f, 0x43, 0xc2]); // cmovae rax, r10
        }
        (IntegerSign::Signed, 64) => {
            bytes.extend_from_slice(&[0x4d, 0x89, 0xd3]); // mov r11, r10
            bytes.extend_from_slice(&[0x49, 0xc1, 0xfb, 0x3f]); // sar r11, 63
            bytes.extend_from_slice(&[0x49, 0xf7, 0xd3]); // not r11
            bytes.extend_from_slice(&[0x49, 0x0f, 0xba, 0xfb, 0x3f]); // btc r11, 63
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x40, 0xc3]); // cmovo rax, r11
        }
        (IntegerSign::Signed, _) => {
            bytes.extend_from_slice(&[0x49, 0x29, 0xc2]); // sub r10, rax
            bytes.extend_from_slice(&[0x4c, 0x89, 0xd0]); // mov rax, r10
            emit_x86_64_mov_r10(bytes, maximum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4f, 0xc2]); // cmovg rax, r10
            emit_x86_64_mov_r10(bytes, minimum);
            bytes.extend_from_slice(&[0x4c, 0x39, 0xd0]); // cmp rax, r10
            bytes.extend_from_slice(&[0x49, 0x0f, 0x4c, 0xc2]); // cmovl rax, r10
        }
    }
}

fn emit_x86_64_mov_r10(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&[0x49, 0xba]);
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[derive(Debug)]
struct Aarch64ExpressionFrame {
    byte_size: u32,
    register_spills: BTreeMap<usize, Aarch64RegisterSpill>,
}

#[derive(Debug)]
struct Aarch64RegisterSpill {
    source_value: ValueId,
    register: u8,
    byte_offset: u32,
}

fn emit_aarch64_integer_expression(
    scalar_type: IntegerType,
    expression: &TerminalTargetIntegerExpression,
) -> Result<Vec<u8>, EmissionError> {
    let frame = plan_aarch64_expression_frame(expression)?;
    let mut instructions = Vec::new();
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, false)?;
        for spill in frame.register_spills.values() {
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                spill.register,
                spill.source_value,
                spill.byte_offset,
            )?); // str xN, [sp, #spill]
        }
    }
    emit_aarch64_expression_node(&mut instructions, scalar_type, expression, &frame, 0)?;
    if frame.byte_size != 0 {
        emit_aarch64_adjust_sp(&mut instructions, frame.byte_size, true)?;
    }
    instructions.push(0xd65f_03c0); // ret x30
    Ok(instructions
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect())
}

fn plan_aarch64_expression_frame(
    expression: &TerminalTargetIntegerExpression,
) -> Result<Aarch64ExpressionFrame, EmissionError> {
    let locations = expression_parameter_locations(expression)?;
    let mut register_spills = BTreeMap::new();
    for (parameter_index, (source_value, location)) in locations {
        let TerminalScalarParameterLocation::Register(register) = location else {
            continue;
        };
        let MachineRegister::Aarch64X(register) = register else {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source_value,
                register,
                architecture: Architecture::Aarch64,
            });
        };
        if register >= 31 {
            return Err(EmissionError::ParameterRegisterArchitectureMismatch {
                value: source_value,
                register: MachineRegister::Aarch64X(register),
                architecture: Architecture::Aarch64,
            });
        }
        let byte_offset = u32::try_from(register_spills.len())
            .ok()
            .and_then(|count| count.checked_mul(8))
            .ok_or(EmissionError::ExpressionStackFrameNotEncodable)?;
        register_spills.insert(
            parameter_index,
            Aarch64RegisterSpill {
                source_value,
                register,
                byte_offset,
            },
        );
    }
    let used_bytes = u32::try_from(register_spills.len())
        .ok()
        .and_then(|count| count.checked_mul(8))
        .ok_or(EmissionError::ExpressionStackFrameNotEncodable)?;
    let byte_size = used_bytes
        .checked_add(15)
        .map(|bytes| bytes & !15)
        .ok_or(EmissionError::ExpressionStackFrameNotEncodable)?;
    if byte_size > 0xfff {
        return Err(EmissionError::ExpressionStackFrameNotEncodable);
    }
    Ok(Aarch64ExpressionFrame {
        byte_size,
        register_spills,
    })
}

fn emit_aarch64_expression_node(
    instructions: &mut Vec<u32>,
    scalar_type: IntegerType,
    expression: &TerminalTargetIntegerExpression,
    frame: &Aarch64ExpressionFrame,
    stack_depth: u32,
) -> Result<(), EmissionError> {
    match expression {
        TerminalTargetIntegerExpression::Immediate {
            source_value,
            value,
        } => {
            let bits = integer_bits(*source_value, scalar_type, *value)?;
            emit_aarch64_mov_immediate(instructions, 0, bits);
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalTargetIntegerExpression::Parameter {
            source_value,
            parameter_index,
            location,
        } => {
            let byte_offset = match location {
                TerminalScalarParameterLocation::Register(register) => {
                    let spill = frame.register_spills.get(parameter_index).ok_or(
                        EmissionError::ExpressionParameterSpillMissing {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        },
                    )?;
                    if *register != MachineRegister::Aarch64X(spill.register) {
                        return Err(EmissionError::ExpressionParameterLocationConflict {
                            value: *source_value,
                            parameter_index: *parameter_index,
                        });
                    }
                    stack_depth.checked_add(spill.byte_offset)
                }
                TerminalScalarParameterLocation::IncomingStack { byte_offset } => stack_depth
                    .checked_add(frame.byte_size)
                    .and_then(|offset| offset.checked_add(*byte_offset)),
            }
            .ok_or(EmissionError::IncomingStackOffsetNotEncodable {
                value: *source_value,
                byte_offset: match location {
                    TerminalScalarParameterLocation::Register(_) => 0,
                    TerminalScalarParameterLocation::IncomingStack { byte_offset } => *byte_offset,
                },
            })?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                0,
                *source_value,
                byte_offset,
            )?); // ldr x0, [sp, #value]
            emit_aarch64_normalize(instructions, scalar_type);
        }
        TerminalTargetIntegerExpression::WrappingAdd { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingAdd { left, right, .. }
        | TerminalTargetIntegerExpression::WrappingSubtract { left, right, .. }
        | TerminalTargetIntegerExpression::SaturatingSubtract { left, right, .. } => {
            emit_aarch64_expression_node(instructions, scalar_type, left, frame, stack_depth)?;
            emit_aarch64_adjust_sp(instructions, 16, false)?;
            instructions.push(aarch64_stack_access(
                0xf900_0000,
                0,
                expression_source(left),
                0,
            )?); // str x0, [sp]
            let nested_depth = stack_depth.checked_add(16).ok_or(
                EmissionError::ExpressionStackDepthNotEncodable {
                    value: expression_source(left),
                },
            )?;
            emit_aarch64_expression_node(instructions, scalar_type, right, frame, nested_depth)?;
            instructions.push(aarch64_stack_access(
                0xf940_0000,
                9,
                expression_source(left),
                0,
            )?); // ldr x9, [sp]
            emit_aarch64_adjust_sp(instructions, 16, true)?;
            match expression {
                TerminalTargetIntegerExpression::WrappingAdd { .. } => {
                    instructions.push(0x8b00_0120); // add x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalTargetIntegerExpression::SaturatingAdd { .. } => {
                    emit_aarch64_saturating_add(instructions, scalar_type);
                }
                TerminalTargetIntegerExpression::WrappingSubtract { .. } => {
                    instructions.push(0xcb00_0120); // sub x0, x9, x0
                    emit_aarch64_normalize(instructions, scalar_type);
                }
                TerminalTargetIntegerExpression::SaturatingSubtract { .. } => {
                    emit_aarch64_saturating_subtract(instructions, scalar_type);
                }
                _ => unreachable!("outer match admits only binary arithmetic nodes"),
            }
        }
    }
    Ok(())
}

fn emit_aarch64_normalize(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    if scalar_type.bits() == 64 {
        return;
    }
    let base = match scalar_type.sign() {
        IntegerSign::Signed => 0x9340_0000,   // sbfm
        IntegerSign::Unsigned => 0xd340_0000, // ubfm
    };
    instructions.push(base | (u32::from(scalar_type.bits() - 1) << 10));
}

fn emit_aarch64_saturating_add(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, 64) => {
            instructions.push(0xab00_0120); // adds x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(aarch64_csel(0, 0, 10, 3)); // csel x0, x0, x10, cc
        }
        (IntegerSign::Unsigned, _) => {
            instructions.push(0x8b00_0120); // add x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 9)); // csel x0, x0, x10, ls
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0x937f_fd2a); // asr x10, x9, 63
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(0xca0b_014a); // eor x10, x10, x11
            instructions.push(0xab00_0120); // adds x0, x9, x0
            instructions.push(aarch64_csel(0, 0, 10, 7)); // csel x0, x0, x10, vc
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0x8b00_0120); // add x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
}

fn emit_aarch64_saturating_subtract(instructions: &mut Vec<u32>, scalar_type: IntegerType) {
    let (minimum, maximum) = native_integer_bounds(scalar_type);
    match (scalar_type.sign(), scalar_type.bits()) {
        (IntegerSign::Unsigned, _) => {
            instructions.push(0xeb00_0129); // subs x9, x9, x0
            instructions.push(aarch64_csel(0, 9, 31, 2)); // csel x0, x9, xzr, cs
        }
        (IntegerSign::Signed, 64) => {
            instructions.push(0x937f_fd2a); // asr x10, x9, 63
            emit_aarch64_mov_immediate(instructions, 11, maximum);
            instructions.push(0xca0b_014a); // eor x10, x10, x11
            instructions.push(0xeb00_0120); // subs x0, x9, x0
            instructions.push(aarch64_csel(0, 0, 10, 7)); // csel x0, x0, x10, vc
        }
        (IntegerSign::Signed, _) => {
            instructions.push(0xcb00_0120); // sub x0, x9, x0
            emit_aarch64_mov_immediate(instructions, 10, maximum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 13)); // csel x0, x0, x10, le
            emit_aarch64_mov_immediate(instructions, 10, minimum);
            instructions.push(0xeb0a_001f); // cmp x0, x10
            instructions.push(aarch64_csel(0, 0, 10, 10)); // csel x0, x0, x10, ge
        }
    }
}

fn emit_aarch64_mov_immediate(instructions: &mut Vec<u32>, register: u8, bits: u64) {
    for chunk in 0..4 {
        let immediate = ((bits >> (chunk * 16)) & 0xffff) as u32;
        if chunk == 0 || immediate != 0 {
            let base = if chunk == 0 { 0xd280_0000 } else { 0xf280_0000 };
            instructions
                .push(base | ((chunk as u32) << 21) | (immediate << 5) | u32::from(register));
        }
    }
}

fn emit_aarch64_adjust_sp(
    instructions: &mut Vec<u32>,
    byte_size: u32,
    add: bool,
) -> Result<(), EmissionError> {
    if byte_size > 0xfff {
        return Err(EmissionError::ExpressionStackFrameNotEncodable);
    }
    let base = if add { 0x9100_03ff } else { 0xd100_03ff };
    instructions.push(base | (byte_size << 10));
    Ok(())
}

fn aarch64_stack_access(
    base: u32,
    register: u8,
    source_value: ValueId,
    byte_offset: u32,
) -> Result<u32, EmissionError> {
    if byte_offset % 8 != 0 || byte_offset / 8 > 0xfff {
        return Err(EmissionError::IncomingStackOffsetNotEncodable {
            value: source_value,
            byte_offset,
        });
    }
    Ok(base | ((byte_offset / 8) << 10) | (31 << 5) | u32::from(register))
}

const fn aarch64_csel(destination: u8, left: u8, right: u8, condition: u8) -> u32 {
    0x9a80_0000
        | ((right as u32) << 16)
        | ((condition as u32) << 12)
        | ((left as u32) << 5)
        | destination as u32
}

fn expression_parameter_locations(
    expression: &TerminalTargetIntegerExpression,
) -> Result<BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>, EmissionError> {
    fn collect(
        expression: &TerminalTargetIntegerExpression,
        locations: &mut BTreeMap<usize, (ValueId, TerminalScalarParameterLocation)>,
    ) -> Result<(), EmissionError> {
        match expression {
            TerminalTargetIntegerExpression::Immediate { .. } => {}
            TerminalTargetIntegerExpression::Parameter {
                source_value,
                parameter_index,
                location,
            } => {
                if let Some((_, established)) = locations.get(parameter_index) {
                    if established != location {
                        return Err(EmissionError::ExpressionParameterLocationConflict {
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
            | TerminalTargetIntegerExpression::SaturatingSubtract { left, right, .. } => {
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

fn expression_source(expression: &TerminalTargetIntegerExpression) -> ValueId {
    match expression {
        TerminalTargetIntegerExpression::Immediate { source_value, .. }
        | TerminalTargetIntegerExpression::Parameter { source_value, .. } => *source_value,
        TerminalTargetIntegerExpression::WrappingAdd { left, .. }
        | TerminalTargetIntegerExpression::SaturatingAdd { left, .. }
        | TerminalTargetIntegerExpression::WrappingSubtract { left, .. }
        | TerminalTargetIntegerExpression::SaturatingSubtract { left, .. } => {
            expression_source(left)
        }
    }
}

fn native_integer_bounds(scalar_type: IntegerType) -> (u64, u64) {
    let width = scalar_type.bits();
    match scalar_type.sign() {
        IntegerSign::Unsigned => {
            let maximum = if width == 64 {
                u64::MAX
            } else {
                (1_u64 << width) - 1
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let maximum = if width == 64 {
                i64::MAX as u64
            } else {
                (1_u64 << (width - 1)) - 1
            };
            let minimum = if width == 64 {
                i64::MIN as u64
            } else {
                u64::MAX << (width - 1)
            };
            (minimum, maximum)
        }
    }
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
    ExpressionScratchRegisterConflict {
        value: ValueId,
        register: MachineRegister,
    },
    ExpressionParameterLocationConflict {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionParameterSpillMissing {
        value: ValueId,
        parameter_index: usize,
    },
    ExpressionStackDepthNotEncodable {
        value: ValueId,
    },
    ExpressionStackFrameNotEncodable,
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
        TerminalPsiProvenance, TerminalTargetFunction, TerminalTargetIntegerExpression,
        TerminalTargetOperation, TerminalTargetOperationPlan,
    };
    use psi_core::{EdgeId, MachineId, OperationId};
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
    fn emits_parameter_fed_wrapping_add_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
                0x41, 0x5a, 0x4c, 0x01, 0xd0, 0x25, 0xff, 0, 0, 0, 0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        assert_eq!(
            aarch64_instructions(&aarch64.functions[0].bytes),
            [
                0xd100_43ff,
                0xf900_03e0,
                0xf900_07e1,
                0xf940_03e0,
                0xd340_1c00,
                0xd100_43ff,
                0xf900_03e0,
                0xf940_0fe0,
                0xd340_1c00,
                0xf940_03e9,
                0x9100_43ff,
                0x8b00_0120,
                0xd340_1c00,
                0x9100_43ff,
                0xd65f_03c0,
            ]
        );
    }

    #[test]
    fn emits_parameter_fed_wrapping_subtract_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let expression = |left, right| TerminalTargetIntegerExpression::WrappingSubtract {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert_eq!(
            x86.functions[0].bytes,
            [
                0x48, 0x89, 0xf8, 0x25, 0xff, 0, 0, 0, 0x50, 0x48, 0x89, 0xf0, 0x25, 0xff, 0, 0, 0,
                0x41, 0x5a, 0x49, 0x29, 0xc2, 0x4c, 0x89, 0xd0, 0x25, 0xff, 0, 0, 0, 0xc3,
            ]
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0xcb00_0120)); // sub x0, x9, x0
        assert_eq!(instructions.last(), Some(&0xd65f_03c0));
    }

    #[test]
    fn emits_parameter_fed_saturating_subtract_for_both_architectures() {
        let expression = |left, right| TerminalTargetIntegerExpression::SaturatingSubtract {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            u8_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0].bytes.windows(12).any(
                |window| window == [0x49, 0x29, 0xc2, 0xb8, 0, 0, 0, 0, 0x49, 0x0f, 0x43, 0xc2]
            )
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            u8_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0xeb00_0129)); // subs x9, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 9, 31, 2))); // cs

        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            i64_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0]
                .bytes
                .windows(4)
                .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
        ); // cmovo

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            i64_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0xeb00_0120)); // subs x0, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 0, 10, 7))); // vc
    }

    #[test]
    fn runtime_expression_stack_loads_retain_the_incoming_stack_base() {
        let scalar_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ))
        .unwrap();
        assert!(
            x86.functions[0]
                .bytes
                .windows(5)
                .any(|window| window == [0x48, 0x8b, 0x44, 0x24, 16])
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            wrapping_expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::IncomingStack { byte_offset: 0 },
            ),
        ))
        .unwrap();
        assert!(aarch64_instructions(&aarch64.functions[0].bytes).contains(&0xf940_13e0));
    }

    #[test]
    fn emits_signed_i64_saturation_for_both_architectures() {
        let scalar_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");
        let expression = |left, right| TerminalTargetIntegerExpression::SaturatingAdd {
            psi_operation: OperationId::new(3).expect("operation"),
            left: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(1).expect("left"),
                parameter_index: 0,
                location: left,
            }),
            right: Box::new(TerminalTargetIntegerExpression::Parameter {
                source_value: ValueId::new(2).expect("right"),
                parameter_index: 1,
                location: right,
            }),
        };
        let x86 = emit_machine_code(&expression_plan(
            NativeTarget::linux_x64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rdi),
                TerminalScalarParameterLocation::Register(MachineRegister::X86Rsi),
            ),
        ))
        .unwrap();
        let x86_bytes = &x86.functions[0].bytes;
        assert!(
            x86_bytes
                .windows(5)
                .any(|window| window == [0x49, 0x0f, 0xba, 0xfb, 0x3f])
        );
        assert!(
            x86_bytes
                .windows(4)
                .any(|window| window == [0x49, 0x0f, 0x40, 0xc3])
        );

        let aarch64 = emit_machine_code(&expression_plan(
            NativeTarget::linux_arm64(),
            scalar_type,
            expression(
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(0)),
                TerminalScalarParameterLocation::Register(MachineRegister::Aarch64X(1)),
            ),
        ))
        .unwrap();
        let instructions = aarch64_instructions(&aarch64.functions[0].bytes);
        assert!(instructions.contains(&0x937f_fd2a)); // asr x10, x9, 63
        assert!(instructions.contains(&0xca0b_014a)); // eor x10, x10, x11
        assert!(instructions.contains(&0xab00_0120)); // adds x0, x9, x0
        assert!(instructions.contains(&aarch64_csel(0, 0, 10, 7))); // vc
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

    fn expression_plan(
        target: NativeTarget,
        scalar_type: IntegerType,
        expression: TerminalTargetIntegerExpression,
    ) -> TerminalTargetOperationPlan {
        TerminalTargetOperationPlan {
            terminal_psi: identity(),
            target,
            entry: MachineId::new(1).expect("machine"),
            functions: vec![TerminalTargetFunction {
                machine: MachineId::new(1).expect("machine"),
                provenance: TerminalPsiProvenance::default(),
                operation: TerminalTargetOperation::ReturnIntegerExpression {
                    psi_edge: EdgeId::new(1).expect("edge"),
                    source_value: ValueId::new(3).expect("result"),
                    scalar_type,
                    expression,
                },
            }],
        }
    }

    fn wrapping_expression(
        left_location: TerminalScalarParameterLocation,
        right_location: TerminalScalarParameterLocation,
    ) -> TerminalTargetIntegerExpression {
        TerminalTargetIntegerExpression::WrappingAdd {
            psi_operation: OperationId::new(3).expect("operation"),
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
        }
    }

    fn aarch64_instructions(bytes: &[u8]) -> Vec<u32> {
        bytes
            .chunks_exact(4)
            .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("instruction")))
            .collect()
    }

    fn identity() -> TerminalPsiIdentity {
        TerminalPsiIdentity {
            semantic_version: SemanticVersion::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        }
    }
}
