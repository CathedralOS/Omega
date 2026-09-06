use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    RegisterWriteSemantics,
};
use selected_instructions::LivenessIdentity;
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::MachineId;
use target::{Architecture, NativeTarget, ObjectFormat};

use crate::{
    X86XorZeroInstructionDisposition, X86XorZeroMaterializationAction,
    X86XorZeroMaterializationAttempt, X86XorZeroMaterializationAttemptOutcome,
    X86XorZeroMaterializationBlock, X86XorZeroMaterializationFunction,
    X86XorZeroMaterializationIdentity, X86XorZeroMaterializationInstruction,
    X86XorZeroMaterializationPlan, X86XorZeroMaterializationPolicy,
    X86XorZeroMaterializationRevisionIdentity, X86XorZeroPhysicalWrite,
    x86_xor_zero_materialization_identity,
};
use physical_instructions::PostAllocationMachineIdentity;

const MAGIC: &[u8; 8] = b"OMGXRZ\0\0";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86XorZeroMaterializationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for X86XorZeroMaterializationDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid x86-64 XOR-zero materialization artifact: {self:?}"
        )
    }
}

impl std::error::Error for X86XorZeroMaterializationDecodeError {}

pub(crate) fn encode(plan: &X86XorZeroMaterializationPlan) -> Vec<u8> {
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
) -> Result<X86XorZeroMaterializationPlan, X86XorZeroMaterializationDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(X86XorZeroMaterializationDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(X86XorZeroMaterializationDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = X86XorZeroMaterializationIdentity::from_bytes(cursor.array()?);
    let source = PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let liveness = LivenessIdentity::from_bytes(cursor.array()?);
    let target = decode_target(&mut cursor)?;
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => X86XorZeroMaterializationPolicy::X86SelectXorZeroI64MaterializationV1,
        _ => return Err(X86XorZeroMaterializationDecodeError::InvalidField),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| X86XorZeroMaterializationDecodeError::InvalidField)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| X86XorZeroMaterializationDecodeError::InvalidField)?;
    let output_revision = X86XorZeroMaterializationRevisionIdentity::from_bytes(cursor.array()?);
    let attempt_count = cursor.length()?;
    let mut attempts = Vec::with_capacity(attempt_count.min(cursor.remaining()));
    for _ in 0..attempt_count {
        attempts.push(X86XorZeroMaterializationAttempt {
            iteration: cursor.u64()?,
            input: X86XorZeroMaterializationRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            literal_bits: cursor.u64()?,
            destination: decode_write(&mut cursor)?,
            rflags_units: decode_units(&mut cursor)?,
            baseline_byte_count: cursor.byte()?,
            selected_byte_count: cursor.byte()?,
            outcome: match cursor.byte()? {
                0 => X86XorZeroMaterializationAttemptOutcome::AlreadySelected,
                1 => X86XorZeroMaterializationAttemptOutcome::NonZeroLiteral,
                2 => X86XorZeroMaterializationAttemptOutcome::RflagsLiveOut,
                3 => X86XorZeroMaterializationAttemptOutcome::SelectedForRewrite,
                _ => return Err(X86XorZeroMaterializationDecodeError::InvalidField),
            },
        });
    }
    let action_count = cursor.length()?;
    let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
    for _ in 0..action_count {
        actions.push(X86XorZeroMaterializationAction {
            iteration: cursor.u64()?,
            input: X86XorZeroMaterializationRevisionIdentity::from_bytes(cursor.array()?),
            output: X86XorZeroMaterializationRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            destination: decode_write(&mut cursor)?,
            rflags_units: decode_units(&mut cursor)?,
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
                    0 => X86XorZeroInstructionDisposition::RetainedV1,
                    1 => X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
                        destination: decode_write(&mut cursor)?,
                        rflags_units: decode_units(&mut cursor)?,
                        baseline_byte_count: cursor.byte()?,
                        selected_byte_count: cursor.byte()?,
                    },
                    _ => return Err(X86XorZeroMaterializationDecodeError::InvalidField),
                };
                instructions.push(X86XorZeroMaterializationInstruction {
                    instruction,
                    disposition,
                });
            }
            blocks.push(X86XorZeroMaterializationBlock {
                block,
                instructions,
            });
        }
        functions.push(X86XorZeroMaterializationFunction { machine, blocks });
    }
    if cursor.remaining() != 0 {
        return Err(X86XorZeroMaterializationDecodeError::TrailingBytes);
    }
    let plan = X86XorZeroMaterializationPlan {
        identity,
        source,
        selected,
        liveness,
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
    if plan.identity != x86_xor_zero_materialization_identity(&plan) {
        return Err(X86XorZeroMaterializationDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_write(
    cursor: &mut Cursor<'_>,
) -> Result<X86XorZeroPhysicalWrite, X86XorZeroMaterializationDecodeError> {
    Ok(X86XorZeroPhysicalWrite {
        instruction: SelectedInstructionId(cursor.u32()?),
        operand: cursor.u16()?,
        virtual_register: VirtualRegisterId(cursor.u32()?),
        class: RegisterClassId(cursor.u16()?),
        view: RegisterViewId(cursor.u16()?),
        storage_units: decode_units(cursor)?,
        write_units: decode_units(cursor)?,
        write_semantics: match cursor.byte()? {
            0 => RegisterWriteSemantics::ExactView,
            1 => RegisterWriteSemantics::PreservesUnwritten,
            2 => RegisterWriteSemantics::ZeroExtendsParent,
            3 => RegisterWriteSemantics::ZeroExtendsWithinUnit,
            4 => RegisterWriteSemantics::Discards,
            5 => RegisterWriteSemantics::InstructionDefined,
            _ => return Err(X86XorZeroMaterializationDecodeError::InvalidField),
        },
    })
}

fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, X86XorZeroMaterializationDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn machine(cursor: &mut Cursor<'_>) -> Result<MachineId, X86XorZeroMaterializationDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(X86XorZeroMaterializationDecodeError::InvalidField)
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, X86XorZeroMaterializationDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(X86XorZeroMaterializationDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(X86XorZeroMaterializationDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| X86XorZeroMaterializationDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| X86XorZeroMaterializationDecodeError::InvalidField)?;
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
    fn take(&mut self, count: usize) -> Result<&'a [u8], X86XorZeroMaterializationDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(X86XorZeroMaterializationDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(X86XorZeroMaterializationDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], X86XorZeroMaterializationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| X86XorZeroMaterializationDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, X86XorZeroMaterializationDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, X86XorZeroMaterializationDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, X86XorZeroMaterializationDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, X86XorZeroMaterializationDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn length(&mut self) -> Result<usize, X86XorZeroMaterializationDecodeError> {
        usize::try_from(self.u64()?).map_err(|_| X86XorZeroMaterializationDecodeError::InvalidField)
    }
}

#[cfg(test)]
mod tests {
    use optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use physical_instructions::PostAllocationMachineIdentity;
    use register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
        RegisterWriteSemantics,
    };
    use selected_instructions::LivenessIdentity;
    use selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    };
    use semantic_vocabulary::MachineId;
    use target::NativeTarget;

    use super::X86XorZeroMaterializationDecodeError;
    use crate::*;

    fn plan() -> X86XorZeroMaterializationPlan {
        let instruction = SelectedInstructionId(3);
        let destination = X86XorZeroPhysicalWrite {
            instruction,
            operand: 0,
            virtual_register: VirtualRegisterId(4),
            class: RegisterClassId(0),
            view: RegisterViewId(0),
            storage_units: vec![RegisterUnitId(0)],
            write_units: vec![RegisterUnitId(0)],
            write_semantics: RegisterWriteSemantics::ExactView,
        };
        let mut plan = X86XorZeroMaterializationPlan {
            identity: X86XorZeroMaterializationIdentity::from_bytes([0; 32]),
            source: PostAllocationMachineIdentity::from_bytes([1; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            liveness: LivenessIdentity::from_bytes([3; 32]),
            target: NativeTarget::linux_x64(),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes([4; 32]),
            policy: X86XorZeroMaterializationPolicy::X86SelectXorZeroI64MaterializationV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage::default(),
            output_revision: X86XorZeroMaterializationRevisionIdentity::from_bytes([5; 32]),
            attempts: vec![],
            actions: vec![],
            functions: vec![X86XorZeroMaterializationFunction {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![X86XorZeroMaterializationBlock {
                    block: SelectedBlockId(2),
                    instructions: vec![X86XorZeroMaterializationInstruction {
                        instruction,
                        disposition: X86XorZeroInstructionDisposition::XorZeroMaterializationV1 {
                            destination,
                            rflags_units: vec![RegisterUnitId(1)],
                            baseline_byte_count: 10,
                            selected_byte_count: 3,
                        },
                    }],
                }],
            }],
        };
        plan.identity = x86_xor_zero_materialization_identity(&plan);
        plan
    }

    #[test]
    fn codec_round_trips_and_authenticates_every_field() {
        let plan = plan();
        let encoded = plan.encode();
        assert_eq!(encoded, plan.encode());
        assert_eq!(X86XorZeroMaterializationPlan::decode(&encoded), Ok(plan));
    }

    #[test]
    fn codec_rejects_framing_and_content_corruption() {
        let encoded = plan().encode();
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            X86XorZeroMaterializationPlan::decode(&wrong_magic),
            Err(X86XorZeroMaterializationDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            X86XorZeroMaterializationPlan::decode(&wrong_version),
            Err(X86XorZeroMaterializationDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            X86XorZeroMaterializationPlan::decode(&encoded[..encoded.len() - 1]),
            Err(X86XorZeroMaterializationDecodeError::Truncated)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            X86XorZeroMaterializationPlan::decode(&trailing),
            Err(X86XorZeroMaterializationDecodeError::TrailingBytes)
        );
        let mut corrupted = encoded;
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        assert_eq!(
            X86XorZeroMaterializationPlan::decode(&corrupted),
            Err(X86XorZeroMaterializationDecodeError::InvalidIdentity)
        );
    }
}
