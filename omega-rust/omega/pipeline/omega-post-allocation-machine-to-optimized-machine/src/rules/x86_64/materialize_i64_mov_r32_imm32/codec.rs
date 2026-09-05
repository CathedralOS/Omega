use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    RegisterWriteSemantics,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::MachineId;

use crate::{
    X86MovR32Imm32InstructionDisposition, X86MovR32Imm32MaterializationAction,
    X86MovR32Imm32MaterializationAttempt, X86MovR32Imm32MaterializationAttemptOutcome,
    X86MovR32Imm32MaterializationBlock, X86MovR32Imm32MaterializationFunction,
    X86MovR32Imm32MaterializationIdentity, X86MovR32Imm32MaterializationInstruction,
    X86MovR32Imm32MaterializationPlan, X86MovR32Imm32MaterializationPolicy,
    X86MovR32Imm32MaterializationRevisionIdentity, X86MovR32Imm32PhysicalWrite,
    x86_mov_r32_imm32_materialization_identity,
};
use omega_physical_instructions::PostAllocationMachineIdentity;

const MAGIC: &[u8; 8] = b"OMGXM32\0";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86MovR32Imm32MaterializationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for X86MovR32Imm32MaterializationDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 MOV-r32-imm32 materialization artifact: {self:?}"
        )
    }
}

impl std::error::Error for X86MovR32Imm32MaterializationDecodeError {}

