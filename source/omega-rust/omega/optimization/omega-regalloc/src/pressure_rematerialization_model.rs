use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{
    RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlockId, TerminalSelectedInstructionId, TerminalSelectedInstructionPlan,
    TerminalSelectedInstructionPlanIdentity, TerminalVirtualRegisterId,
};
use psi_core::{FuelScheduleIdentity, IntegerValue, MachineId, ValueId};

use crate::pressure_rematerialization_identity::encode_terminal_pressure_rematerialization_content;
use crate::{
    TerminalAllocationLegalityIdentity, TerminalAllocatorAvailabilityIdentity,
    TerminalLiveRangeIdentity, TerminalLiveRangePoint, TerminalRecoveryClassificationIdentity,
    TerminalSpillChoiceIdentity,
};

const MAGIC: &[u8; 8] = b"OMGREM\0\0";
const VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalPressureRematerializationIdentity(pub(crate) [u8; 32]);

impl TerminalPressureRematerializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalPressureRematerializationPolicy {
    /// One reconstructed suffix value serves the sole future flexible Use.
    SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
    /// One reconstructed suffix value is inserted before the first of two or
    /// more canonical future flexible Uses and serves the complete suffix.
    SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
}

/// Canonical recipe and output commitment. Decoding never grants validation
/// authority; the selected CFG is reconstructed independently from the roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPressureRematerializationPlan {
    pub source_selected: TerminalSelectedInstructionPlanIdentity,
    pub spill_choices: TerminalSpillChoiceIdentity,
    pub recovery_classifications: TerminalRecoveryClassificationIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub legality: TerminalAllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub allocator_availability: TerminalAllocatorAvailabilityIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: TerminalPressureRematerializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<TerminalFunctionPressureRematerialization>,
    pub transformed_selected: TerminalSelectedInstructionPlanIdentity,
}

