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
    Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationAction,
    Aarch64MovnMaterializationAttempt, Aarch64MovnMaterializationAttemptOutcome,
    Aarch64MovnMaterializationBlock, Aarch64MovnMaterializationFunction,
    Aarch64MovnMaterializationIdentity, Aarch64MovnMaterializationInstruction,
    Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationPolicy,
    Aarch64MovnMaterializationRevisionIdentity, Aarch64MovnPatch, Aarch64MovnRecipe,
    PostAllocationMachineIdentity, QualifiedPhysicalWrite, aarch64_movn_materialization_identity,
};

const MAGIC: &[u8; 8] = b"OMGMVN\0\0";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64MovnMaterializationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for Aarch64MovnMaterializationDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid AArch64 MOVN materialization artifact: {self:?}"
        )
    }
}

impl std::error::Error for Aarch64MovnMaterializationDecodeError {}

pub(crate) fn encode(plan: &Aarch64MovnMaterializationPlan) -> Vec<u8> {
    let content = crate::aarch64_movn_identity::encode_content(plan);
    let mut encoded = Vec::with_capacity(44 + content.len());
    encoded.extend_from_slice(MAGIC);
    encoded.extend_from_slice(&VERSION.to_le_bytes());
    encoded.extend_from_slice(&plan.identity.bytes());
    encoded.extend_from_slice(&content);
    encoded
}

