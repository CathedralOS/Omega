//! Pressure-recovery records and their unchanged v2 canonical codec.
//! Decoding establishes byte identity only, never a completed rewrite run.
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    SpillChoiceIdentity,
};
use identity::encode_terminal_pressure_rematerialization_content;
pub use identity::pressure_rematerialization_identity;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, PressureRematerializationIdentity, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{IntegerValue, MachineId, ValueId, WorkScheduleIdentity};
use wire::cursor::{Cursor, TakeError, Truncated};
const MAGIC: &[u8; 8] = b"OMGREM\0\0";
const VERSION: u32 = 2;

use crate::PressureRematerializationPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRematerializationPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub policy: PressureRematerializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionPressureRematerialization>,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl PressureRematerializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_pressure_rematerialization_content(self);
        let identity = pressure_rematerialization_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        wire::codec_bytes::encode_header(&mut encoded, MAGIC, VERSION);
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PressureRematerializationDecodeError> {
        let mut cursor = Cursor::new(encoded);
        wire::codec_bytes::decode_header(
            &mut cursor,
            MAGIC,
            VERSION,
            || PressureRematerializationDecodeError::WrongMagic,
            PressureRematerializationDecodeError::UnsupportedVersion,
        )?;
        let identity = PressureRematerializationIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_work = cursor.u32()?;
        let work_schedule = WorkScheduleIdentity::new(raw_work).ok_or(
            PressureRematerializationDecodeError::InvalidWorkSchedule(raw_work),
        )?;
        let policy = match cursor.byte()? {
            0 => PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            1 => PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            tag => return Err(PressureRematerializationDecodeError::UnknownPolicy(tag)),
        };
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| PressureRematerializationDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| PressureRematerializationDecodeError::InvalidUsage)?;
        let count = cursor.length()?;
        let mut functions = Vec::with_capacity(count.min(cursor.remaining()));
        for _ in 0..count {
            let raw_machine = cursor.u64()?;
            let machine = MachineId::new(raw_machine).ok_or(
                PressureRematerializationDecodeError::InvalidMachineId(raw_machine),
            )?;
            let action = match cursor.byte()? {
                0 => None,
                1 => {
                    let block = SelectedBlockId(cursor.u32()?);
                    let pressure_point = LiveRangePoint(cursor.u32()?);
                    let victim = VirtualRegisterId(cursor.u32()?);
                    let current_view = RegisterViewId(cursor.u16()?);
                    let reclaimed_view = RegisterViewId(cursor.u16()?);
                    let original_materialize = SelectedInstructionId(cursor.u32()?);
                    let source_value = ValueId::new(cursor.u64()?)
                        .ok_or(PressureRematerializationDecodeError::InvalidValueId)?;
                    let value = decode_integer_value(&mut cursor)?;
                    let rewrite_count = cursor.length()?;
                    let mut rewrites = Vec::with_capacity(rewrite_count.min(cursor.remaining()));
                    for _ in 0..rewrite_count {
                        rewrites.push(PressureRematerializationRewrite {
                            point: LiveRangePoint(cursor.u32()?),
                            instruction: SelectedInstructionId(cursor.u32()?),
                            operand: cursor.u16()?,
                        });
                    }
                    Some(PressureRematerializationAction {
                        block,
                        pressure_point,
                        victim,
                        current_view,
                        reclaimed_view,
                        original_materialize,
                        source_value,
                        value,
                        rewrites,
                        fresh_materialize: SelectedInstructionId(u32::from_le_bytes(
                            cursor.array()?,
                        )),
                        result_virtual_register: VirtualRegisterId(u32::from_le_bytes(
                            cursor.array()?,
                        )),
                        materialize_constraint: decode_constraint_key(&mut cursor)?,
                    })
                }
                tag => {
                    return Err(PressureRematerializationDecodeError::UnknownOption(tag));
                }
            };
            functions.push(FunctionPressureRematerialization { machine, action });
        }
        let transformed_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(PressureRematerializationDecodeError::TrailingBytes);
        }
        let plan = Self {
            source_selected,
            spill_choices,
            recovery_classifications,
            ranges,
            legality,
            register_environment,
            allocator_availability,
            optimization_unit,
            work_schedule,
            policy,
            budget,
            usage,
            functions,
            transformed_selected,
        };
        if pressure_rematerialization_identity(&plan) != identity {
            return Err(PressureRematerializationDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionPressureRematerialization {
    pub machine: MachineId,
    pub action: Option<PressureRematerializationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PressureRematerializationAction {
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub victim: VirtualRegisterId,
    pub current_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
    pub original_materialize: SelectedInstructionId,
    pub source_value: ValueId,
    pub value: IntegerValue,
    pub rewrites: Vec<PressureRematerializationRewrite>,
    pub fresh_materialize: SelectedInstructionId,
    pub result_virtual_register: VirtualRegisterId,
    pub materialize_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PressureRematerializationRewrite {
    pub point: LiveRangePoint,
    pub instruction: SelectedInstructionId,
    pub operand: u16,
}

pub(crate) mod identity {
    use crate::{PressureRematerializationPlan, PressureRematerializationPolicy};
    use selected_instructions::PressureRematerializationIdentity;
    use wire::codec_bytes::push_encoded;

    pub fn pressure_rematerialization_identity(
        plan: &PressureRematerializationPlan,
    ) -> PressureRematerializationIdentity {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"omega.terminal-pressure-rematerialization.v2\0");
        bytes.extend_from_slice(&encode_terminal_pressure_rematerialization_content(plan));
        PressureRematerializationIdentity::from_canonical_bytes(&bytes)
    }

    pub(crate) fn encode_terminal_pressure_rematerialization_content(
        plan: &PressureRematerializationPlan,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&plan.source_selected.bytes());
        bytes.extend_from_slice(&plan.spill_choices.bytes());
        bytes.extend_from_slice(&plan.recovery_classifications.bytes());
        bytes.extend_from_slice(&plan.ranges.bytes());
        bytes.extend_from_slice(&plan.legality.bytes());
        bytes.extend_from_slice(&plan.register_environment.bytes());
        bytes.extend_from_slice(&plan.allocator_availability.bytes());
        bytes.extend_from_slice(&plan.optimization_unit.bytes());
        push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
        bytes.push(match plan.policy {
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1 => 0,
            PressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1 => 1,
        });
        bytes.extend_from_slice(&plan.budget.encode());
        bytes.extend_from_slice(&plan.usage.encode());
        bytes.extend_from_slice(
            &u64::try_from(plan.functions.len())
                .expect("function count fits u64")
                .to_le_bytes(),
        );
        for function in &plan.functions {
            push_encoded(&mut bytes, function.machine.get().to_le_bytes());
            match &function.action {
                None => bytes.push(0),
                Some(action) => {
                    bytes.push(1);
                    push_encoded(&mut bytes, action.block.0.to_le_bytes());
                    push_encoded(&mut bytes, action.pressure_point.0.to_le_bytes());
                    push_encoded(&mut bytes, action.victim.0.to_le_bytes());
                    push_encoded(&mut bytes, action.current_view.0.to_le_bytes());
                    push_encoded(&mut bytes, action.reclaimed_view.0.to_le_bytes());
                    push_encoded(&mut bytes, action.original_materialize.0.to_le_bytes());
                    push_encoded(&mut bytes, action.source_value.get().to_le_bytes());
                    match action.value {
                        semantic_vocabulary::IntegerValue::Unsigned(value) => {
                            push_encoded(&mut bytes, value.to_le_bytes());
                            bytes.push(0);
                        }
                        semantic_vocabulary::IntegerValue::Signed(value) => {
                            push_encoded(&mut bytes, value.to_le_bytes());
                            bytes.push(1);
                        }
                    }
                    bytes.extend_from_slice(
                        &u64::try_from(action.rewrites.len())
                            .expect("rewrite count fits u64")
                            .to_le_bytes(),
                    );
                    for rewrite in &action.rewrites {
                        push_encoded(&mut bytes, rewrite.point.0.to_le_bytes());
                        push_encoded(&mut bytes, rewrite.instruction.0.to_le_bytes());
                        push_encoded(&mut bytes, rewrite.operand.to_le_bytes());
                    }
                    push_encoded(&mut bytes, action.fresh_materialize.0.to_le_bytes());
                    push_encoded(&mut bytes, action.result_virtual_register.0.to_le_bytes());
                    register_model::encode_constraint_key(
                        &mut bytes,
                        action.materialize_constraint,
                    );
                }
            }
        }
        bytes.extend_from_slice(&plan.transformed_selected.bytes());
        bytes
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureRematerializationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    UnknownConstraintFamily(u8),
    UnknownIntegerValue(u8),
    InvalidWorkSchedule(u32),
    InvalidMachineId(u64),
    InvalidValueId,
    InvalidBudget,
    InvalidUsage,
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}
crate::decode_error_display!(
    PressureRematerializationDecodeError,
    "invalid terminal pressure rematerialization"
);

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, PressureRematerializationDecodeError> {
    let family = match cursor.byte()? {
        0 => register_model::RegisterConstraintFamily::Call,
        1 => register_model::RegisterConstraintFamily::Return,
        2 => register_model::RegisterConstraintFamily::SystemCall,
        3 => register_model::RegisterConstraintFamily::InlineAssembly,
        4 => register_model::RegisterConstraintFamily::Instruction,
        tag => {
            return Err(PressureRematerializationDecodeError::UnknownConstraintFamily(tag));
        }
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

fn decode_integer_value(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, PressureRematerializationDecodeError> {
    let value = cursor.u128()?;
    match cursor.byte()? {
        0 => Ok(IntegerValue::Unsigned(value)),
        1 => Ok(IntegerValue::Signed(value as i128)),
        tag => Err(PressureRematerializationDecodeError::UnknownIntegerValue(
            tag,
        )),
    }
}

impl From<Truncated> for PressureRematerializationDecodeError {
    fn from(_: Truncated) -> Self {
        Self::Truncated
    }
}

impl From<TakeError> for PressureRematerializationDecodeError {
    fn from(error: TakeError) -> Self {
        match error {
            TakeError::Truncated => Self::Truncated,
            TakeError::LengthOverflow => Self::LengthOverflow,
        }
    }
}
