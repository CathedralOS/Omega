use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{RegisterConstraintKey, TargetRegisterEnvironmentIdentity};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalSelectedInstructionPlan,
    TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
};
use psi_core::{FuelScheduleIdentity, MachineId};

use crate::literal_fold_identity::encode_terminal_literal_fold_content;
use crate::{
    TerminalAllocationLegalityIdentity, TerminalAllocatorAvailabilityIdentity,
    TerminalLiveRangeIdentity, TerminalLiveRangePoint, TerminalRecoveryClassificationIdentity,
    TerminalSpillChoiceIdentity,
};

const LITERAL_FOLD_MAGIC: &[u8; 8] = b"OMGLFD\0\0";
const LITERAL_FOLD_VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalLiteralFoldIdentity(pub(crate) [u8; 32]);

impl TerminalLiteralFoldIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Narrow proof-preserving physical-form fold. This is not a generic constant
/// fold, instruction scheduler, rematerializer, spill policy, or opt level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalLiteralFoldPolicy {
    SelectedIncomingU12ExactAddImmediateV1,
    SelectedIncomingU12ExactSubtractImmediateV1,
    SelectedIncomingU12ExactAddAndSubtractImmediateV1,
}

/// Canonical recipe and output commitment. The transformed selected CFG stays
/// private to the validated carrier and is independently reconstructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalLiteralFoldPlan {
    pub source_selected: TerminalSelectedInstructionPlanIdentity,
    pub spill_choices: TerminalSpillChoiceIdentity,
    pub recovery_classifications: TerminalRecoveryClassificationIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub legality: TerminalAllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: TerminalAllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: TerminalLiteralFoldPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<TerminalFunctionLiteralFold>,
    pub transformed_selected: TerminalSelectedInstructionPlanIdentity,
}

