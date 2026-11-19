//! Selected literal-fold records and their canonical version 13 wire format.
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    SpillChoiceIdentity,
};
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    LiteralFoldIdentity, LiveRangeIdentity, MachineEffectCatalogIdentity, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{MachineId, WorkScheduleIdentity};
use wire::codec_bytes::push_encoded;
use wire::cursor::{Cursor, TakeError, Truncated};
const LITERAL_FOLD_MAGIC: &[u8; 8] = b"OMGLFD\0\0";
const LITERAL_FOLD_VERSION: u32 = 13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LiteralFoldPolicy {
    enabled_rules: u32,
}

impl LiteralFoldPolicy {
    const EXACT_ADD_BIT: u32 = 1 << 0;
    const EXACT_SUBTRACT_BIT: u32 = 1 << 1;
    const COMPARE_BIT: u32 = 1 << 2;
    const EXTENSION_BIT: u32 = 1 << 3;
    const LOAD8_INDEXED_BIT: u32 = 1 << 4;
    const COPY_BIT: u32 = 1 << 5;
    const BYTE_VIEW_ADDRESS_BIT: u32 = 1 << 6;
    const EXACT_DIVIDE_BIT: u32 = 1 << 7;
    const WRAPPING_REMAINDER_BIT: u32 = 1 << 8;
    const BITWISE_AND_ZERO_BIT: u32 = 1 << 9;
    const BITWISE_XOR_ZERO_BIT: u32 = 1 << 10;
    const WRAPPING_ADD_ZERO_BIT: u32 = 1 << 11;
    const BITWISE_AND_ONES_BIT: u32 = 1 << 12;
    const WRAPPING_REMAINDER_ZERO_BIT: u32 = 1 << 13;
    const EXACT_DIVIDE_ZERO_BIT: u32 = 1 << 14;
    const SATURATING_ADD_ZERO_BIT: u32 = 1 << 15;
    const SATURATING_SUBTRACT_ZERO_BIT: u32 = 1 << 16;
    const SATURATING_DIVIDE_ONE_BIT: u32 = 1 << 17;
    const SATURATING_DIVIDE_ZERO_BIT: u32 = 1 << 18;
    const SATURATING_SUBTRACT_ZERO_MINUEND_BIT: u32 = 1 << 19;
    const SATURATING_ADD_UPPER_BOUND_BIT: u32 = 1 << 20;
    const WRAPPING_REMAINDER_MINUS_ONE_BIT: u32 = 1 << 21;
    const SATURATING_SUBTRACT_UPPER_BOUND_BIT: u32 = 1 << 22;
    const KNOWN_BITS: u32 = Self::EXACT_ADD_BIT
        | Self::EXACT_SUBTRACT_BIT
        | Self::COMPARE_BIT
        | Self::EXTENSION_BIT
        | Self::LOAD8_INDEXED_BIT
        | Self::COPY_BIT
        | Self::BYTE_VIEW_ADDRESS_BIT
        | Self::EXACT_DIVIDE_BIT
        | Self::WRAPPING_REMAINDER_BIT
        | Self::BITWISE_AND_ZERO_BIT
        | Self::BITWISE_XOR_ZERO_BIT
        | Self::WRAPPING_ADD_ZERO_BIT
        | Self::BITWISE_AND_ONES_BIT
        | Self::WRAPPING_REMAINDER_ZERO_BIT
        | Self::EXACT_DIVIDE_ZERO_BIT
        | Self::SATURATING_ADD_ZERO_BIT
        | Self::SATURATING_SUBTRACT_ZERO_BIT
        | Self::SATURATING_DIVIDE_ONE_BIT
        | Self::SATURATING_DIVIDE_ZERO_BIT
        | Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT
        | Self::SATURATING_ADD_UPPER_BOUND_BIT
        | Self::WRAPPING_REMAINDER_MINUS_ONE_BIT
        | Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT;

