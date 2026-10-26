//! Optimizer module role: executable entrance. Exact active-resident rematerialization and replay entrance.
use crate::ValidatedAllocationLegality;
use crate::ValidatedAllocatorAvailability;
use crate::ValidatedLiveRanges;
use crate::ValidatedRecoveryClassifications;
use crate::ValidatedSelectedAnalysis;
use crate::ValidatedSpillChoices;
use crate::cursor::{Cursor, TakeError, Truncated};
use register_model::{
    TargetRegisterEnvironmentConstraintKeys, TargetRegisterEnvironmentIdentity,
    ValidatedRegisterConstraintCatalog,
};

pub(crate) mod compute;
pub(crate) mod identity;
mod receipt;
use identity::encode_terminal_pressure_rematerialization_content;
pub(crate) use identity::pressure_rematerialization_identity;
use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use register_homes::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, RecoveryClassificationIdentity,
    SpillChoiceIdentity,
};
use register_model::{RegisterConstraintKey, RegisterViewId};
use selected_instructions::{
    LiveRangeIdentity, LiveRangePoint, PressureRematerializationIdentity, SelectedBlockId,
    SelectedInstructionId, SelectedInstructionPlan, SelectedInstructionPlanIdentity,
    VirtualRegisterId,
};
use semantic_vocabulary::{FuelScheduleIdentity, IntegerValue, MachineId, ValueId};

/// Insert one value-lineage-only, zero-fuel rematerialization immediately
/// before the supported future-use boundary while retaining the semantic
/// source materialization and its charge.
pub fn rematerialize_selected_active_resident<S: ValidatedSelectedAnalysis>(
    selected: &S,
    ranges: &ValidatedLiveRanges,
    legality: &ValidatedAllocationLegality,
    spill_choices: &ValidatedSpillChoices,
    recovery: &ValidatedRecoveryClassifications,
    availability: &ValidatedAllocatorAvailability,
    register_environment: TargetRegisterEnvironmentIdentity,
    constraints: &ValidatedRegisterConstraintCatalog,
    selected_keys: &TargetRegisterEnvironmentConstraintKeys,
    policy: PressureRematerializationPolicy,
    budget: optimization_core::OptimizationWorkBudget,
) -> Result<ValidatedPressureRematerialization, PressureRematerializationError> {
    // The plan and its transformed program were derived together in this
    // compile; nothing re-derives them before admission.
    let compute::ComputedPressureRematerialization {
        plan,
        transformed,
        applied,
        rewritten_uses,
    } = compute::compute_terminal_pressure_rematerialization(
        selected,
        ranges,
        legality,
        spill_choices,
        recovery,
        availability,
        register_environment,
        constraints,
        selected_keys,
        policy,
        budget,
    )?;
    let receipt = receipt::bind(
        &plan,
        plan.transformed_selected,
        transformed.functions.len(),
        applied,
        rewritten_uses,
    );
    Ok(ValidatedPressureRematerialization {
        plan,
        transformed: transformed.into(),
        receipt,
    })
}

const MAGIC: &[u8; 8] = b"OMGREM\0\0";
const VERSION: u32 = 2;

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
        let identity = pressure_rematerialization_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        crate::codec_bytes::encode_header(&mut encoded, MAGIC, VERSION);
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, PressureRematerializationDecodeError> {
        let mut cursor = Cursor::new(encoded);
        crate::codec_bytes::decode_header(
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
        let raw_fuel = cursor.u32()?;
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
            fuel_schedule,
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
    receipt_accessor_rows!(PressureRematerializationValidationReceipt {
        identity: PressureRematerializationIdentity,
        spill_choices: SpillChoiceIdentity,
        recovery_classifications: RecoveryClassificationIdentity,
        policy: PressureRematerializationPolicy,
        usage: OptimizationWorkUsage,
        function_count: usize,
        applied_count: usize,
        rewritten_use_count: usize,
        source_selected: SelectedInstructionPlanIdentity,
        transformed_selected: SelectedInstructionPlanIdentity,
    });
    shared_analysis_identity_accessors!(pub);
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
    receipt_accessor_rows!(ref ValidatedPressureRematerialization {
        receipt: PressureRematerializationValidationReceipt,
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PressureRematerializationError {
    RootMismatch,
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
}

failed_display!(
    PressureRematerializationError,
    "terminal pressure rematerialization failed"
);
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
