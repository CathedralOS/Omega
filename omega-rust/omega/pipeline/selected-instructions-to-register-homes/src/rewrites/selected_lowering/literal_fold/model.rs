use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_model::{RegisterConstraintKey, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use semantic_vocabulary::{FuelScheduleIdentity, MachineId};

use crate::rewrites::selected_lowering::literal_fold::identity::encode_terminal_literal_fold_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    RecoveryClassificationIdentity, SpillChoiceIdentity,
};

const LITERAL_FOLD_MAGIC: &[u8; 8] = b"OMGLFD\0\0";
const LITERAL_FOLD_VERSION: u32 = 3;

pub use register_homes::LiteralFoldIdentity;

pub use register_homes::LiteralFoldPolicy;

/// Canonical recipe and output commitment. The transformed selected CFG stays
/// private to the validated carrier and is independently reconstructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralFoldPlan {
    pub source_selected: SelectedInstructionPlanIdentity,
    pub spill_choices: SpillChoiceIdentity,
    pub recovery_classifications: RecoveryClassificationIdentity,
    pub ranges: LiveRangeIdentity,
    pub legality: AllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: AllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: LiteralFoldPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionLiteralFold>,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl LiteralFoldPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_literal_fold_content(self);
        let identity = crate::literal_fold_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(LITERAL_FOLD_MAGIC);
        encoded.extend_from_slice(&LITERAL_FOLD_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, LiteralFoldDecodeError> {
        let mut cursor = LiteralFoldCursor::new(encoded);
        if cursor.take(LITERAL_FOLD_MAGIC.len())? != LITERAL_FOLD_MAGIC {
            return Err(LiteralFoldDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != LITERAL_FOLD_VERSION {
            return Err(LiteralFoldDecodeError::UnsupportedVersion(version));
        }
        let identity = LiteralFoldIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel)
            .ok_or(LiteralFoldDecodeError::InvalidFuelSchedule(raw_fuel))?;
        let policy_bits = cursor.byte()?;
        let policy = LiteralFoldPolicy::from_canonical_bits(policy_bits)
            .ok_or(LiteralFoldDecodeError::UnknownPolicy(policy_bits))?;
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| LiteralFoldDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine)
                .ok_or(LiteralFoldDecodeError::InvalidMachineId(raw_machine))?;
            let action = match cursor.byte()? {
                0 => None,
                1 => Some(LiteralFoldAction {
                    block: SelectedBlockId(u32::from_le_bytes(cursor.array()?)),
                    pressure_point: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                    literal_instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
                    victim: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    consumer_instruction: SelectedInstructionId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    left: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    result: VirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    immediate: u64::from_le_bytes(cursor.array()?),
                    immediate_constraint: decode_constraint_key(&mut cursor)?,
                }),
                tag => return Err(LiteralFoldDecodeError::UnknownOption(tag)),
            };
            functions.push(FunctionLiteralFold { machine, action });
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
            optimization_unit,
            fuel_schedule,
            policy,
            budget,
            usage,
            functions,
            transformed_selected,
        };
        if crate::literal_fold_identity(&plan) != identity {
            return Err(LiteralFoldDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionLiteralFold {
    pub machine: MachineId,
    pub action: Option<LiteralFoldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldAction {
    pub block: SelectedBlockId,
    pub pressure_point: LiveRangePoint,
    pub literal_instruction: SelectedInstructionId,
    pub victim: VirtualRegisterId,
    pub consumer_instruction: SelectedInstructionId,
    pub left: VirtualRegisterId,
    pub result: VirtualRegisterId,
    pub immediate: u64,
    pub immediate_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralFoldValidationReceipt {
    pub(crate) identity: LiteralFoldIdentity,
    pub(crate) source_selected: SelectedInstructionPlanIdentity,
    pub(crate) spill_choices: SpillChoiceIdentity,
    pub(crate) recovery_classifications: RecoveryClassificationIdentity,
    pub(crate) ranges: LiveRangeIdentity,
    pub(crate) legality: AllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: AllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) transformed_selected: SelectedInstructionPlanIdentity,
    pub(crate) policy: LiteralFoldPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
}

impl LiteralFoldValidationReceipt {
    pub const fn identity(self) -> LiteralFoldIdentity {
        self.identity
    }
    pub const fn source_selected(self) -> SelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn spill_choices(self) -> SpillChoiceIdentity {
        self.spill_choices
    }
    pub const fn recovery_classifications(self) -> RecoveryClassificationIdentity {
        self.recovery_classifications
    }
    pub const fn ranges(self) -> LiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> AllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> AllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn transformed_selected(self) -> SelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> LiteralFoldPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn applied_count(self) -> usize {
        self.applied_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedLiteralFold {
    pub(crate) plan: LiteralFoldPlan,
    pub(crate) transformed: std::sync::Arc<SelectedInstructionPlan>,
    pub(crate) receipt: LiteralFoldValidationReceipt,
}

impl ValidatedLiteralFold {
    pub const fn plan(&self) -> &LiteralFoldPlan {
        &self.plan
    }
    /// Share current immutable data; the returned artifact grants no new authority.
    pub fn shared_transformed(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.transformed)
    }

    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }
    pub const fn receipt(&self) -> LiteralFoldValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LiteralFoldError {
    RootMismatch,
    UnsupportedPolicy,
    WorkOverflow,
    BudgetExceeded {
        required: OptimizationWorkUsage,
        budget: OptimizationWorkBudget,
    },
    FunctionMismatch {
        function: usize,
    },
    ClassificationNotAdmitted {
        function: usize,
    },
    UnsupportedVictimRole {
        function: usize,
    },
    UnsupportedImmediate {
        function: usize,
    },
    FutureUseMismatch {
        function: usize,
    },
    LiteralMismatch {
        function: usize,
    },
    ConsumerMismatch {
        function: usize,
    },
    ImmediateConstraintMismatch,
    IdentifierUnderflow {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    UsageMismatch,
    TransformedPlanMismatch,
    TransformedIdentityMismatch,
}

impl std::fmt::Display for LiteralFoldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "terminal literal fold failed: {self:?}")
    }
}

impl std::error::Error for LiteralFoldError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralFoldDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    UnknownConstraintFamily(u8),
    InvalidFuelSchedule(u32),
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

impl std::error::Error for LiteralFoldDecodeError {}

fn decode_constraint_key(
    cursor: &mut LiteralFoldCursor<'_>,
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
        variant: u32::from_le_bytes(cursor.array()?),
    })
}

struct LiteralFoldCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> LiteralFoldCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], LiteralFoldDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(LiteralFoldDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(LiteralFoldDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], LiteralFoldDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| LiteralFoldDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, LiteralFoldDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, LiteralFoldDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| LiteralFoldDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}
