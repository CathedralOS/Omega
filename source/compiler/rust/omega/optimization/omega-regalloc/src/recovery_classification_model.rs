use omega_optimization_core::{
    OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
};
use omega_optimization_unit::ValueDefinitionSite;
use omega_register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use omega_terminal_selected_instructions::{
    TerminalSelectedBlockId, TerminalSelectedInstructionId,
    TerminalSelectedInstructionPlanIdentity, TerminalSelectedInstructionProvenance,
    TerminalVirtualRegisterId, TerminalVirtualRegisterOrigin,
};
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, ScalarType, ValueId,
};

use crate::recovery_classification_identity::encode_terminal_recovery_classification_content;
use crate::{
    TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity, TerminalLiveRangePoint,
    TerminalSpillChoiceIdentity,
};

const RECOVERY_CLASSIFICATION_MAGIC: &[u8; 8] = b"OMGRCV\0\0";
const RECOVERY_CLASSIFICATION_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalRecoveryClassificationIdentity(pub(crate) [u8; 32]);

impl TerminalRecoveryClassificationIdentity {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Named, deliberately bounded policy for classifying the already selected
/// pressure victim. This is not a spill, rematerialization, or cost policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalRecoveryClassificationPolicy {
    SelectedVictimImmediateU64EligibilityV1,
}

/// Semantic eligibility evidence for the first selected local pressure victim.
///
/// This artifact does not choose or insert recovery code. In particular, it
/// grants no storage, stack-slot, frame-layout, instruction-emission, logical-
/// fuel, or publication authority. A future materialization stage must join
/// target frame policy and independently validate any concrete transformation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRecoveryClassificationPlan {
    pub selected: TerminalSelectedInstructionPlanIdentity,
    pub spill_choices: TerminalSpillChoiceIdentity,
    pub ranges: TerminalLiveRangeIdentity,
    pub legality: TerminalAllocationLegalityIdentity,
    pub register_environment: TargetRegisterEnvironmentIdentity,
    pub optimization_unit: OptimizationUnitIdentity,
    pub fuel_schedule: FuelScheduleIdentity,
    pub policy: TerminalRecoveryClassificationPolicy,
    pub budget: OptimizationWorkBudget,
    pub usage: OptimizationWorkUsage,
    pub functions: Vec<TerminalFunctionRecoveryClassification>,
}