pub(crate) fn decode(
    encoded: &[u8],
) -> Result<Aarch64MovnMaterializationPlan, Aarch64MovnMaterializationDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(Aarch64MovnMaterializationDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(Aarch64MovnMaterializationDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = Aarch64MovnMaterializationIdentity::from_bytes(cursor.array()?);
    let source = PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let target = decode_target(&mut cursor)?;
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1,
        _ => return Err(Aarch64MovnMaterializationDecodeError::InvalidField),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| Aarch64MovnMaterializationDecodeError::InvalidField)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| Aarch64MovnMaterializationDecodeError::InvalidField)?;
    let output_revision = Aarch64MovnMaterializationRevisionIdentity::from_bytes(cursor.array()?);
    let attempt_count = cursor.length()?;
    let mut attempts = Vec::with_capacity(attempt_count.min(cursor.remaining()));
    for _ in 0..attempt_count {
        attempts.push(Aarch64MovnMaterializationAttempt {
            iteration: cursor.u64()?,
            input: Aarch64MovnMaterializationRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            literal_bits: cursor.u64()?,
            destination: decode_write(&mut cursor)?,
            baseline_word_count: cursor.byte()?,
            recipe: decode_recipe(&mut cursor)?,
            outcome: match cursor.byte()? {
                0 => Aarch64MovnMaterializationAttemptOutcome::AlreadySelected,
                1 => Aarch64MovnMaterializationAttemptOutcome::BaselineNotLonger,
                2 => Aarch64MovnMaterializationAttemptOutcome::SelectedForRewrite,
                _ => return Err(Aarch64MovnMaterializationDecodeError::InvalidField),
            },
        });
    }
    let action_count = cursor.length()?;
    let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
    for _ in 0..action_count {
        actions.push(Aarch64MovnMaterializationAction {
            iteration: cursor.u64()?,
            input: Aarch64MovnMaterializationRevisionIdentity::from_bytes(cursor.array()?),
            output: Aarch64MovnMaterializationRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            instruction: SelectedInstructionId(cursor.u32()?),
            literal_bits: cursor.u64()?,
            destination: decode_write(&mut cursor)?,
            baseline_word_count: cursor.byte()?,
            recipe: decode_recipe(&mut cursor)?,
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
                    0 => Aarch64MovnInstructionDisposition::RetainedV1,
                    1 => Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
                        literal_bits: cursor.u64()?,
                        destination: decode_write(&mut cursor)?,
                        baseline_word_count: cursor.byte()?,
                        recipe: decode_recipe(&mut cursor)?,
                    },
                    _ => return Err(Aarch64MovnMaterializationDecodeError::InvalidField),
                };
                instructions.push(Aarch64MovnMaterializationInstruction {
                    instruction,
                    disposition,
                });
            }
            blocks.push(Aarch64MovnMaterializationBlock {
                block,
                instructions,
            });
        }
        functions.push(Aarch64MovnMaterializationFunction { machine, blocks });
    }
    if cursor.remaining() != 0 {
        return Err(Aarch64MovnMaterializationDecodeError::TrailingBytes);
    }
    let plan = Aarch64MovnMaterializationPlan {
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
    if plan.identity != aarch64_movn_materialization_identity(&plan) {
        return Err(Aarch64MovnMaterializationDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_write(
    cursor: &mut Cursor<'_>,
) -> Result<QualifiedPhysicalWrite, Aarch64MovnMaterializationDecodeError> {
    Ok(QualifiedPhysicalWrite {
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
            _ => return Err(Aarch64MovnMaterializationDecodeError::InvalidField),
        },
    })
}

fn decode_recipe(
    cursor: &mut Cursor<'_>,
) -> Result<Aarch64MovnRecipe, Aarch64MovnMaterializationDecodeError> {
    let seed_halfword = cursor.byte()?;
    let seed_immediate = cursor.u16()?;
    let patch_count = cursor.length()?;
    let mut patches = Vec::with_capacity(patch_count.min(cursor.remaining()));
    for _ in 0..patch_count {
        patches.push(Aarch64MovnPatch {
            halfword: cursor.byte()?,
            immediate: cursor.u16()?,
        });
    }
    Ok(Aarch64MovnRecipe {
        seed_halfword,
        seed_immediate,
        patches,
    })
}

fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, Aarch64MovnMaterializationDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn machine(cursor: &mut Cursor<'_>) -> Result<MachineId, Aarch64MovnMaterializationDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(Aarch64MovnMaterializationDecodeError::InvalidField)
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, Aarch64MovnMaterializationDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(Aarch64MovnMaterializationDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(Aarch64MovnMaterializationDecodeError::InvalidField),
    };
    let pointer_size = usize::try_from(cursor.u64()?)
        .map_err(|_| Aarch64MovnMaterializationDecodeError::InvalidField)?;
    let pointer_alignment = usize::try_from(cursor.u64()?)
        .map_err(|_| Aarch64MovnMaterializationDecodeError::InvalidField)?;
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
    fn take(&mut self, count: usize) -> Result<&'a [u8], Aarch64MovnMaterializationDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(Aarch64MovnMaterializationDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Aarch64MovnMaterializationDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Aarch64MovnMaterializationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| Aarch64MovnMaterializationDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, Aarch64MovnMaterializationDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, Aarch64MovnMaterializationDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, Aarch64MovnMaterializationDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, Aarch64MovnMaterializationDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn length(&mut self) -> Result<usize, Aarch64MovnMaterializationDecodeError> {
        usize::try_from(self.u64()?)
            .map_err(|_| Aarch64MovnMaterializationDecodeError::InvalidField)
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use omega_register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
        RegisterWriteSemantics,
    };
    use omega_selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    };
    use omega_target::NativeTarget;
    use psi_core::MachineId;

    use crate::{
        Aarch64MovnInstructionDisposition, Aarch64MovnMaterializationBlock,
        Aarch64MovnMaterializationFunction, Aarch64MovnMaterializationIdentity,
        Aarch64MovnMaterializationInstruction, Aarch64MovnMaterializationPlan,
        Aarch64MovnMaterializationPolicy, Aarch64MovnMaterializationRevisionIdentity,
        Aarch64MovnPatch, Aarch64MovnRecipe, PostAllocationMachineIdentity, QualifiedPhysicalWrite,
        aarch64_movn_materialization_identity,
    };

    use super::Aarch64MovnMaterializationDecodeError;

    fn plan() -> Aarch64MovnMaterializationPlan {
        let instruction = SelectedInstructionId(3);
        let destination = QualifiedPhysicalWrite {
            instruction,
            operand: 0,
            virtual_register: VirtualRegisterId(4),
            class: RegisterClassId(0),
            view: RegisterViewId(0),
            storage_units: vec![RegisterUnitId(0)],
            write_units: vec![RegisterUnitId(0)],
            write_semantics: RegisterWriteSemantics::ExactView,
        };
        let recipe = Aarch64MovnRecipe {
            seed_halfword: 0,
            seed_immediate: 0x1234,
            patches: vec![Aarch64MovnPatch {
                halfword: 2,
                immediate: 0xabcd,
            }],
        };
        let mut plan = Aarch64MovnMaterializationPlan {
            identity: Aarch64MovnMaterializationIdentity::from_bytes([0; 32]),
            source: PostAllocationMachineIdentity::from_bytes([1; 32]),
            selected: SelectedInstructionPlanIdentity::from_bytes([2; 32]),
            target: NativeTarget::linux_arm64(),
            physical_register_model: PhysicalRegisterModelIdentity::from_bytes([3; 32]),
            policy: Aarch64MovnMaterializationPolicy::Aarch64SelectShortestMovnSeededI64MaterializationV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage::default(),
            output_revision: Aarch64MovnMaterializationRevisionIdentity::from_bytes([4; 32]),
            attempts: vec![],
            actions: vec![],
            functions: vec![Aarch64MovnMaterializationFunction {
                machine: MachineId::new(1).unwrap(),
                blocks: vec![Aarch64MovnMaterializationBlock {
                    block: SelectedBlockId(2),
                    instructions: vec![Aarch64MovnMaterializationInstruction {
                        instruction,
                        disposition: Aarch64MovnInstructionDisposition::MovnSeededMaterializationV1 {
                            literal_bits: 0xffff_abcd_ffff_edcb,
                            destination,
                            baseline_word_count: 4,
                            recipe,
                        },
                    }],
                }],
            }],
        };
        plan.identity = aarch64_movn_materialization_identity(&plan);
        plan
    }

    #[test]
    fn codec_is_deterministic_and_round_trips_every_field() {
        let plan = plan();
        let encoded = plan.encode();
        assert_eq!(encoded, plan.encode());
        assert_eq!(Aarch64MovnMaterializationPlan::decode(&encoded), Ok(plan));
    }

    #[test]
    fn codec_rejects_corruption_and_framing_failures() {
        let encoded = plan().encode();
        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            Aarch64MovnMaterializationPlan::decode(&wrong_magic),
            Err(Aarch64MovnMaterializationDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            Aarch64MovnMaterializationPlan::decode(&wrong_version),
            Err(Aarch64MovnMaterializationDecodeError::UnsupportedVersion(2))
        );
        assert_eq!(
            Aarch64MovnMaterializationPlan::decode(&encoded[..encoded.len() - 1]),
            Err(Aarch64MovnMaterializationDecodeError::Truncated)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            Aarch64MovnMaterializationPlan::decode(&trailing),
            Err(Aarch64MovnMaterializationDecodeError::TrailingBytes)
        );
        let mut corrupted = encoded;
        let last = corrupted.len() - 1;
        corrupted[last] ^= 1;
        assert_eq!(
            Aarch64MovnMaterializationPlan::decode(&corrupted),
            Err(Aarch64MovnMaterializationDecodeError::InvalidIdentity)
        );
    }
}
