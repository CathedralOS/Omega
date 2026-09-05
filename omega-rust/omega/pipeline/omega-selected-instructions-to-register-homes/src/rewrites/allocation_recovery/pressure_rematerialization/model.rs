use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_register_model::{
    RegisterConstraintKey, RegisterViewId, TargetRegisterEnvironmentIdentity,
};
use omega_selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlan,
    SelectedInstructionPlanIdentity, VirtualRegisterId,
};
use psi_core::{FuelScheduleIdentity, IntegerValue, MachineId, ValueId};

use crate::rewrites::allocation_recovery::pressure_rematerialization::identity::encode_terminal_pressure_rematerialization_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, LiveRangeIdentity, LiveRangePoint,
    RecoveryClassificationIdentity, SpillChoiceIdentity,
};

const MAGIC: &[u8; 8] = b"OMGREM\0\0";
const VERSION: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PressureRematerializationIdentity(pub(crate) [u8; 32]);

impl PressureRematerializationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PressureRematerializationPolicy {
    /// One reconstructed suffix value serves the sole future flexible Use.
    SelectedActiveResidentImmediateU64BeforeSingleFutureFlexibleUseV1,
    /// One reconstructed suffix value is inserted before the first of two or
    /// more canonical future flexible Uses and serves the complete suffix.
    SelectedActiveResidentImmediateU64BeforeFirstOfMultipleFutureFlexibleUsesV1,
}

/// Canonical recipe and output commitment. Decoding never grants validation
/// authority; the selected CFG is reconstructed independently from the roots.
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
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: PressureRematerializationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<FunctionPressureRematerialization>,
    pub transformed_selected: SelectedInstructionPlanIdentity,
}

