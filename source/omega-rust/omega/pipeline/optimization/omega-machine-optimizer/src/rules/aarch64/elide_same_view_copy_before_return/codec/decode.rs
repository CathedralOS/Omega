use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
use omega_regalloc::LivenessIdentity;
use omega_register_model::{
    PhysicalRegisterModelIdentity, RegisterClassId, RegisterUnitId, RegisterViewId,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{MachineId, ValueId};

use crate::PostAllocationMachineIdentity;

use super::super::*;
use super::{
    cursor::Cursor,
    encode::{MAGIC, VERSION},
};

pub(super) fn decode(
    encoded: &[u8],
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionDecodeError> {
    let mut cursor = Cursor::new(encoded);
    if cursor.take(8)? != MAGIC {
        return Err(Aarch64SameViewCopyElisionDecodeError::WrongMagic);
    }
    let version = cursor.u32()?;
    if version != VERSION {
        return Err(Aarch64SameViewCopyElisionDecodeError::UnsupportedVersion(
            version,
        ));
    }
    let identity = Aarch64SameViewCopyElisionIdentity::from_bytes(cursor.array()?);
    let plan = decode_content(&mut cursor, identity)?;
    if cursor.remaining() != 0 {
        return Err(Aarch64SameViewCopyElisionDecodeError::TrailingBytes);
    }
    Ok(plan)
}

pub(super) fn authenticate(
    plan: Aarch64SameViewCopyElisionPlan,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionDecodeError> {
    if plan.identity != aarch64_same_view_copy_elision_identity(&plan) {
        return Err(Aarch64SameViewCopyElisionDecodeError::InvalidIdentity);
    }
    Ok(plan)
}

fn decode_content(
    cursor: &mut Cursor<'_>,
    identity: Aarch64SameViewCopyElisionIdentity,
) -> Result<Aarch64SameViewCopyElisionPlan, Aarch64SameViewCopyElisionDecodeError> {
    let source = PostAllocationMachineIdentity::from_bytes(cursor.array()?);
    let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
    let liveness = LivenessIdentity::from_bytes(cursor.array()?);
    let target = decode_target(cursor)?;
    let physical_register_model = PhysicalRegisterModelIdentity::from_bytes(cursor.array()?);
    let policy = match cursor.byte()? {
        0 => Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeReturnV1,
        1 => Aarch64SameViewCopyElisionPolicy::Aarch64ElideSameViewCopyI64BeforeCompareZeroV1,
        _ => return Err(Aarch64SameViewCopyElisionDecodeError::InvalidField),
    };
    let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
        .map_err(|_| Aarch64SameViewCopyElisionDecodeError::InvalidField)?;
    let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
        .map_err(|_| Aarch64SameViewCopyElisionDecodeError::InvalidField)?;
    let output_revision = Aarch64SameViewCopyElisionRevisionIdentity::from_bytes(cursor.array()?);
    Ok(Aarch64SameViewCopyElisionPlan {
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
        attempts: decode_attempts(cursor)?,
        actions: decode_actions(cursor)?,
        functions: decode_functions(cursor)?,
    })
}

fn decode_attempts(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<Aarch64SameViewCopyElisionAttempt>, Aarch64SameViewCopyElisionDecodeError> {
    let count = cursor.length()?;
    let mut attempts = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        attempts.push(Aarch64SameViewCopyElisionAttempt {
            iteration: cursor.u64()?,
            input: Aarch64SameViewCopyElisionRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            copy: SelectedInstructionId(cursor.u32()?),
            consumer: SelectedInstructionId(cursor.u32()?),
            outcome: match cursor.byte()? {
                0 => Aarch64SameViewCopyElisionAttemptOutcome::AlreadyElided,
                1 => Aarch64SameViewCopyElisionAttemptOutcome::DifferentPhysicalStorage,
                2 => Aarch64SameViewCopyElisionAttemptOutcome::DestinationNotConsumed,
                3 => Aarch64SameViewCopyElisionAttemptOutcome::SemanticProvenance,
                4 => Aarch64SameViewCopyElisionAttemptOutcome::SelectedForElision,
                _ => return Err(Aarch64SameViewCopyElisionDecodeError::InvalidField),
            },
        });
    }
    Ok(attempts)
}

fn decode_actions(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<Aarch64SameViewCopyElisionAction>, Aarch64SameViewCopyElisionDecodeError> {
    let count = cursor.length()?;
    let mut actions = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        actions.push(Aarch64SameViewCopyElisionAction {
            iteration: cursor.u64()?,
            input: Aarch64SameViewCopyElisionRevisionIdentity::from_bytes(cursor.array()?),
            output: Aarch64SameViewCopyElisionRevisionIdentity::from_bytes(cursor.array()?),
            machine: machine(cursor)?,
            block: SelectedBlockId(cursor.u32()?),
            copy: SelectedInstructionId(cursor.u32()?),
            consumer: SelectedInstructionId(cursor.u32()?),
            source: decode_operand(cursor)?,
            destination: decode_operand(cursor)?,
            consumed: decode_operand(cursor)?,
            source_value: ValueId::new(cursor.u64()?)
                .ok_or(Aarch64SameViewCopyElisionDecodeError::InvalidField)?,
        });
    }
    Ok(actions)
}

fn decode_functions(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<Aarch64SameViewCopyElisionFunction>, Aarch64SameViewCopyElisionDecodeError> {
    let count = cursor.length()?;
    let mut functions = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        let machine = machine(cursor)?;
        let block_count = cursor.length()?;
        let mut blocks = Vec::with_capacity(block_count.min(cursor.remaining()));
        for _ in 0..block_count {
            let block = SelectedBlockId(cursor.u32()?);
            let instruction_count = cursor.length()?;
            let mut instructions = Vec::with_capacity(instruction_count.min(cursor.remaining()));
            for _ in 0..instruction_count {
                let instruction = SelectedInstructionId(cursor.u32()?);
                let disposition = match cursor.byte()? {
                    0 => Aarch64SameViewCopyInstructionDisposition::RetainedV1,
                    1 => Aarch64SameViewCopyInstructionDisposition::ElidedSameViewCopyI64V1 {
                        consumer: SelectedInstructionId(cursor.u32()?),
                    },
                    _ => return Err(Aarch64SameViewCopyElisionDecodeError::InvalidField),
                };
                instructions.push(Aarch64SameViewCopyElisionInstruction {
                    instruction,
                    disposition,
                });
            }
            blocks.push(Aarch64SameViewCopyElisionBlock {
                block,
                instructions,
            });
        }
        functions.push(Aarch64SameViewCopyElisionFunction { machine, blocks });
    }
    Ok(functions)
}

fn decode_operand(
    cursor: &mut Cursor<'_>,
) -> Result<QualifiedPhysicalOperand, Aarch64SameViewCopyElisionDecodeError> {
    Ok(QualifiedPhysicalOperand {
        instruction: SelectedInstructionId(cursor.u32()?),
        operand: cursor.u16()?,
        virtual_register: VirtualRegisterId(cursor.u32()?),
        class: RegisterClassId(cursor.u16()?),
        view: RegisterViewId(cursor.u16()?),
        storage_units: decode_units(cursor)?,
    })
}

fn decode_units(
    cursor: &mut Cursor<'_>,
) -> Result<Vec<RegisterUnitId>, Aarch64SameViewCopyElisionDecodeError> {
    let count = cursor.length()?;
    let mut units = Vec::with_capacity(count.min(cursor.remaining()));
    for _ in 0..count {
        units.push(RegisterUnitId(cursor.u16()?));
    }
    Ok(units)
}

fn machine(cursor: &mut Cursor<'_>) -> Result<MachineId, Aarch64SameViewCopyElisionDecodeError> {
    MachineId::new(cursor.u64()?).ok_or(Aarch64SameViewCopyElisionDecodeError::InvalidField)
}

fn decode_target(
    cursor: &mut Cursor<'_>,
) -> Result<NativeTarget, Aarch64SameViewCopyElisionDecodeError> {
    let architecture = match cursor.byte()? {
        0 => Architecture::Aarch64,
        1 => Architecture::X86_64,
        _ => return Err(Aarch64SameViewCopyElisionDecodeError::InvalidField),
    };
    let object_format = match cursor.byte()? {
        0 => ObjectFormat::Elf,
        1 => ObjectFormat::MachO,
        2 => ObjectFormat::Coff,
        _ => return Err(Aarch64SameViewCopyElisionDecodeError::InvalidField),
    };
    Ok(NativeTarget {
        architecture,
        object_format,
        pointer_size: usize::try_from(cursor.u64()?)
            .map_err(|_| Aarch64SameViewCopyElisionDecodeError::InvalidField)?,
        pointer_alignment: usize::try_from(cursor.u64()?)
            .map_err(|_| Aarch64SameViewCopyElisionDecodeError::InvalidField)?,
    })
}
