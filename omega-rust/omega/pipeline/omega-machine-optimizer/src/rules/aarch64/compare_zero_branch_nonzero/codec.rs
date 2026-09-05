use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_selected_instructions_to_register_homes::LivenessIdentity;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{EdgeId, MachineId};

use crate::{
    Aarch64CbnzFusionAction, Aarch64CbnzFusionAttempt, Aarch64CbnzFusionAttemptOutcome,
    Aarch64CbnzFusionBlock, Aarch64CbnzFusionFunction, Aarch64CbnzFusionIdentity,
    Aarch64CbnzFusionInstruction, Aarch64CbnzFusionPlan, Aarch64CbnzFusionPolicy,
    Aarch64CbnzFusionRevisionIdentity, Aarch64CbnzInstructionDisposition,
    PostAllocationMachineIdentity, QualifiedPhysicalRead, aarch64_cbnz_fusion_identity,
};

const MAGIC: &[u8; 8] = b"OMGCNZ\0\0";
const VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aarch64CbnzFusionDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    InvalidField,
    InvalidIdentity,
    TrailingBytes,
}

impl std::fmt::Display for Aarch64CbnzFusionDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid AArch64 CBNZ fusion artifact: {self:?}")
    }
}

impl std::error::Error for Aarch64CbnzFusionDecodeError {}