impl TerminalRecoveryClassificationPlan {
    /// Canonical transport only. Decoding returns an unchecked plain plan; only
    /// independent replay against all retained roots may grant classification
    /// authority.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_recovery_classification_content(self);
        let identity = crate::terminal_recovery_classification_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(RECOVERY_CLASSIFICATION_MAGIC);
        encoded.extend_from_slice(&RECOVERY_CLASSIFICATION_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TerminalRecoveryClassificationDecodeError> {
        let mut cursor = RecoveryClassificationCursor::new(encoded);
        if cursor.take(RECOVERY_CLASSIFICATION_MAGIC.len())? != RECOVERY_CLASSIFICATION_MAGIC {
            return Err(TerminalRecoveryClassificationDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != RECOVERY_CLASSIFICATION_VERSION {
            return Err(TerminalRecoveryClassificationDecodeError::UnsupportedVersion(version));
        }
        let identity = TerminalRecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let selected = TerminalSelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = TerminalSpillChoiceIdentity::from_bytes(cursor.array()?);
        let ranges = TerminalLiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = TerminalAllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel_schedule = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel_schedule).ok_or(
            TerminalRecoveryClassificationDecodeError::InvalidFuelSchedule(raw_fuel_schedule),
        )?;
        let policy = match cursor.byte()? {
            0 => TerminalRecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            tag => {
                return Err(TerminalRecoveryClassificationDecodeError::UnknownPolicy(
                    tag,
                ));
            }
        };
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| TerminalRecoveryClassificationDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| TerminalRecoveryClassificationDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let machine = cursor.machine()?;
            let classification = match cursor.byte()? {
                0 => None,
                1 => Some(cursor.classification_row()?),
                tag => {
                    return Err(TerminalRecoveryClassificationDecodeError::UnknownOption(
                        tag,
                    ));
                }
            };
            functions.push(TerminalFunctionRecoveryClassification {
                machine,
                classification,
            });
        }
        if cursor.remaining() != 0 {
            return Err(TerminalRecoveryClassificationDecodeError::TrailingBytes);
        }
        let plan = Self {
            selected,
            spill_choices,
            ranges,
            legality,
            register_environment,
            optimization_unit,
            fuel_schedule,
            policy,
            budget,
            usage,
            functions,
        };
        if crate::terminal_recovery_classification_identity(&plan) != identity {
            return Err(TerminalRecoveryClassificationDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFunctionRecoveryClassification {
    pub machine: MachineId,
    /// Exactly one row when the rooted spill-choice function selected a victim;
    /// otherwise `None`.
    pub classification: Option<TerminalPressureRecoveryClassification>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPressureRecoveryClassification {
    pub block: TerminalSelectedBlockId,
    pub point: TerminalLiveRangePoint,
    pub victim: TerminalVirtualRegisterId,
    pub role: TerminalRecoveryVictimRole,
    pub scalar_type: ScalarType,
    pub class: RegisterClassId,
    pub origin: TerminalVirtualRegisterOrigin,
    pub definition_site: ValueDefinitionSite,
    pub classification: TerminalRecoveryClassification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRecoveryVictimRole {
    Incoming,
    ActiveResident {
        current_view: RegisterViewId,
        reclaimed_view: RegisterViewId,
    },
}

/// A semantic classification only. Even an admitted candidate is not
/// permission to recompute the value or alter its logical charge placement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRecoveryClassification {
    ImmediateU64RematerializationCandidate {
        defining_instruction: TerminalSelectedInstructionId,
        source_value: ValueId,
        value: IntegerValue,
        provenance: TerminalSelectedInstructionProvenance,
        future_uses: Vec<TerminalRecoveryFutureUse>,
    },
    NoAdmittedRecovery {
        reason: TerminalNoAdmittedRecoveryReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TerminalRecoveryFutureUse {
    pub block: TerminalSelectedBlockId,
    pub point: TerminalLiveRangePoint,
    pub instruction: TerminalSelectedInstructionId,
    pub operand: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalNoAdmittedRecoveryReason {
    UnsupportedScalarType,
    EntryParameter,
    UnsupportedRangeShape,
    FutureFixedUse,
    NonMaterializeDefinition,
    ProofBearingDefinition,
    NoFutureUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalRecoveryClassificationValidationReceipt {
    pub(crate) identity: TerminalRecoveryClassificationIdentity,
    pub(crate) selected: TerminalSelectedInstructionPlanIdentity,
    pub(crate) spill_choices: TerminalSpillChoiceIdentity,
    pub(crate) ranges: TerminalLiveRangeIdentity,
    pub(crate) legality: TerminalAllocationLegalityIdentity,
    pub(crate) register_environment: TargetRegisterEnvironmentIdentity,
    pub(crate) optimization_unit: OptimizationUnitIdentity,
    pub(crate) fuel_schedule: FuelScheduleIdentity,
    pub(crate) policy: TerminalRecoveryClassificationPolicy,
    pub(crate) usage: OptimizationWorkUsage,
    pub(crate) function_count: usize,
    pub(crate) classification_count: usize,
    pub(crate) immediate_candidate_count: usize,
}

impl TerminalRecoveryClassificationValidationReceipt {
    pub const fn identity(self) -> TerminalRecoveryClassificationIdentity {
        self.identity
    }
    pub const fn selected(self) -> TerminalSelectedInstructionPlanIdentity {
        self.selected
    }
    pub const fn spill_choices(self) -> TerminalSpillChoiceIdentity {
        self.spill_choices
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
    pub const fn optimization_unit(self) -> OptimizationUnitIdentity {
        self.optimization_unit
    }
    pub const fn fuel_schedule(self) -> FuelScheduleIdentity {
        self.fuel_schedule
    }
    pub const fn policy(self) -> TerminalRecoveryClassificationPolicy {
        self.policy
    }
    pub const fn usage(self) -> OptimizationWorkUsage {
        self.usage
    }
    pub const fn function_count(self) -> usize {
        self.function_count
    }
    pub const fn classification_count(self) -> usize {
        self.classification_count
    }
    pub const fn immediate_candidate_count(self) -> usize {
        self.immediate_candidate_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTerminalRecoveryClassifications {
    pub(crate) plan: TerminalRecoveryClassificationPlan,
    pub(crate) receipt: TerminalRecoveryClassificationValidationReceipt,
}

impl ValidatedTerminalRecoveryClassifications {
    pub const fn plan(&self) -> &TerminalRecoveryClassificationPlan {
        &self.plan
    }
    pub const fn receipt(&self) -> TerminalRecoveryClassificationValidationReceipt {
        self.receipt
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalRecoveryClassificationError {
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
    ChoiceMismatch {
        function: usize,
    },
    VictimMismatch {
        function: usize,
        register: u32,
    },
    ClassificationMismatch {
        function: usize,
    },
    UsageMismatch,
}

impl std::fmt::Display for TerminalRecoveryClassificationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Terminal recovery classification failed: {self:?}"
        )
    }
}

impl std::error::Error for TerminalRecoveryClassificationError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRecoveryClassificationDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownPolicy(u8),
    UnknownOption(u8),
    UnknownVictimRole(u8),
    UnknownClassification(u8),
    UnknownReason(u8),
    UnknownScalarType(u8),
    UnknownIntegerValue(u8),
    UnknownOrigin(u8),
    UnknownDefinitionSite(u8),
    UnknownProvenance(u8),
    InvalidBudget,
    InvalidUsage,
    InvalidFuelSchedule(u32),
    InvalidMachineId(u64),
    InvalidBlockId(u64),
    InvalidOperationId(u64),
    InvalidValueId(u64),
    InvalidEdgeId(u64),
    InvalidObligationId(u64),
    InvalidIntegerType,
    LengthOverflow,
    IdentityMismatch,
    TrailingBytes,
}

impl std::fmt::Display for TerminalRecoveryClassificationDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "invalid Terminal recovery-classification encoding: {self:?}"
        )
    }
}

impl std::error::Error for TerminalRecoveryClassificationDecodeError {}

struct RecoveryClassificationCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> RecoveryClassificationCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'encoded [u8], TerminalRecoveryClassificationDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(TerminalRecoveryClassificationDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(TerminalRecoveryClassificationDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], TerminalRecoveryClassificationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| TerminalRecoveryClassificationDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, TerminalRecoveryClassificationDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, TerminalRecoveryClassificationDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| TerminalRecoveryClassificationDecodeError::LengthOverflow)
    }

    fn machine(&mut self) -> Result<MachineId, TerminalRecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        MachineId::new(raw).ok_or(TerminalRecoveryClassificationDecodeError::InvalidMachineId(
            raw,
        ))
    }

    fn block_id(&mut self) -> Result<BlockId, TerminalRecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        BlockId::new(raw).ok_or(TerminalRecoveryClassificationDecodeError::InvalidBlockId(
            raw,
        ))
    }

    fn operation_id(&mut self) -> Result<OperationId, TerminalRecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        OperationId::new(raw)
            .ok_or(TerminalRecoveryClassificationDecodeError::InvalidOperationId(raw))
    }

    fn value_id(&mut self) -> Result<ValueId, TerminalRecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        ValueId::new(raw).ok_or(TerminalRecoveryClassificationDecodeError::InvalidValueId(
            raw,
        ))
    }

    fn edge_id(&mut self) -> Result<EdgeId, TerminalRecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        EdgeId::new(raw).ok_or(TerminalRecoveryClassificationDecodeError::InvalidEdgeId(
            raw,
        ))
    }

    fn obligation_id(&mut self) -> Result<ObligationId, TerminalRecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        ObligationId::new(raw)
            .ok_or(TerminalRecoveryClassificationDecodeError::InvalidObligationId(raw))
    }

