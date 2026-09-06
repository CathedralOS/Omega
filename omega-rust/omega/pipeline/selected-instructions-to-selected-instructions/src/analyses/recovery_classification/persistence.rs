//! Versioned recovery-classification transport and canonical decoding.

use optimization_core::{OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage};
use optimization_unit::ValueDefinitionSite;
use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
use selected_instructions::{
    SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
    SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
    ObligationId, OperationId, ScalarType, ValueId,
};

use crate::analyses::recovery_classification::identity::encode_terminal_recovery_classification_content;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionRecoveryClassification,
    LiveRangeIdentity, LiveRangePoint, NoAdmittedRecoveryReason, PressureRecoveryClassification,
    RecoveryClassification, RecoveryClassificationDecodeError, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, SpillChoiceIdentity,
};

const RECOVERY_CLASSIFICATION_MAGIC: &[u8; 8] = b"OMGRCV\0\0";
const RECOVERY_CLASSIFICATION_VERSION: u32 = 4;

impl RecoveryClassificationPlan {
    /// Canonical transport only. Decoding returns an unchecked plain plan; only
    /// independent replay against all retained roots may grant classification
    /// authority.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_recovery_classification_content(self);
        let identity = crate::recovery_classification_identity(self);
        let mut encoded = Vec::with_capacity(44 + content.len());
        encoded.extend_from_slice(RECOVERY_CLASSIFICATION_MAGIC);
        encoded.extend_from_slice(&RECOVERY_CLASSIFICATION_VERSION.to_le_bytes());
        encoded.extend_from_slice(&identity.bytes());
        encoded.extend_from_slice(&content);
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, RecoveryClassificationDecodeError> {
        let mut cursor = RecoveryClassificationCursor::new(encoded);
        if cursor.take(RECOVERY_CLASSIFICATION_MAGIC.len())? != RECOVERY_CLASSIFICATION_MAGIC {
            return Err(RecoveryClassificationDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(cursor.array()?);
        if version != RECOVERY_CLASSIFICATION_VERSION {
            return Err(RecoveryClassificationDecodeError::UnsupportedVersion(
                version,
            ));
        }
        let identity = RecoveryClassificationIdentity::from_bytes(cursor.array()?);
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel_schedule = u32::from_le_bytes(cursor.array()?);
        let fuel_schedule = FuelScheduleIdentity::new(raw_fuel_schedule).ok_or(
            RecoveryClassificationDecodeError::InvalidFuelSchedule(raw_fuel_schedule),
        )?;
        let policy = match cursor.byte()? {
            0 => RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            tag => {
                return Err(RecoveryClassificationDecodeError::UnknownPolicy(tag));
            }
        };
        let budget = OptimizationWorkBudget::decode(cursor.take(40)?)
            .map_err(|_| RecoveryClassificationDecodeError::InvalidBudget)?;
        let usage = OptimizationWorkUsage::decode(cursor.take(40)?)
            .map_err(|_| RecoveryClassificationDecodeError::InvalidUsage)?;
        let function_count = cursor.length()?;
        let mut functions = Vec::with_capacity(function_count.min(cursor.remaining()));
        for _ in 0..function_count {
            let machine = cursor.machine()?;
            let classification = match cursor.byte()? {
                0 => None,
                1 => Some(cursor.classification_row()?),
                tag => {
                    return Err(RecoveryClassificationDecodeError::UnknownOption(tag));
                }
            };
            functions.push(FunctionRecoveryClassification {
                machine,
                classification,
            });
        }
        if cursor.remaining() != 0 {
            return Err(RecoveryClassificationDecodeError::TrailingBytes);
        }
        let plan = Self {
            selected,
            spill_choices,
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
        };
        if crate::recovery_classification_identity(&plan) != identity {
            return Err(RecoveryClassificationDecodeError::IdentityMismatch);
        }
        Ok(plan)
    }
}

struct RecoveryClassificationCursor<'encoded> {
    encoded: &'encoded [u8],
    offset: usize,
}

impl<'encoded> RecoveryClassificationCursor<'encoded> {
    const fn new(encoded: &'encoded [u8]) -> Self {
        Self { encoded, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'encoded [u8], RecoveryClassificationDecodeError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(RecoveryClassificationDecodeError::Truncated)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(RecoveryClassificationDecodeError::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], RecoveryClassificationDecodeError> {
        self.take(N)?
            .try_into()
            .map_err(|_| RecoveryClassificationDecodeError::Truncated)
    }

    fn byte(&mut self) -> Result<u8, RecoveryClassificationDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn length(&mut self) -> Result<usize, RecoveryClassificationDecodeError> {
        usize::try_from(u64::from_le_bytes(self.array()?))
            .map_err(|_| RecoveryClassificationDecodeError::LengthOverflow)
    }

    fn machine(&mut self) -> Result<MachineId, RecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        MachineId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidMachineId(raw))
    }

