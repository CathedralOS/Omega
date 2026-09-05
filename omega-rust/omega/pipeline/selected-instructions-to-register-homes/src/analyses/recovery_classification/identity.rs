use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
use selected_instructions::{SelectedInstructionProvenance, VirtualRegisterOrigin};
use semantic_vocabulary::{IntegerSign, IntegerValue, ScalarType};
use sha2::{Digest, Sha256};

use crate::{
    NoAdmittedRecoveryReason, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryVictimRole,
};

pub fn recovery_classification_identity(
    plan: &RecoveryClassificationPlan,
) -> RecoveryClassificationIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-recovery-classification.v3\0");
    bytes.extend_from_slice(&encode_terminal_recovery_classification_content(plan));
    RecoveryClassificationIdentity::from_bytes(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_recovery_classification_content(
    plan: &RecoveryClassificationPlan,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.selected.bytes());
    bytes.extend_from_slice(&plan.spill_choices.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.extend_from_slice(&plan.allocator_availability.bytes());
    bytes.extend_from_slice(&plan.optimization_unit.bytes());
    bytes.extend_from_slice(&plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.push(u8::from(function.classification.is_some()));
        if let Some(row) = &function.classification {
            bytes.extend_from_slice(&row.block.0.to_le_bytes());
            bytes.extend_from_slice(&row.point.0.to_le_bytes());
            bytes.extend_from_slice(&row.victim.0.to_le_bytes());
            match row.role {
                RecoveryVictimRole::Incoming => bytes.push(0),
                RecoveryVictimRole::ActiveResident {
                    current_view,
                    reclaimed_view,
                } => {
                    bytes.push(1);
                    bytes.extend_from_slice(&current_view.0.to_le_bytes());
                    bytes.extend_from_slice(&reclaimed_view.0.to_le_bytes());
                }
            }
            encode_scalar_type(&mut bytes, row.scalar_type);
            bytes.extend_from_slice(&row.class.0.to_le_bytes());
            encode_origin(&mut bytes, row.origin);
            encode_definition_site(&mut bytes, row.definition_site);
            match &row.classification {
                RecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction,
                    source_value,
                    value,
                    provenance,
                    future_uses,
                } => {
                    bytes.push(0);
                    bytes.extend_from_slice(&defining_instruction.0.to_le_bytes());
                    bytes.extend_from_slice(&source_value.get().to_le_bytes());
                    encode_integer_value(&mut bytes, *value);
                    encode_provenance(&mut bytes, provenance);
                    encode_len(&mut bytes, future_uses.len());
                    for future_use in future_uses {
                        bytes.extend_from_slice(&future_use.block.0.to_le_bytes());
                        bytes.extend_from_slice(&future_use.point.0.to_le_bytes());
                        bytes.extend_from_slice(&future_use.instruction.0.to_le_bytes());
                        bytes.extend_from_slice(&future_use.operand.to_le_bytes());
                    }
                }
                RecoveryClassification::NoAdmittedRecovery { reason } => {
                    bytes.push(1);
                    bytes.push(match reason {
                        NoAdmittedRecoveryReason::UnsupportedScalarType => 0,
                        NoAdmittedRecoveryReason::EntryParameter => 1,
                        NoAdmittedRecoveryReason::UnsupportedRangeShape => 2,
                        NoAdmittedRecoveryReason::FutureFixedUse => 3,
                        NoAdmittedRecoveryReason::NonMaterializeDefinition => 4,
                        NoAdmittedRecoveryReason::ProofBearingDefinition => 5,
                        NoAdmittedRecoveryReason::NoFutureUse => 6,
                    });
                }
            }
        }
    }
    bytes
}

fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => bytes.push(0),
        ScalarType::Integer(integer) if integer.is_address() => {
            bytes.push(3);
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        ScalarType::Integer(integer) => {
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            bytes.extend_from_slice(&integer.bits().to_le_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            bytes.push(4);
            bytes.push(match format {
                semantic_vocabulary::IeeeFloatFormat::Binary32 => 0,
                semantic_vocabulary::IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
}

fn encode_integer_value(bytes: &mut Vec<u8>, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            bytes.push(0);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            bytes.push(1);
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
}

fn encode_origin(bytes: &mut Vec<u8>, origin: VirtualRegisterOrigin) {
    match origin {
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
            encode_len(bytes, parameter_index);
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
        VirtualRegisterOrigin::LegalizationTemporary {
            instruction,
            temporary,
            source_value,
        } => {
            bytes.push(2);
            bytes.extend_from_slice(&instruction.0.to_le_bytes());
            bytes.extend_from_slice(&temporary.0.to_le_bytes());
            bytes.extend_from_slice(&source_value.get().to_le_bytes());
        }
    }
}

fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(0);
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(1);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(2);
            bytes.extend_from_slice(&block.get().to_le_bytes());
            bytes.extend_from_slice(&node.to_le_bytes());
        }
    }
}

fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    encode_len(bytes, provenance.operations.len());
    for operation in &provenance.operations {
        bytes.extend_from_slice(&operation.get().to_le_bytes());
    }
    encode_len(bytes, provenance.values.len());
    for value in &provenance.values {
        bytes.extend_from_slice(&value.get().to_le_bytes());
    }
    encode_len(bytes, provenance.edges.len());
    for edge in &provenance.edges {
        bytes.extend_from_slice(&edge.get().to_le_bytes());
    }
    encode_len(bytes, provenance.obligations.len());
    for obligation in &provenance.obligations {
        bytes.extend_from_slice(&obligation.get().to_le_bytes());
    }
    encode_len(bytes, provenance.fuel.len());
    for settlement in &provenance.fuel {
        encode_fuel(bytes, *settlement);
    }
}