    fn classification_row(
        &mut self,
    ) -> Result<TerminalPressureRecoveryClassification, TerminalRecoveryClassificationDecodeError>
    {
        let block = TerminalSelectedBlockId(u32::from_le_bytes(self.array()?));
        let point = TerminalLiveRangePoint(u32::from_le_bytes(self.array()?));
        let victim = TerminalVirtualRegisterId(u32::from_le_bytes(self.array()?));
        let role = match self.byte()? {
            0 => TerminalRecoveryVictimRole::Incoming,
            1 => TerminalRecoveryVictimRole::ActiveResident {
                current_view: RegisterViewId(u16::from_le_bytes(self.array()?)),
                reclaimed_view: RegisterViewId(u16::from_le_bytes(self.array()?)),
            },
            tag => {
                return Err(TerminalRecoveryClassificationDecodeError::UnknownVictimRole(tag));
            }
        };
        let scalar_type = self.scalar_type()?;
        let class = RegisterClassId(u16::from_le_bytes(self.array()?));
        let origin = self.origin()?;
        let definition_site = self.definition_site()?;
        let classification = match self.byte()? {
            0 => {
                let defining_instruction =
                    TerminalSelectedInstructionId(u32::from_le_bytes(self.array()?));
                let source_value = self.value_id()?;
                let value = self.integer_value()?;
                let provenance = self.provenance()?;
                let use_count = self.length()?;
                let mut future_uses = Vec::with_capacity(use_count.min(self.remaining()));
                for _ in 0..use_count {
                    future_uses.push(TerminalRecoveryFutureUse {
                        block: TerminalSelectedBlockId(u32::from_le_bytes(self.array()?)),
                        point: TerminalLiveRangePoint(u32::from_le_bytes(self.array()?)),
                        instruction: TerminalSelectedInstructionId(u32::from_le_bytes(
                            self.array()?,
                        )),
                        operand: u16::from_le_bytes(self.array()?),
                    });
                }
                TerminalRecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction,
                    source_value,
                    value,
                    provenance,
                    future_uses,
                }
            }
            1 => TerminalRecoveryClassification::NoAdmittedRecovery {
                reason: match self.byte()? {
                    0 => TerminalNoAdmittedRecoveryReason::UnsupportedScalarType,
                    1 => TerminalNoAdmittedRecoveryReason::EntryParameter,
                    2 => TerminalNoAdmittedRecoveryReason::UnsupportedRangeShape,
                    3 => TerminalNoAdmittedRecoveryReason::FutureFixedUse,
                    4 => TerminalNoAdmittedRecoveryReason::NonMaterializeDefinition,
                    5 => TerminalNoAdmittedRecoveryReason::ProofBearingDefinition,
                    6 => TerminalNoAdmittedRecoveryReason::NoFutureUse,
                    tag => {
                        return Err(TerminalRecoveryClassificationDecodeError::UnknownReason(
                            tag,
                        ));
                    }
                },
            },
            tag => {
                return Err(TerminalRecoveryClassificationDecodeError::UnknownClassification(tag));
            }
        };
        Ok(TerminalPressureRecoveryClassification {
            block,
            point,
            victim,
            role,
            scalar_type,
            class,
            origin,
            definition_site,
            classification,
        })
    }

    fn scalar_type(&mut self) -> Result<ScalarType, TerminalRecoveryClassificationDecodeError> {
        let scalar = match self.byte()? {
            0 => ScalarType::Boolean,
            1 => ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, u16::from_le_bytes(self.array()?))
                    .map_err(|_| TerminalRecoveryClassificationDecodeError::InvalidIntegerType)?,
            ),
            2 => ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, u16::from_le_bytes(self.array()?))
                    .map_err(|_| TerminalRecoveryClassificationDecodeError::InvalidIntegerType)?,
            ),
            3 => ScalarType::Integer(
                IntegerType::address(u16::from_le_bytes(self.array()?))
                    .map_err(|_| TerminalRecoveryClassificationDecodeError::InvalidIntegerType)?,
            ),
            tag => return Err(TerminalRecoveryClassificationDecodeError::UnknownScalarType(tag)),
        };
        Ok(scalar)
    }

    fn integer_value(&mut self) -> Result<IntegerValue, TerminalRecoveryClassificationDecodeError> {
        match self.byte()? {
            0 => Ok(IntegerValue::Signed(i128::from_le_bytes(self.array()?))),
            1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(self.array()?))),
            tag => Err(TerminalRecoveryClassificationDecodeError::UnknownIntegerValue(tag)),
        }
    }

    fn origin(
        &mut self,
    ) -> Result<TerminalVirtualRegisterOrigin, TerminalRecoveryClassificationDecodeError> {
        match self.byte()? {
            0 => Ok(TerminalVirtualRegisterOrigin::EntryParameter {
                source_value: self.value_id()?,
                parameter_index: self.length()?,
            }),
            1 => Ok(TerminalVirtualRegisterOrigin::InstructionResult {
                instruction: TerminalSelectedInstructionId(u32::from_le_bytes(self.array()?)),
                source_value: self.value_id()?,
            }),
            tag => Err(TerminalRecoveryClassificationDecodeError::UnknownOrigin(
                tag,
            )),
        }
    }

    fn definition_site(
        &mut self,
    ) -> Result<ValueDefinitionSite, TerminalRecoveryClassificationDecodeError> {
        match self.byte()? {
            0 => Ok(ValueDefinitionSite::FunctionParameter(u32::from_le_bytes(
                self.array()?,
            ))),
            1 => Ok(ValueDefinitionSite::BlockParameter {
                block: self.block_id()?,
                position: u32::from_le_bytes(self.array()?),
            }),
            2 => Ok(ValueDefinitionSite::Node {
                block: self.block_id()?,
                node: u32::from_le_bytes(self.array()?),
            }),
            tag => Err(TerminalRecoveryClassificationDecodeError::UnknownDefinitionSite(tag)),
        }
    }

    fn provenance(
        &mut self,
    ) -> Result<TerminalSelectedInstructionProvenance, TerminalRecoveryClassificationDecodeError>
    {
        let operation_count = self.length()?;
        let mut operations = Vec::with_capacity(operation_count.min(self.remaining()));
        for _ in 0..operation_count {
            operations.push(self.operation_id()?);
        }
        let value_count = self.length()?;
        let mut values = Vec::with_capacity(value_count.min(self.remaining()));
        for _ in 0..value_count {
            values.push(self.value_id()?);
        }
        let edge_count = self.length()?;
        let mut edges = Vec::with_capacity(edge_count.min(self.remaining()));
        for _ in 0..edge_count {
            edges.push(self.edge_id()?);
        }
        let obligation_count = self.length()?;
        let mut obligations = Vec::with_capacity(obligation_count.min(self.remaining()));
        for _ in 0..obligation_count {
            obligations.push(self.obligation_id()?);
        }
        let fuel_count = self.length()?;
        let mut fuel = Vec::with_capacity(fuel_count.min(self.remaining()));
        for _ in 0..fuel_count {
            let site = match self.byte()? {
                0 => omega_optimization_unit::PsiProvenance::Operation(self.operation_id()?),
                1 => omega_optimization_unit::PsiProvenance::Edge(self.edge_id()?),
                tag => {
                    return Err(TerminalRecoveryClassificationDecodeError::UnknownProvenance(tag));
                }
            };
            fuel.push(omega_optimization_unit::FuelSettlement {
                site,
                units: u64::from_le_bytes(self.array()?),
            });
        }
        Ok(TerminalSelectedInstructionProvenance {
            operations,
            values,
            edges,
            obligations,
            fuel,
        })
    }

    fn remaining(&self) -> usize {
        self.encoded.len() - self.offset
    }
}