impl PressureRematerializationPlan {
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_pressure_rematerialization_content(self);
        let identity = crate::pressure_rematerialization_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(MAGIC);
        encoded.extend_from_slice(&VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PressureRematerializationDecodeError> {
        let mut cursor = Cursor::new(encoded);
        if cursor.take(8)? != MAGIC {
            return Err(PressureRematerializationDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != VERSION {
            return Err(PressureRematerializationDecodeError::UnsupportedVersion(
                version,
            ));
        }
        let identity = PressureRematerializationIdentity::from_bytes(cursor.array()?);
        let source_selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let recovery_classifications = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel).ok_or(
            PressureRematerializationDecodeError::InvalidFuelSchedule(raw_fuel),
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
            let raw_machine = u64::from_le_bytes(cursor.array()?);
            let machine = MachineId::new(raw_machine).ok_or(
                PressureRematerializationDecodeError::InvalidMachineId(raw_machine),
            )?;
            let action = match cursor.byte()? {
                0 => None,
                1 => {
                    let block = SelectedBlockId(u32::from_le_bytes(cursor.array()?));
                    let pressure_point = LiveRangePoint(u32::from_le_bytes(cursor.array()?));
                    let victim = VirtualRegisterId(u32::from_le_bytes(cursor.array()?));
                    let current_view = RegisterViewId(u16::from_le_bytes(cursor.array()?));
                    let reclaimed_view = RegisterViewId(u16::from_le_bytes(cursor.array()?));
                    let original_materialize =
                        SelectedInstructionId(u32::from_le_bytes(cursor.array()?));
                    let source_value = ValueId::new(u64::from_le_bytes(cursor.array()?))
                        .ok_or(PressureRematerializationDecodeError::InvalidValueId)?;
                    let value = decode_integer_value(&mut cursor)?;
                    let rewrite_count = cursor.length()?;
                    let mut rewrites = Vec::with_capacity(rewrite_count.min(cursor.remaining()));
                    for _ in 0..rewrite_count {
                        rewrites.push(PressureRematerializationRewrite {
                            point: LiveRangePoint(u32::from_le_bytes(cursor.array()?)),
                            instruction: SelectedInstructionId(u32::from_le_bytes(cursor.array()?)),
                            operand: u16::from_le_bytes(cursor.array()?),
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
            fuel_schedule,
            policy,
            budget,
            usage,
            functions,
            transformed_selected,
        };
        if crate::pressure_rematerialization_identity(&plan) != identity {
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
    /// Canonical exact future flexible Uses rewritten to the one fresh suffix
    /// value. The first row also determines the insertion instruction.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PressureRematerializationValidationReceipt {
    pub(crate) identity: PressureRematerializationIdentity,
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
    pub(crate) policy: PressureRematerializationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) applied_count: usize,
    pub(crate) rewritten_use_count: usize,
}

impl PressureRematerializationValidationReceipt {
    pub const fn identity(self) -> PressureRematerializationIdentity {
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
    pub const fn policy(self) -> PressureRematerializationPolicy {
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
pub struct ValidatedPressureRematerialization {
    pub(crate) plan: PressureRematerializationPlan,
    pub(crate) transformed: std::sync::Arc<SelectedInstructionPlan>,
    pub(crate) receipt: PressureRematerializationValidationReceipt,
}

impl ValidatedPressureRematerialization {
    pub const fn plan(&self) -> &PressureRematerializationPlan {
        &self.plan
    }
    /// Share current immutable data; the returned artifact grants no new authority.
    pub fn shared_transformed(&self) -> std::sync::Arc<SelectedInstructionPlan> {
        std::sync::Arc::clone(&self.transformed)
    }

    pub fn transformed(&self) -> &SelectedInstructionPlan {
        &self.transformed
    }
    pub const fn receipt(&self) -> PressureRematerializationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PressureRematerializationError {
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

impl std::fmt::Display for PressureRematerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "terminal pressure rematerialization failed: {self:?}")
    }
}
impl std::error::Error for PressureRematerializationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PressureRematerializationDecodeError {
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
impl std::fmt::Display for PressureRematerializationDecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid terminal pressure rematerialization: {self:?}")
    }
}
impl std::error::Error for PressureRematerializationDecodeError {}

struct Cursor<'a> {
    encoded: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], PressureRematerializationDecodeError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(PressureRematerializationDecodeError::LengthOverflow)?;
        let bytes = self
            .encoded
            .get(self.offset..end)
            .ok_or(PressureRematerializationDecodeError::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], PressureRematerializationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| PressureRematerializationDecodeError::Truncated)
    }
    fn byte(&mut self) -> Result<u8, PressureRematerializationDecodeError> {
        Ok(self.array::<1>()?[0])
    }
    fn length(&mut self) -> Result<usize, PressureRematerializationDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| PressureRematerializationDecodeError::LengthOverflow)
    }
    fn remaining(&self) -> usize {
        self.encoded.len().saturating_sub(self.offset)
    }
}

fn decode_constraint_key(
    cursor: &mut Cursor<'_>,
) -> Result<RegisterConstraintKey, PressureRematerializationDecodeError> {
    let family = match cursor.byte()? {
        0 => omega_register_model::RegisterConstraintFamily::Call,
        1 => omega_register_model::RegisterConstraintFamily::Return,
        2 => omega_register_model::RegisterConstraintFamily::SystemCall,
        3 => omega_register_model::RegisterConstraintFamily::InlineAssembly,
        4 => omega_register_model::RegisterConstraintFamily::Instruction,
        tag => {
            return Err(PressureRematerializationDecodeError::UnknownConstraintFamily(tag));
        }
    };
    Ok(RegisterConstraintKey {
        family,
        variant: u32::from_le_bytes(cursor.array()?),
    })
}

fn decode_integer_value(
    cursor: &mut Cursor<'_>,
) -> Result<IntegerValue, PressureRematerializationDecodeError> {
    let value = u128::from_le_bytes(cursor.array()?);
    match cursor.byte()? {
        0 => Ok(IntegerValue::Unsigned(value)),
        1 => Ok(IntegerValue::Signed(value as i128)),
        tag => Err(PressureRematerializationDecodeError::UnknownIntegerValue(
            tag,
        )),
    }
}