    fn block_id(&mut self) -> Result<BlockId, RecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        BlockId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidBlockId(raw))
    }

    fn operation_id(&mut self) -> Result<OperationId, RecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        OperationId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidOperationId(raw))
    }

    fn value_id(&mut self) -> Result<ValueId, RecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        ValueId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidValueId(raw))
    }

    fn edge_id(&mut self) -> Result<EdgeId, RecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        EdgeId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidEdgeId(raw))
    }

    fn obligation_id(&mut self) -> Result<ObligationId, RecoveryClassificationDecodeError> {
        let raw = u64::from_le_bytes(self.array()?);
        ObligationId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidObligationId(raw))
    }

    fn classification_row(
        &mut self,
    ) -> Result<PressureRecoveryClassification, RecoveryClassificationDecodeError> {
        let block = SelectedBlockId(u32::from_le_bytes(self.array()?));
        let point = LiveRangePoint(u32::from_le_bytes(self.array()?));
        let victim = VirtualRegisterId(u32::from_le_bytes(self.array()?));
        let role = match self.byte()? {
            0 => RecoveryVictimRole::Incoming,
            1 => RecoveryVictimRole::ActiveResident {
                current_view: RegisterViewId(u16::from_le_bytes(self.array()?)),
                reclaimed_view: RegisterViewId(u16::from_le_bytes(self.array()?)),
            },
            tag => {
                return Err(RecoveryClassificationDecodeError::UnknownVictimRole(tag));
            }
        };
        let scalar_type = self.scalar_type()?;
        let class = RegisterClassId(u16::from_le_bytes(self.array()?));
        let origin = self.origin()?;
        let definition_site = self.definition_site()?;
        let classification = match self.byte()? {
            0 => {
                let defining_instruction = SelectedInstructionId(u32::from_le_bytes(self.array()?));
                let source_value = self.value_id()?;
                let value = self.integer_value()?;
                let provenance = self.provenance()?;
                let use_count = self.length()?;
                let mut future_uses = Vec::with_capacity(use_count.min(self.remaining()));
                for _ in 0..use_count {
                    future_uses.push(RecoveryFutureUse {
                        block: SelectedBlockId(u32::from_le_bytes(self.array()?)),
                        point: LiveRangePoint(u32::from_le_bytes(self.array()?)),
                        instruction: SelectedInstructionId(u32::from_le_bytes(self.array()?)),
                        operand: u16::from_le_bytes(self.array()?),
                    });
                }
                RecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction,
                    source_value,
                    value,
                    provenance,
                    future_uses,
                }
            }
            1 => RecoveryClassification::NoAdmittedRecovery {
                reason: match self.byte()? {
                    0 => NoAdmittedRecoveryReason::UnsupportedScalarType,
                    1 => NoAdmittedRecoveryReason::EntryParameter,
                    2 => NoAdmittedRecoveryReason::UnsupportedRangeShape,
                    3 => NoAdmittedRecoveryReason::FutureFixedUse,
                    4 => NoAdmittedRecoveryReason::NonMaterializeDefinition,
                    5 => NoAdmittedRecoveryReason::ProofBearingDefinition,
                    6 => NoAdmittedRecoveryReason::NoFutureUse,
                    tag => {
                        return Err(RecoveryClassificationDecodeError::UnknownReason(tag));
                    }
                },
            },
            tag => {
                return Err(RecoveryClassificationDecodeError::UnknownClassification(
                    tag,
                ));
            }
        };
        Ok(PressureRecoveryClassification {
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

    fn scalar_type(&mut self) -> Result<ScalarType, RecoveryClassificationDecodeError> {
        let scalar = match self.byte()? {
            0 => ScalarType::Boolean,
            1 => ScalarType::Integer(
                IntegerType::new(IntegerSign::Signed, u16::from_le_bytes(self.array()?))
                    .map_err(|_| RecoveryClassificationDecodeError::InvalidIntegerType)?,
            ),
            2 => ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, u16::from_le_bytes(self.array()?))
                    .map_err(|_| RecoveryClassificationDecodeError::InvalidIntegerType)?,
            ),
            3 => ScalarType::Integer(
                IntegerType::address(u16::from_le_bytes(self.array()?))
                    .map_err(|_| RecoveryClassificationDecodeError::InvalidIntegerType)?,
            ),
            tag => return Err(RecoveryClassificationDecodeError::UnknownScalarType(tag)),
        };
        Ok(scalar)
    }

    fn integer_value(&mut self) -> Result<IntegerValue, RecoveryClassificationDecodeError> {
        match self.byte()? {
            0 => Ok(IntegerValue::Signed(i128::from_le_bytes(self.array()?))),
            1 => Ok(IntegerValue::Unsigned(u128::from_le_bytes(self.array()?))),
            tag => Err(RecoveryClassificationDecodeError::UnknownIntegerValue(tag)),
        }
    }

    fn origin(&mut self) -> Result<VirtualRegisterOrigin, RecoveryClassificationDecodeError> {
        match self.byte()? {
            3 => Ok(VirtualRegisterOrigin::BlockParameter {
                source_value: self.value_id()?,
                block: selected_instructions::SelectedBlockId(u32::from_le_bytes(self.array()?)),
                parameter_index: self.length()?,
            }),
            0 => Ok(VirtualRegisterOrigin::EntryParameter {
                source_value: self.value_id()?,
                parameter_index: self.length()?,
            }),
            1 => Ok(VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(u32::from_le_bytes(self.array()?)),
                source_value: self.value_id()?,
            }),
            2 => Ok(VirtualRegisterOrigin::LegalizationTemporary {
                instruction: SelectedInstructionId(u32::from_le_bytes(self.array()?)),
                temporary: legalized_operations::LegalizedTemporaryId(u32::from_le_bytes(
                    self.array()?,
                )),
                source_value: self.value_id()?,
            }),
            tag => Err(RecoveryClassificationDecodeError::UnknownOrigin(tag)),
        }
    }

    fn definition_site(
        &mut self,
    ) -> Result<ValueDefinitionSite, RecoveryClassificationDecodeError> {
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
            tag => Err(RecoveryClassificationDecodeError::UnknownDefinitionSite(
                tag,
            )),
        }
    }

    fn provenance(
        &mut self,
    ) -> Result<SelectedInstructionProvenance, RecoveryClassificationDecodeError> {
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
                0 => optimization_unit::PsiProvenance::Operation(self.operation_id()?),
                1 => optimization_unit::PsiProvenance::Edge(self.edge_id()?),
                tag => {
                    return Err(RecoveryClassificationDecodeError::UnknownProvenance(tag));
                }
            };
            fuel.push(optimization_unit::FuelSettlement {
                site,
                units: u64::from_le_bytes(self.array()?),
            });
        }
        Ok(SelectedInstructionProvenance {
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
