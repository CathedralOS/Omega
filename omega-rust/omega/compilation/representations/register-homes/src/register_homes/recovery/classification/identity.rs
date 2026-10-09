use optimization_unit::{FuelSettlement, PsiProvenance, encode_value_definition_site_identity};
use selected_instructions::{SelectedInstructionProvenance, VirtualRegisterOrigin};
use semantic_vocabulary::{IntegerSign, ScalarType};

use crate::{
    NoAdmittedRecoveryReason, RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryVictimRole,
};

pub fn recovery_classification_identity(
    plan: &RecoveryClassificationPlan,
) -> RecoveryClassificationIdentity {
    RecoveryClassificationIdentity(crate::codec_bytes::domain_identity(
        b"omega.terminal-recovery-classification.v7\0",
        &encode_terminal_recovery_classification_content(plan),
    ))
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
    crate::codec_bytes::push_encoded(&mut bytes, plan.fuel_schedule.marker().to_le_bytes());
    bytes.push(match plan.policy {
        RecoveryClassificationPolicy::SelectedVictimImmediateU64EligibilityV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    crate::codec_bytes::length(
        &mut bytes,
        plan.functions.len(),
        "recovery-classification identity",
    );
    for function in &plan.functions {
        crate::codec_bytes::push_encoded(&mut bytes, function.machine.get().to_le_bytes());
        bytes.push(u8::from(function.classification.is_some()));
        if let Some(row) = &function.classification {
            crate::codec_bytes::push_encoded(&mut bytes, row.block.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, row.point.0.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut bytes, row.victim.0.to_le_bytes());
            match row.role {
                RecoveryVictimRole::Incoming => bytes.push(0),
                RecoveryVictimRole::ActiveResident {
                    current_view,
                    reclaimed_view,
                } => {
                    bytes.push(1);
                    crate::codec_bytes::push_encoded(&mut bytes, current_view.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, reclaimed_view.0.to_le_bytes());
                }
            }
            encode_scalar_type(&mut bytes, row.scalar_type);
            crate::codec_bytes::push_encoded(&mut bytes, row.class.0.to_le_bytes());
            encode_origin(&mut bytes, row.origin);
            match row.definition_site {
                Some(site) => {
                    bytes.push(1);
                    encode_value_definition_site_identity(&mut bytes, site);
                }
                None => bytes.push(0),
            }
            match &row.classification {
                RecoveryClassification::ImmediateU64RematerializationCandidate {
                    defining_instruction,
                    source_value,
                    value,
                    provenance,
                    future_uses,
                } => {
                    bytes.push(0);
                    crate::codec_bytes::push_encoded(&mut bytes, defining_instruction.0.to_le_bytes());
                    crate::codec_bytes::push_encoded(&mut bytes, source_value.get().to_le_bytes());
                    legalized_operations::encode_integer(&mut bytes, *value);
                    encode_provenance(&mut bytes, provenance);
                    crate::codec_bytes::length(
                        &mut bytes,
                        future_uses.len(),
                        "recovery-classification identity",
                    );
                    for future_use in future_uses {
                        crate::codec_bytes::push_encoded(&mut bytes, future_use.block.0.to_le_bytes());
                        crate::codec_bytes::push_encoded(&mut bytes, future_use.point.0.to_le_bytes());
                        crate::codec_bytes::push_encoded(&mut bytes, future_use.instruction.0.to_le_bytes());
                        crate::codec_bytes::push_encoded(&mut bytes, future_use.operand.to_le_bytes());
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
            crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
        }
        ScalarType::Integer(integer) => {
            bytes.push(match integer.sign() {
                IntegerSign::Signed => 1,
                IntegerSign::Unsigned => 2,
            });
            crate::codec_bytes::push_encoded(bytes, integer.bits().to_le_bytes());
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

fn encode_origin(bytes: &mut Vec<u8>, origin: VirtualRegisterOrigin) {
    match origin {
        VirtualRegisterOrigin::StructuralObservation {
            instruction,
            place,
            byte_offset,
        } => {
            bytes.push(8);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }
        VirtualRegisterOrigin::InstructionScratch {
            instruction,
            operand,
        } => {
            bytes.push(9);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, operand.to_le_bytes());
        }
        VirtualRegisterOrigin::ScalarAbiAddress {
            instruction,
            source_value,
        } => {
            bytes.push(7);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
        }
        VirtualRegisterOrigin::SpillAddress {
            instruction,
            register,
        } => {
            bytes.push(6);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, register.0.to_le_bytes());
        }

        VirtualRegisterOrigin::StructuralParameter {
            place,
            parameter_index,
        } => {
            bytes.push(4);
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            crate::codec_bytes::length(bytes, parameter_index, "recovery-classification identity");
        }
        VirtualRegisterOrigin::AbiTransport {
            instruction,
            place,
            byte_offset,
        } => {
            bytes.push(5);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, place.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, byte_offset.to_le_bytes());
        }

        VirtualRegisterOrigin::BlockParameter {
            source_value,
            block,
            parameter_index,
        } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, block.0.to_le_bytes());
            crate::codec_bytes::length(bytes, parameter_index, "recovery-classification identity");
        }
        VirtualRegisterOrigin::EntryParameter {
            source_value,
            parameter_index,
        } => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
            crate::codec_bytes::length(bytes, parameter_index, "recovery-classification identity");
        }
        VirtualRegisterOrigin::InstructionResult {
            instruction,
            source_value,
        } => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, instruction.0.to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, source_value.get().to_le_bytes());
        }
    }
}

