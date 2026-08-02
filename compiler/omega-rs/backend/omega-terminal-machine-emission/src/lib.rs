#![forbid(unsafe_code)]

//! Machine-code emission for the first source-independent terminal-Psi target
//! operation slice.

use omega_target::Architecture;
use omega_terminal_machine_code::{TerminalMachineCodeFunction, TerminalMachineCodePlan};
use omega_terminal_target_operations::{
    TerminalTargetFunction, TerminalTargetOperation, TerminalTargetOperationPlan,
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
    };
    Ok(TerminalMachineCodeFunction {
        machine: function.machine,
        provenance: function.provenance.clone(),
        bytes,
    })
}

fn integer_bits(
    source: ValueId,
    scalar_type: IntegerType,
    value: IntegerValue,
) -> Result<u64, EmissionError> {
    let width = scalar_type.bits();
    if !matches!(width, 8 | 16 | 32 | 64) {
        return Err(EmissionError::IntegerWidthNotNativelySupported {
            value: source,
            bits: width,
        });
    }
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
    IntegerWidthNotNativelySupported { value: ValueId, bits: u16 },
    IntegerOutsideType(ValueId),
    IntegerSignMismatch(ValueId),
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

    fn plan(target: NativeTarget) -> TerminalTargetOperationPlan {
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        TerminalTargetOperationPlan {
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
    fn rejects_integer_width_without_a_native_scalar_realization() {
        let mut plan = plan(NativeTarget::linux_x64());
        let TerminalTargetOperation::ReturnIntegerImmediate {
            scalar_type, value, ..
        } = &mut plan.functions[0].operation;
        *scalar_type = IntegerType::new(IntegerSign::Signed, 128).expect("i128");
        *value = IntegerValue::Signed(7);
        assert!(matches!(
            emit_machine_code(&plan),
            Err(EmissionError::IntegerWidthNotNativelySupported { bits: 128, .. })
        ));
    }
}