pub(crate) fn encode(plan: &Aarch64CbnzFusionPlan) -> Vec<u8> {
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
) -> Result<Aarch64CbnzFusionPlan, Aarch64CbnzFusionDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(Aarch64CbnzFusionDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(Aarch64CbnzFusionDecodeError::UnsupportedVersion(version));
    }
    let identity = Aarch64CbnzFusionIdentity::from_bytes(cursor.array()?);
    let source = PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let liveness = LivenessIdentity::from_bytes(cursor.array()?);
    let target = decode_target(&mut cursor)?;
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => Aarch64CbnzFusionPolicy::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
        _ => return Err(Aarch64CbnzFusionDecodeError::InvalidField),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| Aarch64CbnzFusionDecodeError::InvalidField)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| Aarch64CbnzFusionDecodeError::InvalidField)?;
    let output_revision = Aarch64CbnzFusionRevisionIdentity::from_bytes(cursor.array()?);
    let attempt_count = cursor.length()?;
    let mut attempts = Vec::with_capacity(attempt_count.min(cursor.remaining()));
    for _ in 0..attempt_count {
        attempts.push(Aarch64CbnzFusionAttempt {
            iteration: cursor.u64()?,
            input: Aarch64CbnzFusionRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            compare: SelectedInstructionId(cursor.u32()?),
            branch: SelectedInstructionId(cursor.u32()?),
            outcome: match cursor.byte()? {
                0 => Aarch64CbnzFusionAttemptOutcome::AlreadyFused,
                1 => Aarch64CbnzFusionAttemptOutcome::CompareCarriesFuel,
                2 => Aarch64CbnzFusionAttemptOutcome::NzcvLiveOut,
                3 => Aarch64CbnzFusionAttemptOutcome::SelectedForFusion,
                _ => return Err(Aarch64CbnzFusionDecodeError::InvalidField),
            },
        });
    }
    let action_count = cursor.length()?;
    let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
    for _ in 0..action_count {
        actions.push(Aarch64CbnzFusionAction {
            iteration: cursor.u64()?,
            input: Aarch64CbnzFusionRevisionIdentity::from_bytes(cursor.array()?),
            output: Aarch64CbnzFusionRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(&mut cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            compare: SelectedInstructionId(cursor.u32()?),
            branch: SelectedInstructionId(cursor.u32()?),
            source_read: decode_read(&mut cursor)?,
            nzcv_units: decode_units(&mut cursor)?,
            pc_units: decode_units(&mut cursor)?,
            when_nonzero_edge: edge(&mut cursor)?,
            when_nonzero_block: SelectedBlockId(cursor.u32()?),
            when_zero_edge: edge(&mut cursor)?,
            when_zero_block: SelectedBlockId(cursor.u32()?),
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
                    0 => Aarch64CbnzInstructionDisposition::RetainedV1,
                    1 => Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 {
                        consumer: SelectedInstructionId(cursor.u32()?),
                    },
                    2 => Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                        compare: SelectedInstructionId(cursor.u32()?),
                        source_read: decode_read(&mut cursor)?,
                    },
                    _ => return Err(Aarch64CbnzFusionDecodeError::InvalidField),
                };
                instructions.push(Aarch64CbnzFusionInstruction {
                    instruction,
                    disposition,
                });
            }
            blocks.push(Aarch64CbnzFusionBlock {
                block,
                instructions,
            });
        }
        functions.push(Aarch64CbnzFusionFunction { machine, blocks });
    }
    if cursor.remaining() != 0 {
        return Err(Aarch64CbnzFusionDecodeError::TrailingBytes);
    }
    let plan = Aarch64CbnzFusionPlan {
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
    if plan.identity != aarch64_cbnz_fusion_identity(&plan) {
        return Err(Aarch64CbnzFusionDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_read(
    cursor: &mut Cursor<'_>,
) -> Result<QualifiedPhysicalRead, Aarch64CbnzFusionDecodeError> {
    Ok(QualifiedPhysicalRead {
        source_instruction: SelectedInstructionId(cursor.u32()?),
        operand: cursor.u16()?,
        virtual_register: VirtualRegisterId(cursor.u32()?),
        class: RegisterClassId(cursor.u16()?),
        view: RegisterViewId(cursor.u16()?),
        units: decode_units(cursor)?,
    })
}

fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, Aarch64CbnzFusionDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn machine(cursor: &mut Cursor<'_>) -> Result<MachineId, Aarch64CbnzFusionDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(Aarch64CbnzFusionDecodeError::InvalidField)
}

fn edge(cursor: &mut Cursor<'_>) -> Result<EdgeId, Aarch64CbnzFusionDecodeError> {
    EdgeId::new(cursor.u64()?).ok_or(Aarch64CbnzFusionDecodeError::InvalidField)
}

fn decode_target(cursor: &mut Cursor<'_>) -> Result<NativeTarget, Aarch64CbnzFusionDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(Aarch64CbnzFusionDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(Aarch64CbnzFusionDecodeError::InvalidField),
    };
    let pointer_size =
        usize::try_from(cursor.u64()?).map_err(|_| Aarch64CbnzFusionDecodeError::InvalidField)?;
    let pointer_alignment =
        usize::try_from(cursor.u64()?).map_err(|_| Aarch64CbnzFusionDecodeError::InvalidField)?;
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
    fn take(&mut self, count: usize) -> Result<&'a [u8], Aarch64CbnzFusionDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(Aarch64CbnzFusionDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(Aarch64CbnzFusionDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], Aarch64CbnzFusionDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| Aarch64CbnzFusionDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, Aarch64CbnzFusionDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, Aarch64CbnzFusionDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, Aarch64CbnzFusionDecodeError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, Aarch64CbnzFusionDecodeError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn length(&mut self) -> Result<usize, Aarch64CbnzFusionDecodeError> {
        usize::try_from(self.u64()?).map_err(|_| Aarch64CbnzFusionDecodeError::InvalidField)
    }
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use omega_register_model::{
        PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
    };
    use omega_selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
    };
    use omega_selected_instructions_to_register_homes::LivenessIdentity;
    use omega_target::NativeTarget;
    use psi_core::{EdgeId, MachineId};

    use crate::{
        Aarch64CbnzFusionAction, Aarch64CbnzFusionAttempt, Aarch64CbnzFusionAttemptOutcome,
        Aarch64CbnzFusionBlock, Aarch64CbnzFusionFunction, Aarch64CbnzFusionIdentity,
        Aarch64CbnzFusionInstruction, Aarch64CbnzFusionPlan, Aarch64CbnzFusionPolicy,
        Aarch64CbnzFusionRevisionIdentity, Aarch64CbnzInstructionDisposition,
        PostAllocationMachineIdentity, QualifiedPhysicalRead, aarch64_cbnz_fusion_identity,
    };

    use super::Aarch64CbnzFusionDecodeError;

    const POLICY_OFFSET: usize = 190;

    fn identity(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn plan() -> Aarch64CbnzFusionPlan {
        let machine = MachineId::new(1).unwrap();
        let block = SelectedBlockId(2);
        let compare = SelectedInstructionId(3);
        let branch = SelectedInstructionId(4);
        let source_read = QualifiedPhysicalRead {
            source_instruction: compare,
            operand: 0,
            virtual_register: VirtualRegisterId(5),
            class: RegisterClassId(6),
            view: RegisterViewId(7),
            units: vec![RegisterUnitId(8)],
        };
        let functions = vec![Aarch64CbnzFusionFunction {
            machine,
            blocks: vec![Aarch64CbnzFusionBlock {
                block,
                instructions: vec![
                    Aarch64CbnzFusionInstruction {
                        instruction: compare,
                        disposition: Aarch64CbnzInstructionDisposition::ElidedCompareI64ZeroV1 {
                            consumer: branch,
                        },
                    },
                    Aarch64CbnzFusionInstruction {
                        instruction: branch,
                        disposition:
                            Aarch64CbnzInstructionDisposition::FusedBranchNonZeroToCbnzV1 {
                                compare,
                                source_read: source_read.clone(),
                            },
                    },
                ],
            }],
        }];
        let source = PostAllocationMachineIdentity::from_bytes(identity(10));
        let selected = SelectedInstructionPlanIdentity::from_bytes(identity(11));
        let liveness = LivenessIdentity::from_bytes(identity(12));
        let target = NativeTarget::linux_arm64();
        let physical = PhysicalRegisterModelIdentity::from_bytes(identity(13));
        let input = Aarch64CbnzFusionRevisionIdentity::from_bytes(identity(14));
        let output =
            crate::rules::aarch64::compare_zero_branch_nonzero::identity::revision_identity(
                source, selected, liveness, target, physical, &functions,
            );
        let mut plan = Aarch64CbnzFusionPlan {
            identity: Aarch64CbnzFusionIdentity::from_bytes([0; 32]),
            source,
            selected,
            liveness,
            target,
            physical_register_model: physical,
            policy: Aarch64CbnzFusionPolicy::Aarch64FuseCompareI64ZeroBranchNonZeroToCbnzV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 10).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 1,
                commits: 1,
                iterations: 2,
            },
            output_revision: output,
            attempts: vec![Aarch64CbnzFusionAttempt {
                iteration: 1,
                input,
                machine,
                block,
                compare,
                branch,
                outcome: Aarch64CbnzFusionAttemptOutcome::SelectedForFusion,
            }],
            actions: vec![Aarch64CbnzFusionAction {
                iteration: 1,
                input,
                output,
                machine,
                block,
                compare,
                branch,
                source_read,
                nzcv_units: vec![RegisterUnitId(9)],
                pc_units: vec![RegisterUnitId(10)],
                when_nonzero_edge: EdgeId::new(11).unwrap(),
                when_nonzero_block: SelectedBlockId(12),
                when_zero_edge: EdgeId::new(13).unwrap(),
                when_zero_block: SelectedBlockId(14),
            }],
            functions,
        };
        plan.identity = aarch64_cbnz_fusion_identity(&plan);
        plan
    }

    #[test]
    fn cbnz_fusion_codec_is_deterministic_and_round_trips_every_field() {
        let plan = plan();
        let first = plan.encode();
        let second = plan.encode();
        assert_eq!(first, second);
        assert_eq!(Aarch64CbnzFusionPlan::decode(&first), Ok(plan));
    }

    #[test]
    fn cbnz_fusion_codec_rejects_bad_framing_and_closed_policy() {
        let encoded = plan().encode();
        assert_eq!(encoded[POLICY_OFFSET], 0);

        let mut wrong_magic = encoded.clone();
        wrong_magic[0] ^= 0xff;
        assert_eq!(
            Aarch64CbnzFusionPlan::decode(&wrong_magic),
            Err(Aarch64CbnzFusionDecodeError::WrongMagic)
        );
        let mut wrong_version = encoded.clone();
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            Aarch64CbnzFusionPlan::decode(&wrong_version),
            Err(Aarch64CbnzFusionDecodeError::UnsupportedVersion(2))
        );
        let mut policy = encoded.clone();
        policy[POLICY_OFFSET] = 0xff;
        assert_eq!(
            Aarch64CbnzFusionPlan::decode(&policy),
            Err(Aarch64CbnzFusionDecodeError::InvalidField)
        );
        assert_eq!(
            Aarch64CbnzFusionPlan::decode(&encoded[..encoded.len() - 1]),
            Err(Aarch64CbnzFusionDecodeError::Truncated)
        );
        let mut trailing = encoded;
        trailing.push(0);
        assert_eq!(
            Aarch64CbnzFusionPlan::decode(&trailing),
            Err(Aarch64CbnzFusionDecodeError::TrailingBytes)
        );
    }

    #[test]
    fn cbnz_fusion_codec_authenticates_header_and_qualified_physical_read() {
        let encoded = plan().encode();
        for offset in [12, 44, 484] {
            let mut corrupted = encoded.clone();
            corrupted[offset] ^= 0x80;
            assert_eq!(
                Aarch64CbnzFusionPlan::decode(&corrupted),
                Err(Aarch64CbnzFusionDecodeError::InvalidIdentity),
                "identity-bearing byte {offset} was accepted"
            );
        }
    }
}
