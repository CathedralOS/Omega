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

use super::identity::encode_terminal_recovery_classification_content;
use crate::codec_bytes::Envelope;
use crate::{
    AllocationLegalityIdentity, AllocatorAvailabilityIdentity, FunctionRecoveryClassification,
    NoAdmittedRecoveryReason, PressureRecoveryClassification, RecoveryClassification,
    RecoveryClassificationDecodeError, RecoveryClassificationIdentity, RecoveryClassificationPlan,
    RecoveryClassificationPolicy, RecoveryFutureUse, RecoveryVictimRole, SpillChoiceIdentity,
};
use selected_instructions::{LiveRangeIdentity, LiveRangePoint};

const RECOVERY_CLASSIFICATION_ENVELOPE: Envelope = Envelope::new(b"OMGRCV\0\0", 8);

impl RecoveryClassificationPlan {
    /// Canonical transport only. Decoding returns an unchecked plain plan; only
    /// independent replay against all retained roots may grant classification
    /// authority.
    pub fn encode(&self) -> Vec<u8> {
        let content = encode_terminal_recovery_classification_content(self);
        let identity = crate::recovery_classification_identity(self);
        RECOVERY_CLASSIFICATION_ENVELOPE.frame(identity.bytes(), &content)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, RecoveryClassificationDecodeError> {
        let (mut cursor, identity) = RECOVERY_CLASSIFICATION_ENVELOPE.open(
            encoded,
            || RecoveryClassificationDecodeError::WrongMagic,
            RecoveryClassificationDecodeError::UnsupportedVersion,
        )?;
        let identity = RecoveryClassificationIdentity::from_bytes(identity);
        let selected = SelectedInstructionPlanIdentity::from_bytes(cursor.array()?);
        let spill_choices = SpillChoiceIdentity::from_bytes(cursor.array()?);
        let ranges = LiveRangeIdentity::from_bytes(cursor.array()?);
        let legality = AllocationLegalityIdentity::from_bytes(cursor.array()?);
        let register_environment = TargetRegisterEnvironmentIdentity::from_bytes(cursor.array()?);
        let allocator_availability = AllocatorAvailabilityIdentity::from_bytes(cursor.array()?);
        let optimization_unit = OptimizationUnitIdentity::from_bytes(cursor.array()?);
        let raw_fuel_schedule = cursor.u32()?;
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
        let functions = crate::codec_bytes::decode_list::<_, RecoveryClassificationDecodeError>(
            &mut cursor,
            |cursor| {
                let machine = machine(&mut *cursor)?;
                let classification = crate::cursor::optional(
                    &mut *cursor,
                    |c| Ok(classification_row(&mut *c)?),
                    RecoveryClassificationDecodeError::UnknownOption,
                )?;
                Ok(FunctionRecoveryClassification {
                    machine,
                    classification,
                })
            },
        )?;
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

fn machine(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<MachineId, RecoveryClassificationDecodeError> {
    let raw = cursor.u64()?;
    MachineId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidMachineId(raw))
}

fn block_id(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<BlockId, RecoveryClassificationDecodeError> {
    let raw = cursor.u64()?;
    BlockId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidBlockId(raw))
}

fn operation_id(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<OperationId, RecoveryClassificationDecodeError> {
    let raw = cursor.u64()?;
    OperationId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidOperationId(raw))
}

fn value_id(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<ValueId, RecoveryClassificationDecodeError> {
    let raw = cursor.u64()?;
    ValueId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidValueId(raw))
}

fn edge_id(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<EdgeId, RecoveryClassificationDecodeError> {
    let raw = cursor.u64()?;
    EdgeId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidEdgeId(raw))
}

fn obligation_id(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<ObligationId, RecoveryClassificationDecodeError> {
    let raw = cursor.u64()?;
    ObligationId::new(raw).ok_or(RecoveryClassificationDecodeError::InvalidObligationId(raw))
}

fn classification_row(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<PressureRecoveryClassification, RecoveryClassificationDecodeError> {
    let block = SelectedBlockId(cursor.u32()?);
    let point = LiveRangePoint(cursor.u32()?);
    let victim = VirtualRegisterId(cursor.u32()?);
    let role = match cursor.byte()? {
        0 => RecoveryVictimRole::Incoming,
        1 => RecoveryVictimRole::ActiveResident {
            current_view: RegisterViewId(cursor.u16()?),
            reclaimed_view: RegisterViewId(cursor.u16()?),
        },
        tag => {
            return Err(RecoveryClassificationDecodeError::UnknownVictimRole(tag));
        }
    };
    let scalar_type = scalar_type(cursor)?;
    let class = RegisterClassId(cursor.u16()?);
    let origin = origin(cursor)?;
    let definition_site = crate::cursor::optional(
        cursor,
        definition_site,
        RecoveryClassificationDecodeError::UnknownDefinitionSite,
    )?;
    let classification = match cursor.byte()? {
        0 => {
            let defining_instruction = SelectedInstructionId(cursor.u32()?);
            let source_value = value_id(cursor)?;
            let value = integer_value(cursor)?;
            let provenance = provenance(cursor)?;
            let use_count = cursor.length()?;
            let mut future_uses = Vec::with_capacity(use_count.min(cursor.remaining()));
            for _ in 0..use_count {
                future_uses.push(RecoveryFutureUse {
                    block: SelectedBlockId(cursor.u32()?),
                    point: LiveRangePoint(cursor.u32()?),
                    instruction: SelectedInstructionId(cursor.u32()?),
                    operand: cursor.u16()?,
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
            reason: match cursor.byte()? {
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

fn scalar_type(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<ScalarType, RecoveryClassificationDecodeError> {
    let scalar = match cursor.byte()? {
        0 => ScalarType::Boolean,
        1 => ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, cursor.u16()?)
                .map_err(|_| RecoveryClassificationDecodeError::InvalidIntegerType)?,
        ),
        2 => ScalarType::Integer(
            IntegerType::new(IntegerSign::Unsigned, cursor.u16()?)
                .map_err(|_| RecoveryClassificationDecodeError::InvalidIntegerType)?,
        ),
        3 => ScalarType::Integer(
            IntegerType::address(cursor.u16()?)
                .map_err(|_| RecoveryClassificationDecodeError::InvalidIntegerType)?,
        ),
        tag => return Err(RecoveryClassificationDecodeError::UnknownScalarType(tag)),
    };
    Ok(scalar)
}

fn integer_value(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<IntegerValue, RecoveryClassificationDecodeError> {
    match cursor.byte()? {
        0 => Ok(IntegerValue::Signed(cursor.i128()?)),
        1 => Ok(IntegerValue::Unsigned(cursor.u128()?)),
        tag => Err(RecoveryClassificationDecodeError::UnknownIntegerValue(tag)),
    }
}

fn origin(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<VirtualRegisterOrigin, RecoveryClassificationDecodeError> {
    match cursor.byte()? {
        9 => Ok(VirtualRegisterOrigin::InstructionScratch {
            instruction: SelectedInstructionId(cursor.u32()?),
            operand: cursor.u16()?,
        }),
        7 => Ok(VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: value_id(cursor)?,
        }),
        6 => Ok(VirtualRegisterOrigin::SpillAddress {
            instruction: SelectedInstructionId(cursor.u32()?),
            register: selected_instructions::VirtualRegisterId(cursor.u32()?),
        }),
        4 => {
            let raw = cursor.u64()?;
            Ok(VirtualRegisterOrigin::StructuralParameter {
                place: semantic_vocabulary::PlaceId::new(raw)
                    .ok_or(RecoveryClassificationDecodeError::InvalidPlaceId(raw))?,
                parameter_index: cursor.length()?,
            })
        }
        8 => {
            let instruction = SelectedInstructionId(cursor.u32()?);
            let raw = cursor.u64()?;
            Ok(VirtualRegisterOrigin::StructuralObservation {
                instruction,
                place: semantic_vocabulary::PlaceId::new(raw)
                    .ok_or(RecoveryClassificationDecodeError::InvalidPlaceId(raw))?,
                byte_offset: cursor.u32()?,
            })
        }
        5 => {
            let instruction = SelectedInstructionId(cursor.u32()?);
            let raw = cursor.u64()?;
            Ok(VirtualRegisterOrigin::AbiTransport {
                instruction,
                place: semantic_vocabulary::PlaceId::new(raw)
                    .ok_or(RecoveryClassificationDecodeError::InvalidPlaceId(raw))?,
                byte_offset: cursor.u32()?,
            })
        }
        3 => Ok(VirtualRegisterOrigin::BlockParameter {
            source_value: value_id(cursor)?,
            block: selected_instructions::SelectedBlockId(cursor.u32()?),
            parameter_index: cursor.length()?,
        }),
        0 => Ok(VirtualRegisterOrigin::EntryParameter {
            source_value: value_id(cursor)?,
            parameter_index: cursor.length()?,
        }),
        1 => Ok(VirtualRegisterOrigin::InstructionResult {
            instruction: SelectedInstructionId(cursor.u32()?),
            source_value: value_id(cursor)?,
        }),
        tag => Err(RecoveryClassificationDecodeError::UnknownOrigin(tag)),
    }
}

fn definition_site(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<ValueDefinitionSite, RecoveryClassificationDecodeError> {
    match cursor.byte()? {
        0 => Ok(ValueDefinitionSite::FunctionParameter(cursor.u32()?)),
        1 => Ok(ValueDefinitionSite::BlockParameter {
            block: block_id(cursor)?,
            position: cursor.u32()?,
        }),
        2 => Ok(ValueDefinitionSite::Node {
            block: block_id(cursor)?,
            node: cursor.u32()?,
        }),
        tag => Err(RecoveryClassificationDecodeError::UnknownDefinitionSite(
            tag,
        )),
    }
}

fn provenance(
    cursor: &mut crate::cursor::Cursor<'_>,
) -> Result<SelectedInstructionProvenance, RecoveryClassificationDecodeError> {
    let operations = crate::codec_bytes::decode_list(cursor, operation_id)?;
    let values = crate::codec_bytes::decode_list(cursor, value_id)?;
    let edges = crate::codec_bytes::decode_list(cursor, edge_id)?;
    let obligations = crate::codec_bytes::decode_list(cursor, obligation_id)?;
    let fuel_count = cursor.length()?;
    let mut fuel = Vec::with_capacity(fuel_count.min(cursor.remaining()));
    for _ in 0..fuel_count {
        let site = match cursor.byte()? {
            0 => optimization_unit::PsiProvenance::Operation(operation_id(cursor)?),
            1 => optimization_unit::PsiProvenance::Edge(edge_id(cursor)?),
            tag => {
                return Err(RecoveryClassificationDecodeError::UnknownProvenance(tag));
            }
        };
        fuel.push(optimization_unit::FuelSettlement {
            site,
            units: cursor.u64()?,
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