impl TerminalPressureRematerializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_pressure_rematerialization_content(self);
        let identity = crate::terminal_pressure_rematerialization_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TerminalPressureRematerializationDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MAGIC {
            return Err(TerminalPressureRematerializationDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(TerminalPressureRematerializationDecodeError::UnsupportedVersion(version));
        }
        let identity = TerminalPressureRematerializationIdentity::from_bytes(cursor.array()?);
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
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel)
            .ok_or(TerminalPressureRematerializationDecodeError::InvalidFuelSchedule(raw_fuel))?;
        let policy = match cursor.byte()? {
            0 => TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
            1 => TerminalPressureRematerializationPolicy::SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
            tag => return Err(TerminalPressureRematerializationDecodeError::UnknownPolicy(tag)),
        };
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| TerminalPressureRematerializationDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| TerminalPressureRematerializationDecodeError::InvalidUsage)?;
        let count = cursor.length()?;
        let mut functions = Vec::with_capacity(count.min(cursor.remaining()));
        for _ in 0..count {
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine).ok_or(
                TerminalPressureRematerializationDecodeError::InvalidMachineId(raw_machine),
            )?;
            let action = match cursor.byte()? {
                0 => None,
                1 => {
                    let block = TerminalSelectedBlockId(u32::from_le_bytes(cursor.array()?));
                    let pressure_point =
                        TerminalLiveRangePoint(u32::from_le_bytes(cursor.array()?));
                    let victim = TerminalVirtualRegisterId(u32::from_le_bytes(cursor.array()?));
                    let current_view = RegisterViewId(u16::from_le_bytes(cursor.array()?));
                    let reclaimed_view = RegisterViewId(u16::from_le_bytes(cursor.array()?));
                    let original_materialize =
                        TerminalSelectedInstructionId(u32::from_le_bytes(cursor.array()?));
                    let source_value = ValueId::new(u64::from_le_bytes(cursor.array()?))
                        .ok_or(TerminalPressureRematerializationDecodeError::InvalidValueId)?;
                    let value = decode_integer_value(&mut cursor)?;
                    let rewrite_count = cursor.length()?;
                    let mut rewrites = Vec::with_capacity(rewrite_count.min(cursor.remaining()));
                    for _ in 0..rewrite_count {
                        rewrites.push(TerminalPressureRematerializationRewrite {
                            point: TerminalLiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                            instruction: TerminalSelectedInstructionId(u32::from_le_bytes(
                                cursor.array()?,
                            )),
                            operand: u16::from_le_bytes(cursor.array()?),
                        });
                    }
                    Some(TerminalPressureRematerializationAction {
                        block,
                        pressure_point,
                        victim,
                        current_view,
                        reclaimed_view,
                        original_materialize,
                        source_value,
                        value,
                        rewrites,
                        fresh_materialize: TerminalSelectedInstructionId(u32::from_le_bytes(
                            cursor.array()?,
                        )),
                        result_virtual_register: TerminalVirtualRegisterId(u32::from_le_bytes(
                            cursor.array()?,
                        )),
                        materialize_constraint: decode_constraint_key(&mut cursor)?,
                    })
                }
                tag => {
                    return Err(TerminalPressureRematerializationDecodeError::UnknownOption(
                        tag,
                    ));
                }
            };
            functions.push(TerminalFunctionPressureRematerialization { machine, action });
        }
        let transformed_selected =
            TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        if cursor.remaining() != 0 {
            return Err(TerminalPressureRematerializationDecodeError::TrailingBytes);
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
        if crate::terminal_pressure_rematerialization_identity(&plan) != identity {
            return Err(TerminalPressureRematerializationDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionPressureRematerialization {
    pub machine: MachineId,
    pub action: Option<TerminalPressureRematerializationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPressureRematerializationAction {
    pub block: TerminalSelectedBlockId,
    pub pressure_point: TerminalLiveRangePoint,
    pub victim: TerminalVirtualRegisterId,
    pub current_view: RegisterViewId,
    pub reclaimed_view: RegisterViewId,
    pub original_materialize: TerminalSelectedInstructionId,
    pub source_value: ValueId,
    pub value: IntegerValue,
    /// Canonical exact future flexible Uses rewritten to the one fresh suffix
    /// value. The first row also determines the insertion instruction.
    pub rewrites: Vec<TerminalPressureRematerializationRewrite>,
    pub fresh_materialize: TerminalSelectedInstructionId,
    pub result_virtual_register: TerminalVirtualRegisterId,
    pub materialize_constraint: RegisterConstraintKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalPressureRematerializationRewrite {
    pub point: TerminalLiveRangePoint,
    pub instruction: TerminalSelectedInstructionId,
    pub operand: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalPressureRematerializationValidationReceipt {
    pub(crate) identity: TerminalPressureRematerializationIdentity,
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
    pub(crate) policy: TerminalPressureRematerializationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
    pub(crate) rewritten_use_count: usize,
}

impl TerminalPressureRematerializationValidationReceipt {
    pub const fn identity(self) -> TerminalPressureRematerializationIdentity {
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
    pub const fn policy(self) -> TerminalPressureRematerializationPolicy {
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
    pub const fn rewritten_use_count(self) -> usize {
        self.rewritten_use_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalPressureRematerialization {
    pub(crate) plan: TerminalPressureRematerializationPlan,
    pub(crate) transformed: TerminalSelectedInstructionPlan,
    pub(crate) receipt: TerminalPressureRematerializationValidationReceipt,
}

impl ValidatedTerminalPressureRematerialization {
    pub const fn plan(&self) -> &TerminalPressureRematerializationPlan {
        &self.plan
    }
    pub const fn transformed(&self) -> &TerminalSelectedInstructionPlan {
        &self.transformed
    }
    pub const fn receipt(&self) -> TerminalPressureRematerializationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalPressureRematerializationError {
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
    FutureUseMismatch {
        function: usize,
    },
    MaterializeMismatch {
        function: usize,
    },
    MaterializeConstraintMismatch,
    IdentifierOverflow {
        function: usize,
    },
    DecisionMismatch {
        function: usize,
    },
    NoAction,
    UsageMismatch,
    TransformedIdentityMismatch,
}

impl std::fmt::Display for TerminalPressureRematerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "terminal pressure rematerialization failed: {self:?}")
    }
}
impl std::error::Error for TerminalPressureRematerializationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalPressureRematerializationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    UnknownConstraintFamily(u8),
    UnknownIntegerValue(u8),
    InvalidFuelSchedule(u32),
    InvalidMachineId(u64),
    InvalidValueId,
    InvalidBudget,
    InvalidUsage,
    LengthOverflow,
    TrailingBytes,
    IdentityMismatch,
}
impl std::fmt::Display for TerminalPressureRematerializationDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid terminal pressure rematerialization: {self:?}")
    }
}
impl std::error::Error for TerminalPressureRematerializationDecodeError {}

struct Cursor<'a> {
    encoded: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }
    fn take(
        &mut self,
        len: usize,
    ) -> Result<&'a [u8], TerminalPressureRematerializationDecodeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(TerminalPressureRematerializationDecodeError::LengthOverflow)?;
        let bytes = self
            .encoded
            .get(self.offset..end)
            .ok_or(TerminalPressureRematerializationDecodeError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], TerminalPressureRematerializationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalPressureRematerializationDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, TerminalPressureRematerializationDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, TerminalPressureRematerializationDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| TerminalPressureRematerializationDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.encoded.len().saturating_sub(self.offset)
    }
}

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, TerminalPressureRematerializationDecodeError> {
    let family = match cursor.byte()? {
        0 => omega_register_model::RegisterConstraintFamily::Call,
        1 => omega_register_model::RegisterConstraintFamily::Return,
        2 => omega_register_model::RegisterConstraintFamily::SystemCall,
        3 => omega_register_model::RegisterConstraintFamily::InlineAssembly,
        4 => omega_register_model::RegisterConstraintFamily::Instruction,
        tag => {
            return Err(TerminalPressureRematerializationDecodeError::UnknownConstraintFamily(tag));
        }
    };
    Ok(RegisterConstraintKey {
        family,
        variant: u32::from_le_bytes(cursor.array()?),
    })
}

fn decode_integer_value(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, TerminalPressureRematerializationDecodeError> {
    let value = u128::from_le_bytes(cursor.array()?);
    match cursor.byte()? {
        0 => Ok(IntegerValue::Unsigned(value)),
        1 => Ok(IntegerValue::Signed(value as i128)),
        tag => Err(TerminalPressureRematerializationDecodeError::UnknownIntegerValue(tag)),
    }
}