fn encode_fuel(bytes: &mut Vec<u8>, settlement: FuelSettlement) {
    match settlement.site {
        PsiProvenance::Operation(operation) => {
            bytes.push(0);
            bytes.extend_from_slice(&operation.get().to_le_bytes());
        }
        PsiProvenance::Edge(edge) => {
            bytes.push(1);
            bytes.extend_from_slice(&edge.get().to_le_bytes());
        }
    }
    bytes.extend_from_slice(&settlement.units.to_le_bytes());
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("recovery-classification identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use optimization_core::{
        OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
    use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
    use selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
        SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, MachineId,
        OperationId, ScalarType, ValueId,
    };

    use super::*;
    use crate::{
        AllocationLegalityIdentity, FunctionRecoveryClassification, LiveRangeIdentity,
        LiveRangePoint, PressureRecoveryClassification, RecoveryClassificationDecodeError,
        RecoveryFutureUse, SpillChoiceIdentity,
    };

    fn plan() -> RecoveryClassificationPlan {
        let operation = OperationId::new(8).unwrap();
        let value = ValueId::new(9).unwrap();
        RecoveryClassificationPlan {
            selected: SelectedInstructionPlanIdentity::from_bytes([1; 32]),
            spill_choices: SpillChoiceIdentity::from_bytes([2; 32]),
            ranges: LiveRangeIdentity::from_bytes([3; 32]),
            legality: AllocationLegalityIdentity::from_bytes([4; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([5; 32]),
            allocator_availability: crate::AllocatorAvailabilityIdentity::from_bytes([9; 32]),
            optimization_unit: OptimizationUnitIdentity::from_bytes([6; 32]),
            fuel_schedule: FuelScheduleIdentity::new(1).unwrap(),
            policy: RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1,
            budget: OptimizationWorkBudget::new(10, 10, 10, 10, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 1,
                validation_steps: 7,
                commits: 1,
                iterations: 1,
            },
            functions: vec![FunctionRecoveryClassification {
                machine: MachineId::new(7).unwrap(),
                classification: Some(PressureRecoveryClassification {
                    block: SelectedBlockId(1),
                    point: LiveRangePoint(2),
                    victim: VirtualRegisterId(3),
                    role: RecoveryVictimRole::ActiveResident {
                        current_view: RegisterViewId(4),
                        reclaimed_view: RegisterViewId(5),
                    },
                    scalar_type: ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    ),
                    class: RegisterClassId(6),
                    origin: VirtualRegisterOrigin::InstructionResult {
                        instruction: SelectedInstructionId(7),
                        source_value: value,
                    },
                    definition_site: ValueDefinitionSite::Node {
                        block: BlockId::new(10).unwrap(),
                        node: 11,
                    },
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: SelectedInstructionId(7),
                            source_value: value,
                            value: IntegerValue::Unsigned(12),
                            provenance: SelectedInstructionProvenance {
                                operations: vec![operation],
                                values: vec![value],
                                edges: Vec::new(),
                                obligations: Vec::new(),
                                fuel: vec![FuelSettlement {
                                    site: PsiProvenance::Operation(operation),
                                    units: 13,
                                }],
                            },
                            future_uses: vec![RecoveryFutureUse {
                                block: SelectedBlockId(1),
                                point: LiveRangePoint(14),
                                instruction: SelectedInstructionId(15),
                                operand: 0,
                            }],
                        },
                }),
            }],
        }
    }

    #[test]
    fn identity_is_deterministic_and_binds_roots_and_nested_classification() {
        let baseline = recovery_classification_identity(&plan());
        assert_eq!(baseline, recovery_classification_identity(&plan()));

        let mut changed = plan();
        changed.spill_choices = SpillChoiceIdentity::from_bytes([20; 32]);
        assert_ne!(baseline, recovery_classification_identity(&changed));

        let mut changed = plan();
        changed.allocator_availability = crate::AllocatorAvailabilityIdentity::from_bytes([21; 32]);
        assert_ne!(baseline, recovery_classification_identity(&changed));

        let mut changed = plan();
        changed.usage.validation_steps += 1;
        assert_ne!(baseline, recovery_classification_identity(&changed));

        let mut changed = plan();
        let Some(row) = &mut changed.functions[0].classification else {
            unreachable!()
        };
        let RecoveryClassification::ImmediateU64RematerializationCandidate {
            provenance,
            future_uses,
            ..
        } = &mut row.classification
        else {
            unreachable!()
        };
        provenance.fuel[0].units += 1;
        future_uses[0].point.0 += 1;
        assert_ne!(baseline, recovery_classification_identity(&changed));
    }

    #[test]
    fn canonical_codec_round_trips_and_rejects_corrupt_framing_and_identity() {
        let plan = plan();
        let encoded = plan.encode();
        assert_eq!(RecoveryClassificationPlan::decode(&encoded), Ok(plan));

        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            RecoveryClassificationPlan::decode(&identity_tamper),
            Err(RecoveryClassificationDecodeError::IdentityMismatch)
        );

        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            RecoveryClassificationPlan::decode(&trailing),
            Err(RecoveryClassificationDecodeError::TrailingBytes)
        );
        assert_eq!(
            RecoveryClassificationPlan::decode(&encoded[..encoded.len() - 1]),
            Err(RecoveryClassificationDecodeError::Truncated)
        );

        let mut wrong_version = encoded;
        wrong_version[8..12].copy_from_slice(&4_u32.to_le_bytes());
        assert_eq!(
            RecoveryClassificationPlan::decode(&wrong_version),
            Err(RecoveryClassificationDecodeError::UnsupportedVersion(4))
        );
    }

    #[test]
    fn canonical_codec_binds_legalization_temporary_origins() {
        let baseline = plan();
        let mut legalized = baseline.clone();
        legalized.functions[0]
            .classification
            .as_mut()
            .unwrap()
            .origin = VirtualRegisterOrigin::LegalizationTemporary {
            instruction: SelectedInstructionId(7),
            temporary: legalized_operations::LegalizedTemporaryId(17),
            source_value: ValueId::new(9).unwrap(),
        };
        assert_ne!(
            recovery_classification_identity(&baseline),
            recovery_classification_identity(&legalized)
        );
        assert_eq!(
            RecoveryClassificationPlan::decode(&legalized.encode()),
            Ok(legalized)
        );
    }
}