impl TerminalLiteralFoldPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_literal_fold_content(self);
        let identity = crate::terminal_literal_fold_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(LITERAL_FOLD_MAGIC);
        encoded.extend_from_slice(&LITERAL_FOLD_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TerminalLiteralFoldDecodeError> {
        let mut cursor = LiteralFoldCursor::new(encoded);
        if cursor.take(LITERAL_FOLD_MAGIC.len())? != LITERAL_FOLD_MAGIC {
            return Err(TerminalLiteralFoldDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != LITERAL_FOLD_VERSION {
            return Err(TerminalLiteralFoldDecodeError::UnsupportedVersion(version));
        }
        let identity = TerminalLiteralFoldIdentity::from_bytes(cursor.array()?);
        let source_selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = TerminalSpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications =
            TerminalRecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = TerminalLiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = TerminalAllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability =
            TerminalAllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel).ok_or(
            TerminalLiteralFoldDecodeError::InvalidFuelSchedule(raw_fuel),
        )?;
        let policy = match cursor.byte()? {
            0 => TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddImmediateV1,
            1 => TerminalLiteralFoldPolicy::SelectedIncomingU12ExactSubtractImmediateV1,
            2 => TerminalLiteralFoldPolicy::SelectedIncomingU12ExactAddAndSubtractImmediateV1,
            tag => return Err(TerminalLiteralFoldDecodeError::UnknownPolicy(tag)),
        };
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| TerminalLiteralFoldDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| TerminalLiteralFoldDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine).ok_or(
                TerminalLiteralFoldDecodeError::InvalidMachineId(raw_machine),
            )?;
            let action = match cursor.byte()? {
                0 => None,
                1 => Some(TerminalLiteralFoldAction {
                    block: TerminalSelectedBlockId(u32::from_le_bytes(cursor.array()?)),
                    pressure_point: TerminalLiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                    literal_instruction: TerminalSelectedInstructionId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    victim: TerminalVirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    consumer_instruction: TerminalSelectedInstructionId(u32::from_le_bytes(
                        cursor.array()?,
                    )),
                    left: TerminalVirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    result: TerminalVirtualRegisterId(u32::from_le_bytes(cursor.array()?)),
                    immediate: u64::from_le_bytes(cursor.array()?),
                    immediate_constraint: decode_constraint_key(&mut cursor)?,
                }),
                tag => return Err(TerminalLiteralFoldDecodeError::UnknownOption(tag)),
            };
            functions.push(TerminalFunctionLiteralFold { machine, action });
        }
        let transformed_selected =
            TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(TerminalLiteralFoldDecodeError::TrailingBytes);
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
        if crate::terminal_literal_fold_identity(&plan) != identity {
            return Err(TerminalLiteralFoldDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionLiteralFold {
    pub machine: MachineId,
    pub action: Option<TerminalLiteralFoldAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLiteralFoldAction {
    pub block: TerminalSelectedBlockId,
    pub pressure_point: TerminalLiveRangePoint,
    pub literal_instruction: TerminalSelectedInstructionId,
    pub victim: TerminalVirtualRegisterId,
    pub consumer_instruction: TerminalSelectedInstructionId,
    pub left: TerminalVirtualRegisterId,
    pub result: TerminalVirtualRegisterId,
    pub immediate: u64,
    pub immediate_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalLiteralFoldValidationReceipt {
    pub(crate) identity: TerminalLiteralFoldIdentity,
    pub(crate) source_selected: TerminalSelectedInstructionPlanIdentity,
    pub(crate) spill_choices: TerminalSpillChoiceIdentity,
    pub(crate) recovery_classifications: TerminalRecoveryClassificationIdentity,
    pub(crate) ranges: TerminalLiveRangeIdentity,
    pub(crate) legality: TerminalAllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) allocator_availability: TerminalAllocatorAvailabilityIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) transformed_selected: TerminalSelectedInstructionPlanIdentity,
    pub(crate) policy: TerminalLiteralFoldPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
}

impl TerminalLiteralFoldValidationReceipt {
    pub const fn identity(self) -> TerminalLiteralFoldIdentity {
        self.identity
    }
    pub const fn source_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.source_selected
    }
    pub const fn spill_choices(self) -> TerminalSpillChoiceIdentity {
        self.spill_choices
    }
    pub const fn recovery_classifications(self) -> TerminalRecoveryClassificationIdentity {
        self.recovery_classifications
    }
    pub const fn ranges(self) -> TerminalLiveRangeIdentity {
        self.ranges
    }
    pub const fn legality(self) -> TerminalAllocationLegalityIdentity {
        self.legality
    }
    pub const fn register_environment(self) -> TargetRegisterEnvironmentIdentity {
        self.register_environment
    }
    pub const fn allocator_availability(self) -> TerminalAllocatorAvailabilityIdentity {
        self.allocator_availability
    }
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn transformed_selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.transformed_selected
    }
    pub const fn policy(self) -> TerminalLiteralFoldPolicy {
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
pub struct ValidatedTerminalLiteralFold {
    pub(crate) plan: TerminalLiteralFoldPlan,
    pub(crate) transformed: TerminalSelectedInstructionPlan,
    pub(crate) receipt: TerminalLiteralFoldValidationReceipt,
}

impl ValidatedTerminalLiteralFold {
    pub const fn plan(&self) -> &TerminalLiteralFoldPlan {
        &self.plan
    }
    pub const fn transformed(&self) -> &TerminalSelectedInstructionPlan {
        &self.transformed
    }
    pub const fn receipt(&self) -> TerminalLiteralFoldValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLiteralFoldError {
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

impl std::fmt::Display for TerminalLiteralFoldError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "terminal literal fold failed: {self:?}")
    }
}

impl std::error::Error for TerminalLiteralFoldError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalLiteralFoldDecodeError {
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

impl std::fmt::Display for TerminalLiteralFoldDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid terminal literal fold: {self:?}")
    }
}

impl std::error::Error for TerminalLiteralFoldDecodeError {}

fn decode_constraint_key(
    cursor: &mut LiteralFoldCursor<'_>,
) -> Result<RegisterConstraintKey, TerminalLiteralFoldDecodeError> {
    let family = match cursor.byte()? {
        0 => omega_register_model::RegisterConstraintFamily::Call,
        1 => omega_register_model::RegisterConstraintFamily::Return,
        2 => omega_register_model::RegisterConstraintFamily::SystemCall,
        3 => omega_register_model::RegisterConstraintFamily::InlineAssembly,
        4 => omega_register_model::RegisterConstraintFamily::Instruction,
        tag => return Err(TerminalLiteralFoldDecodeError::UnknownConstraintFamily(tag)),
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
    fn take(&mut self, count: usize) -> Result<&'a [u8], TerminalLiteralFoldDecodeError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(TerminalLiteralFoldDecodeError::Truncated)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(TerminalLiteralFoldDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], TerminalLiteralFoldDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalLiteralFoldDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, TerminalLiteralFoldDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, TerminalLiteralFoldDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| TerminalLiteralFoldDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.offset)
    }
}