    pub const EXACT_ADD_V1: Self = Self {
        enabled_rules: Self::EXACT_ADD_BIT,
    };
    pub const EXACT_SUBTRACT_V1: Self = Self {
        enabled_rules: Self::EXACT_SUBTRACT_BIT,
    };
    pub const COMPARE_V1: Self = Self {
        enabled_rules: Self::COMPARE_BIT,
    };
    pub const EXTENSION_V1: Self = Self {
        enabled_rules: Self::EXTENSION_BIT,
    };
    pub const LOAD8_INDEXED_V1: Self = Self {
        enabled_rules: Self::LOAD8_INDEXED_BIT,
    };
    pub const COPY_V1: Self = Self {
        enabled_rules: Self::COPY_BIT,
    };
    pub const BYTE_VIEW_ADDRESS_V1: Self = Self {
        enabled_rules: Self::BYTE_VIEW_ADDRESS_BIT,
    };
    pub const EXACT_DIVIDE_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_BIT,
    };
    pub const WRAPPING_REMAINDER_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_BIT,
    };
    pub const BITWISE_AND_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ZERO_BIT,
    };
    pub const BITWISE_XOR_ZERO_V1: Self = Self {
        enabled_rules: Self::BITWISE_XOR_ZERO_BIT,
    };
    pub const WRAPPING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_ADD_ZERO_BIT,
    };
    pub const BITWISE_AND_ONES_V1: Self = Self {
        enabled_rules: Self::BITWISE_AND_ONES_BIT,
    };
    pub const WRAPPING_REMAINDER_ZERO_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_ZERO_BIT,
    };
    pub const EXACT_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::EXACT_DIVIDE_ZERO_BIT,
    };
    pub const SATURATING_ADD_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_ZERO_BIT,
    };
    pub const SATURATING_SUBTRACT_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_ZERO_BIT,
    };
    pub const SATURATING_DIVIDE_ONE_V1: Self = Self {
        enabled_rules: Self::SATURATING_DIVIDE_ONE_BIT,
    };
    pub const SATURATING_DIVIDE_ZERO_V1: Self = Self {
        enabled_rules: Self::SATURATING_DIVIDE_ZERO_BIT,
    };
    pub const SATURATING_SUBTRACT_ZERO_MINUEND_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_ZERO_MINUEND_BIT,
    };
    pub const SATURATING_ADD_UPPER_BOUND_V1: Self = Self {
        enabled_rules: Self::SATURATING_ADD_UPPER_BOUND_BIT,
    };
    pub const WRAPPING_REMAINDER_MINUS_ONE_V1: Self = Self {
        enabled_rules: Self::WRAPPING_REMAINDER_MINUS_ONE_BIT,
    };
    pub const SATURATING_SUBTRACT_UPPER_BOUND_V1: Self = Self {
        enabled_rules: Self::SATURATING_SUBTRACT_UPPER_BOUND_BIT,
    };

    pub const fn empty() -> Self {
        Self { enabled_rules: 0 }
    }

    pub const fn contains(self, other: Self) -> bool {
        other.enabled_rules != 0 && self.enabled_rules & other.enabled_rules == other.enabled_rules
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            enabled_rules: self.enabled_rules | other.enabled_rules,
        }
    }

    pub const fn canonical_bits(self) -> u32 {
        self.enabled_rules
    }

    pub const fn from_canonical_bits(bits: u32) -> Option<Self> {
        if bits == 0 || bits & !Self::KNOWN_BITS != 0 {
            None
        } else {
            Some(Self {
                enabled_rules: bits,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFoldPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub machine_effect_catalog: MachineEffectCatalogIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub work_schedule: WorkScheduleIdentity,
    pub policy: LiteralFoldPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionLiteralFold>,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl LiteralFoldPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_literal_fold_content(self);
        let identity = literal_fold_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        wire::codec_bytes::encode_header(&mut encoded, LITERAL_FOLD_MAGIC, LITERAL_FOLD_VERSION);
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, LiteralFoldDecodeError> {
        let mut cursor = Cursor::new(encoded);
        wire::codec_bytes::decode_header(
            &mut cursor,
            LITERAL_FOLD_MAGIC,
            LITERAL_FOLD_VERSION,
            || LiteralFoldDecodeError::WrongMagic,
            LiteralFoldDecodeError::UnsupportedVersion,
        )?;
        let identity = LiteralFoldIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let machine_effect_catalog = MachineEffectCatalogIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_work = cursor.u32()?;
        let work_schedule = WorkScheduleIdentity::new(raw_work)
            .ok_or(LiteralFoldDecodeError::InvalidWorkSchedule(raw_work))?;
        let policy_bits = cursor.u32()?;
        let policy = LiteralFoldPolicy::from_canonical_bits(policy_bits)
            .ok_or(LiteralFoldDecodeError::UnknownPolicy(policy_bits))?;
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = cursor.u64()?;
            let machine = MachineId::new(raw_machine)
                .ok_or(LiteralFoldDecodeError::InvalidMachineId(raw_machine))?;
            let action_count = cursor.length()?;
            let mut actions = Vec::with_capacity(action_count.min(cursor.remaining()));
            for _ in 0..action_count {
                actions.push(LiteralFoldAction {
                    block: SelectedBlockId(cursor.u32()?),
                    literal_instruction: SelectedInstructionId(cursor.u32()?),
                    victim: VirtualRegisterId(cursor.u32()?),
                    consumer_instruction: SelectedInstructionId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    surviving: VirtualRegisterId(cursor.u32()?),
                    result: match cursor.byte()? {
                        0 => None,
                        1 => Some(VirtualRegisterId(cursor.u32()?)),
                        tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
                    },
                    immediate: cursor.u64()?,
                    immediate_constraint: decode_constraint_key(&mut cursor)?,
                });
            }
            functions.push(FunctionLiteralFold { machine, actions });
        }
        let transformed_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(LiteralFoldDecodeError::TrailingBytes);
        }
        let plan = Self {
            source_selected,
            spill_choices,
            recovery_classifications,
            ranges,
            legality,
            register_environment,
            allocator_availability,
            machine_effect_catalog,
            optimization_unit,
            work_schedule,
            policy,
            budget,
            usage,
            functions,
            transformed_selected,
        };
        if literal_fold_identity(&plan) != identity {
            return Err(LiteralFoldDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiteralFold {
    pub machine: MachineId,
    pub actions: Vec<LiteralFoldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldAction {
    pub block: SelectedBlockId,
    pub literal_instruction: SelectedInstructionId,
    pub victim: VirtualRegisterId,
    pub consumer_instruction: SelectedInstructionId,
    pub surviving: VirtualRegisterId,
    pub result: Option<VirtualRegisterId>,
    pub immediate: u64,
    pub immediate_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralFoldDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u32),
    UnknownOption(u8),
    UnknownConstraintFamily(u8),
    InvalidWorkSchedule(u32),
    InvalidMachineId(u64),
    InvalidBudget,
    InvalidUsage,
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}

impl std::fmt::Display for LiteralFoldDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid terminal literal fold: {self:?}")
    }
}

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, LiteralFoldDecodeError> {
    let family = match cursor.byte()? {
        0 => register_model::RegisterConstraintFamily::Call,
        1 => register_model::RegisterConstraintFamily::Return,
        2 => register_model::RegisterConstraintFamily::SystemCall,
        3 => register_model::RegisterConstraintFamily::InlineAssembly,
        4 => register_model::RegisterConstraintFamily::Instruction,
        tag => return Err(LiteralFoldDecodeError::UnknownConstraintFamily(tag)),
    };
    Ok(RegisterConstraintKey {
        family,
        variant: cursor.u32()?,
    })
}

impl From<Truncated> for LiteralFoldDecodeError {
    fn from(_: Truncated) -> Self {
        Self::Truncated
    }
}

impl From<TakeError> for LiteralFoldDecodeError {
    fn from(error: TakeError) -> Self {
        match error {
            TakeError::Truncated => Self::Truncated,
            TakeError::LengthOverflow => Self::LengthOverflow,
        }
    }
}
pub fn literal_fold_identity(plan: &LiteralFoldPlan) -> LiteralFoldIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-literal-fold.v8\0");
    bytes.extend_from_slice(&encode_terminal_literal_fold_content(plan));
    LiteralFoldIdentity::from_canonical_bytes(&bytes)
}

fn encode_terminal_literal_fold_content(plan: &LiteralFoldPlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.source_selected.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.recovery_classifications.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.machine_effect_catalog.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    push_encoded(&mut bytes, plan.work_schedule.marker().to_le_bytes());
    push_encoded(&mut bytes, plan.policy.canonical_bits().to_le_bytes());
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    length(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        length(&mut bytes, function.actions.len());
        for action in &function.actions {
            push_encoded(&mut bytes, action.block.0.to_le_bytes());
            push_encoded(&mut bytes, action.literal_instruction.0.to_le_bytes());
            push_encoded(&mut bytes, action.victim.0.to_le_bytes());
            push_encoded(&mut bytes, action.consumer_instruction.0.to_le_bytes());
            push_encoded(&mut bytes, action.surviving.0.to_le_bytes());
            match action.result {
                None => bytes.push(0),
                Some(result) => {
                    bytes.push(1);
                    push_encoded(&mut bytes, result.0.to_le_bytes());
                }
            }
            push_encoded(&mut bytes, action.immediate.to_le_bytes());
            register_model::encode_constraint_key(&mut bytes, action.immediate_constraint);
        }
    }
    bytes.extend_from_slice(&plan.transformed_selected.bytes());
    bytes
}

fn length(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("literal-fold identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_v13_literal_fold_record() {
        let plan = LiteralFoldPlan {
            source_selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([2; 32]),
            recovery_classifications: RecoveryClassificationIdentity::from_bytes([3; 32]),
            ranges: LiveRangeIdentity::from_bytes([4; 32]),
            legality: AllocationLegalityIdentity::from_bytes([5; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([6; 32]),
            allocator_availability: AllocatorAvailabilityIdentity::from_bytes([7; 32]),
            machine_effect_catalog: MachineEffectCatalogIdentity::from_bytes([8; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([9; 32]),
            work_schedule: WorkScheduleIdentity::new(1).unwrap(),
            policy: LiteralFoldPolicy::EXACT_ADD_V1,
            budget: OptimizationWorkBudget::new(1, 2, 3, 4, 5).unwrap(),
            usage: OptimizationWorkUsage::default(),
            functions: vec![FunctionLiteralFold {
                machine: MachineId::new(6).unwrap(),
                actions: vec![LiteralFoldAction {
                    block: SelectedBlockId(7),
                    literal_instruction: SelectedInstructionId(8),
                    victim: VirtualRegisterId(9),
                    consumer_instruction: SelectedInstructionId(10),
                    surviving: VirtualRegisterId(11),
                    result: Some(VirtualRegisterId(12)),
                    immediate: 13,
                    immediate_constraint: RegisterConstraintKey {
                        family: register_model::RegisterConstraintFamily::Instruction,
                        variant: 14,
                    },
                }],
            }],
            transformed_selected: SelectedInstructionPlanIdentity::from_bytes([15; 32]),
        };
        let mut bytes = plan.encode();
        assert_eq!(&bytes[..8], b"OMGLFD\0\0");
        assert_eq!(&bytes[8..12], &13u32.to_le_bytes());
        assert_eq!(
            wire::digest_encoder::hex(&bytes[12..44]),
            "21a2c920a5f0116fe3817e8739e23278502401c2b64b1652010e02adf382b8fe"
        );
        assert_eq!(LiteralFoldPlan::decode(&bytes), Ok(plan));
        *bytes.last_mut().unwrap() ^= 1;
        assert_eq!(
            LiteralFoldPlan::decode(&bytes),
            Err(LiteralFoldDecodeError::IdentityMismatch)
        );
    }
}