pub(crate) fn encode(plan: &X86MovR32Imm32MaterializationPlan) -> Vec<u8> {
    let content = super::identity::encode_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode(
    encoded: &[u8],
) -> Result<X86MovR32Imm32MaterializationPlan, X86MovR32Imm32MaterializationDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(X86MovR32Imm32MaterializationDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(X86MovR32Imm32MaterializationDecodeError::UnsupportedVersion(version));
    }
    let identity = X86MovR32Imm32MaterializationIdentity::from_bytes(cursor.array()?);
    let source = PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let target = decode_target(&mut cursor)?;
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => X86MovR32Imm32MaterializationPolicy::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
        _ => return Err(X86MovR32Imm32MaterializationDecodeError::InvalidField),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| X86MovR32Imm32MaterializationDecodeError::InvalidField)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| X86MovR32Imm32MaterializationDecodeError::InvalidField)?;
    let output_revision =
        X86MovR32Imm32MaterializationRevisionIdentity::from_bytes(cursor.array()?);
    let attempt_count = cursor.length()?;
    let mut attempts = Vec::with_capacity(attempt_count.min(cursor.remaining()));
    for _ in 0..attempt_count {
        attempts.push(X86MovR32Imm32MaterializationAttempt {
            iteration: cursor.u64()?,
            input: X86MovR32Imm32MaterializationRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            literal_bits: cursor.u64()?,
            destination: decode_write(&mut cursor)?,
            baseline_byte_count: cursor.byte()?,
            selected_byte_count: cursor.byte()?,
            outcome: match cursor.byte()? {
                0 => X86MovR32Imm32MaterializationAttemptOutcome::AlreadySelected,
                1 => X86MovR32Imm32MaterializationAttemptOutcome::IntegerOutsideZeroExtendedU32,
                2 => X86MovR32Imm32MaterializationAttemptOutcome::SelectedForRewrite,
                _ => return Err(X86MovR32Imm32MaterializationDecodeError::InvalidField),
            },
        });
    }
    let action_count = cursor.length()?;
    let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
    for _ in 0..action_count {
        actions.push(X86MovR32Imm32MaterializationAction {
            iteration: cursor.u64()?,
            input: X86MovR32Imm32MaterializationRevisionIdentity::from_bytes(cursor.array()?),
            output: X86MovR32Imm32MaterializationRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            literal_bits: cursor.u64()?,
            destination: decode_write(&mut cursor)?,
            baseline_byte_count: cursor.byte()?,
            selected_byte_count: cursor.byte()?,
        });
    }
    let function_count = cursor.length()?;
    let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
    for _ in 0..function_count {
        let machine = machine(&mut cursor)?;
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(cursor.u32()?);
            let instruction_count = cursor.length()?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                let instruction = SelectedInstructionId(cursor.u32()?);
                let disposition = match cursor.byte()? {
                    0 => X86MovR32Imm32InstructionDisposition::RetainedV1,
                    1 => X86MovR32Imm32InstructionDisposition::MovR32Imm32MaterializationV1 {
                        literal_bits: cursor.u64()?,
                        destination: decode_write(&mut cursor)?,
                        baseline_byte_count: cursor.byte()?,
                        selected_byte_count: cursor.byte()?,
                    },
                    _ => return Err(X86MovR32Imm32MaterializationDecodeError::InvalidField),
                };
                instructions.push(X86MovR32Imm32MaterializationInstruction {
                    instruction,
                    disposition,
                });
            }
            blocks.push(X86MovR32Imm32MaterializationBlock {
                block,
                instructions,
            });
        }
        functions.push(X86MovR32Imm32MaterializationFunction { machine, blocks });
    }
    if cursor.remaining() != 0 {
        return Err(X86MovR32Imm32MaterializationDecodeError::TrailingBytes);
    }
    let plan = X86MovR32Imm32MaterializationPlan {
        identity,
        source,
        selected,
        target,
        physical_register_model,
        policy,
        budget,
        usage,
        output_revision,
        attempts,
        actions,
        functions,
    };
    if plan.identity != x86_mov_r32_imm32_materialization_identity(&plan) {
        return Err(X86MovR32Imm32MaterializationDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_write(
    cursor: &mut Cursor<'_>,
) -> Result<X86MovR32Imm32PhysicalWrite, X86MovR32Imm32MaterializationDecodeError> {
    Ok(X86MovR32Imm32PhysicalWrite {
        instruction: SelectedInstructionId(cursor.u32()?),
        operand: cursor.u16()?,
        virtual_register: VirtualRegisterId(cursor.u32()?),
        class: RegisterClassId(cursor.u16()?),
        destination_view: RegisterViewId(cursor.u16()?),
        destination_storage_units: decode_units(cursor)?,
        destination_write_units: decode_units(cursor)?,
        destination_write_semantics: decode_write_semantics(cursor)?,
        encoded_view: RegisterViewId(cursor.u16()?),
        encoded_storage_units: decode_units(cursor)?,
        encoded_write_units: decode_units(cursor)?,
        encoded_write_semantics: decode_write_semantics(cursor)?,
    })
}

fn decode_write_semantics(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterWriteSemantics, X86MovR32Imm32MaterializationDecodeError> {
    Ok(match cursor.byte()? {
        0 => RegisterWriteSemantics::ExactView,
        1 => RegisterWriteSemantics::PreservesUnwritten,
        2 => RegisterWriteSemantics::ZeroExtendsParent,
        3 => RegisterWriteSemantics::ZeroExtendsWithinUnit,
        4 => RegisterWriteSemantics::Discards,
        5 => RegisterWriteSemantics::InstructionDefined,
        _ => return Err(X86MovR32Imm32MaterializationDecodeError::InvalidField),
    })
}

fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, X86MovR32Imm32MaterializationDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn machine(cursor: &mut Cursor<'_>) -> Result<MachineId, X86MovR32Imm32MaterializationDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(X86MovR32Imm32MaterializationDecodeError::InvalidField)
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, X86MovR32Imm32MaterializationDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(X86MovR32Imm32MaterializationDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(X86MovR32Imm32MaterializationDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| X86MovR32Imm32MaterializationDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| X86MovR32Imm32MaterializationDecodeError::InvalidField)?;
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size,
        pointer_alignment,
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], X86MovR32Imm32MaterializationDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(X86MovR32Imm32MaterializationDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(X86MovR32Imm32MaterializationDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], X86MovR32Imm32MaterializationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| X86MovR32Imm32MaterializationDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, X86MovR32Imm32MaterializationDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, X86MovR32Imm32MaterializationDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, X86MovR32Imm32MaterializationDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, X86MovR32Imm32MaterializationDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn length(&mut self) -> Result<usize, X86MovR32Imm32MaterializationDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| X86MovR32Imm32MaterializationDecodeError::InvalidField)
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use omega_physical_instructions::PostAllocationMachineIdentity;
    use omega_register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
        RegisterWriteSemantics,
    };
    use omega_selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    };
    use omega_target::NativeTarget;
    use psi_core::MachineId;

    use super::X86MovR32Imm32MaterializationDecodeError;
    use crate::*;

    fn plan() -> X86MovR32Imm32MaterializationPlan {
        let instruction = SelectedInstructionId(3);
        let destination = X86MovR32Imm32PhysicalWrite {
            instruction,
            operand: 0,
            virtual_register: VirtualRegisterId(4),
            class: RegisterClassId(0),
            destination_view: RegisterViewId(0),
            destination_storage_units: vec![
                RegisterUnitId(0),
                RegisterUnitId(1),
                RegisterUnitId(2),
                RegisterUnitId(3),
            ],
            destination_write_units: vec![
                RegisterUnitId(0),
                RegisterUnitId(1),
                RegisterUnitId(2),
                RegisterUnitId(3),
            ],
            destination_write_semantics: RegisterWriteSemantics::ExactView,
            encoded_view: RegisterViewId(1),
            encoded_storage_units: vec![RegisterUnitId(0), RegisterUnitId(1), RegisterUnitId(2)],
            encoded_write_units: vec![
                RegisterUnitId(0),
                RegisterUnitId(1),
                RegisterUnitId(2),
                RegisterUnitId(3),
            ],
            encoded_write_semantics: RegisterWriteSemantics::ZeroExtendsParent,
        };
        let input = X86MovR32Imm32MaterializationRevisionIdentity::from_bytes([5; 32]);
        let output = X86MovR32Imm32MaterializationRevisionIdentity::from_bytes([6; 32]);
        let machine = MachineId::new(1).unwrap();
        let mut plan = X86MovR32Imm32MaterializationPlan {
            identity: X86MovR32Imm32MaterializationIdentity::from_bytes([0; 32]),
            source: PostAllocationMachineIdentity::from_bytes([1; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            target: NativeTarget::linux_x64(),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes([4; 32]),
            policy: X86MovR32Imm32MaterializationPolicy::X86SelectMovR32Imm32ZeroExtendedI64MaterializationV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage::default(),
            output_revision: output,
            attempts: vec![X86MovR32Imm32MaterializationAttempt {
                iteration: 1,
                input,
                machine,
                block: SelectedBlockId(2),
                instruction,
                literal_bits: u64::from(u32::MAX),
                destination: destination.clone(),
                baseline_byte_count: 10,
                selected_byte_count: 5,
                outcome: X86MovR32Imm32MaterializationAttemptOutcome::SelectedForRewrite,
            }],
            actions: vec![X86MovR32Imm32MaterializationAction {
                iteration: 1,
                input,
                output,
                machine,
                block: SelectedBlockId(2),
                instruction,
                literal_bits: u64::from(u32::MAX),
                destination: destination.clone(),
                baseline_byte_count: 10,
                selected_byte_count: 5,
            }],
            functions: vec![X86MovR32Imm32MaterializationFunction {
                machine,
                blocks: vec![X86MovR32Imm32MaterializationBlock {
                    block: SelectedBlockId(2),
                    instructions: vec![X86MovR32Imm32MaterializationInstruction {
                        instruction,
                        disposition: X86MovR32Imm32InstructionDisposition::MovR32Imm32MaterializationV1 {
                            literal_bits: u64::from(u32::MAX),
                            destination,
                            baseline_byte_count: 10,
                            selected_byte_count: 5,
                        },
                    }],
                }],
            }],
        };
        plan.identity = x86_mov_r32_imm32_materialization_identity(&plan);
        plan
    }

    #[test]
    fn codec_round_trips_and_authenticates_every_field() {
        let plan = plan();
        let encoded = plan.encode();
        assert_eq!(encoded, plan.encode());
        assert_eq!(
            X86MovR32Imm32MaterializationPlan::decode(&encoded),
            Ok(plan)
        );
    }

    #[test]
    fn codec_rejects_framing_and_content_corruption() {
        let encoded = plan().encode();
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            X86MovR32Imm32MaterializationPlan::decode(&wrong_magic),
            Err(X86MovR32Imm32MaterializationDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            X86MovR32Imm32MaterializationPlan::decode(&wrong_version),
            Err(X86MovR32Imm32MaterializationDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            X86MovR32Imm32MaterializationPlan::decode(&encoded[..encoded.len() - 1]),
            Err(X86MovR32Imm32MaterializationDecodeError::Truncated)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            X86MovR32Imm32MaterializationPlan::decode(&trailing),
            Err(X86MovR32Imm32MaterializationDecodeError::TrailingBytes)
        );
        let mut corrupted = encoded;
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        assert_eq!(
            X86MovR32Imm32MaterializationPlan::decode(&corrupted),
            Err(X86MovR32Imm32MaterializationDecodeError::InvalidIdentity)
        );
    }
}