fn encode_provenance(bytes: &mut Vec<u8>, provenance: &SelectedInstructionProvenance) {
    const LABEL: &str = "recovery-classification identity";
    crate::codec_bytes::list(bytes, &provenance.operations, LABEL, |bytes, operation| {
        crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
    });
    crate::codec_bytes::list(bytes, &provenance.values, LABEL, |bytes, value| {
        crate::codec_bytes::push_encoded(bytes, value.get().to_le_bytes());
    });
    crate::codec_bytes::list(bytes, &provenance.edges, LABEL, |bytes, edge| {
        crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
    });
    crate::codec_bytes::list(
        bytes,
        &provenance.obligations,
        LABEL,
        |bytes, obligation| {
            crate::codec_bytes::push_encoded(bytes, obligation.get().to_le_bytes());
        },
    );
    crate::codec_bytes::list(bytes, &provenance.fuel, LABEL, |bytes, settlement| {
        encode_fuel(bytes, *settlement);
    });
}

fn encode_fuel(bytes: &mut Vec<u8>, settlement: FuelSettlement) {
    match settlement.site {
        PsiProvenance::Operation(operation) => {
            bytes.push(0);
            crate::codec_bytes::push_encoded(bytes, operation.get().to_le_bytes());
        }
        PsiProvenance::Edge(edge) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, edge.get().to_le_bytes());
        }
    }
    crate::codec_bytes::push_encoded(bytes, settlement.units.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::{
        RecoveryClassification, RecoveryClassificationPlan, RecoveryClassificationPolicy,
        RecoveryVictimRole, recovery_classification_identity,
    };
    use crate::{
        AllocationLegalityIdentity, FunctionRecoveryClassification, PressureRecoveryClassification,
        RecoveryClassificationDecodeError, RecoveryFutureUse, SpillChoiceIdentity,
    };
    use optimization_core::{
        OptimizationUnitIdentity, OptimizationWorkBudget, OptimizationWorkUsage,
    };
    use optimization_unit::{FuelSettlement, PsiProvenance, ValueDefinitionSite};
    use register_model::{RegisterClassId, RegisterViewId, TargetRegisterEnvironmentIdentity};
    use selected_instructions::{LiveRangeIdentity, LiveRangePoint};
    use selected_instructions::{
        SelectedBlockId, SelectedInstructionId, SelectedInstructionPlanIdentity,
        SelectedInstructionProvenance, VirtualRegisterId, VirtualRegisterOrigin,
    };
    use semantic_vocabulary::{
        BlockId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
        ScalarType, ValueId,
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
                    definition_site: Some(ValueDefinitionSite::Node {
                        block: BlockId::new(10).unwrap(),
                        node: 11,
                    }),
                    classification:
                        RecoveryClassification::ImmediateU64RematerializationCandidate {
                            defining_instruction: SelectedInstructionId(7),
                            source_value: value,
                            value: semantic_vocabulary::IntegerValue::Unsigned(12),
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
    fn scalar_abi_address_recovery_codec_retains_its_distinct_origin() {
        let mut source = plan();
        let value = ValueId::new(17).unwrap();
        let row = source.functions[0].classification.as_mut().unwrap();
        row.origin = VirtualRegisterOrigin::ScalarAbiAddress {
            instruction: SelectedInstructionId(3),
            source_value: value,
        };
        row.classification = RecoveryClassification::NoAdmittedRecovery {
            reason: crate::NoAdmittedRecoveryReason::UnsupportedRangeShape,
        };
        assert_eq!(
            RecoveryClassificationPlan::decode(&source.encode()).unwrap(),
            source
        );
        let identity = recovery_classification_identity(&source);
        for origin in [
            VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: SelectedInstructionId(4),
                source_value: value,
            },
            VirtualRegisterOrigin::ScalarAbiAddress {
                instruction: SelectedInstructionId(3),
                source_value: ValueId::new(18).unwrap(),
            },
            VirtualRegisterOrigin::InstructionResult {
                instruction: SelectedInstructionId(3),
                source_value: value,
            },
        ] {
            let mut changed = source.clone();
            changed.functions[0].classification.as_mut().unwrap().origin = origin;
            assert_ne!(recovery_classification_identity(&changed), identity);
        }
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
        crate::codec_bytes::patch_encoded(&mut wrong_version, 8, 3_u32.to_le_bytes());
        assert_eq!(
            RecoveryClassificationPlan::decode(&wrong_version),
            Err(RecoveryClassificationDecodeError::UnsupportedVersion(3))
        );
    }
}
