use sha2::{Digest, Sha256};

use crate::{TerminalSpillChoiceIdentity, TerminalSpillChoicePlan, TerminalSpillChoicePolicy};

pub fn terminal_spill_choice_identity(
    plan: &TerminalSpillChoicePlan,
) -> TerminalSpillChoiceIdentity {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"omega.terminal-spill-choices.v1\0");
    bytes.extend_from_slice(&encode_terminal_spill_choice_content(plan));
    TerminalSpillChoiceIdentity(Sha256::digest(bytes).into())
}

pub(crate) fn encode_terminal_spill_choice_content(plan: &TerminalSpillChoicePlan) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&plan.legality.bytes());
    bytes.extend_from_slice(&plan.ranges.bytes());
    bytes.extend_from_slice(&plan.register_environment.bytes());
    bytes.push(match plan.policy {
        TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1 => 0,
    });
    bytes.extend_from_slice(&plan.budget.encode());
    bytes.extend_from_slice(&plan.usage.encode());
    encode_len(&mut bytes, plan.functions.len());
    for function in &plan.functions {
        bytes.extend_from_slice(&function.machine.get().to_le_bytes());
        bytes.push(u8::from(function.choice.is_some()));
        if let Some(choice) = &function.choice {
            bytes.extend_from_slice(&choice.block.0.to_le_bytes());
            bytes.extend_from_slice(&choice.point.0.to_le_bytes());
            bytes.extend_from_slice(&choice.incoming.0.to_le_bytes());
            bytes.extend_from_slice(&choice.incoming_class.0.to_le_bytes());
            encode_len(&mut bytes, choice.incoming_common_candidates.len());
            for view in &choice.incoming_common_candidates {
                bytes.extend_from_slice(&view.0.to_le_bytes());
            }
            encode_len(&mut bytes, choice.active_residents.len());
            for resident in &choice.active_residents {
                bytes.extend_from_slice(&resident.virtual_register.0.to_le_bytes());
                bytes.extend_from_slice(&resident.class.0.to_le_bytes());
                bytes.extend_from_slice(&resident.start.0.to_le_bytes());
                bytes.extend_from_slice(&resident.exclusive_end.0.to_le_bytes());
                bytes.extend_from_slice(&resident.view.0.to_le_bytes());
            }
            encode_len(&mut bytes, choice.contenders.len());
            for contender in &choice.contenders {
                bytes.extend_from_slice(&contender.virtual_register.0.to_le_bytes());
                bytes.extend_from_slice(&contender.exclusive_end.0.to_le_bytes());
                match contender.reclaimed_view {
                    None => bytes.push(0),
                    Some(view) => {
                        bytes.push(1);
                        bytes.extend_from_slice(&view.0.to_le_bytes());
                    }
                }
            }
            bytes.extend_from_slice(&choice.selected_victim.0.to_le_bytes());
        }
    }
    bytes
}

fn encode_len(bytes: &mut Vec<u8>, value: usize) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .expect("spill-choice identity length fits u64")
            .to_le_bytes(),
    );
}

#[cfg(test)]
mod tests {
    use omega_optimization_core::{OptimizationWorkBudget, OptimizationWorkUsage};
    use omega_register_model::TargetRegisterEnvironmentIdentity;

    use super::*;
    use crate::{
        TerminalAllocationLegalityIdentity, TerminalLiveRangeIdentity, TerminalSpillChoicePlan,
        TerminalSpillChoicePolicy,
    };

    #[test]
    fn identity_binds_roots_policy_work_and_functions() {
        let plan = TerminalSpillChoicePlan {
            legality: TerminalAllocationLegalityIdentity([1; 32]),
            ranges: TerminalLiveRangeIdentity([2; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            policy: TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            budget: OptimizationWorkBudget::new(1, 1, 1, 1, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 1,
                candidates: 0,
                validation_steps: 1,
                commits: 0,
                iterations: 1,
            },
            functions: Vec::new(),
        };
        let baseline = terminal_spill_choice_identity(&plan);
        let mut changed = plan.clone();
        changed.usage.validation_steps += 1;
        assert_ne!(baseline, terminal_spill_choice_identity(&changed));
        changed = plan.clone();
        changed.ranges = TerminalLiveRangeIdentity([4; 32]);
        assert_ne!(baseline, terminal_spill_choice_identity(&changed));
    }

    #[test]
    fn canonical_codec_round_trips_and_rejects_framing_and_identity_corruption() {
        use omega_register_model::{RegisterClassId, RegisterViewId};
        use omega_terminal_selected_instructions::{
            TerminalSelectedBlockId, TerminalVirtualRegisterId,
        };
        use psi_core::MachineId;

        use crate::{
            TerminalFunctionSpillChoices, TerminalLiveRangePoint, TerminalPressureContender,
            TerminalPressureResident, TerminalSpillChoice, TerminalSpillChoiceDecodeError,
        };

        let plan = TerminalSpillChoicePlan {
            legality: TerminalAllocationLegalityIdentity([1; 32]),
            ranges: TerminalLiveRangeIdentity([2; 32]),
            register_environment: TargetRegisterEnvironmentIdentity::from_bytes([3; 32]),
            policy: TerminalSpillChoicePolicy::SingleBlockFarthestEndThenHighestVregV1,
            budget: OptimizationWorkBudget::new(10, 10, 20, 2, 1).unwrap(),
            usage: OptimizationWorkUsage {
                rule_evaluations: 3,
                candidates: 2,
                validation_steps: 7,
                commits: 1,
                iterations: 1,
            },
            functions: vec![TerminalFunctionSpillChoices {
                machine: MachineId::new(1).unwrap(),
                choice: Some(TerminalSpillChoice {
                    block: TerminalSelectedBlockId(0),
                    point: TerminalLiveRangePoint(2),
                    incoming: TerminalVirtualRegisterId(2),
                    incoming_class: RegisterClassId(0),
                    incoming_common_candidates: vec![RegisterViewId(0)],
                    active_residents: vec![TerminalPressureResident {
                        virtual_register: TerminalVirtualRegisterId(0),
                        class: RegisterClassId(0),
                        start: TerminalLiveRangePoint(0),
                        exclusive_end: TerminalLiveRangePoint(4),
                        view: RegisterViewId(0),
                    }],
                    contenders: vec![
                        TerminalPressureContender {
                            virtual_register: TerminalVirtualRegisterId(0),
                            exclusive_end: TerminalLiveRangePoint(4),
                            reclaimed_view: Some(RegisterViewId(0)),
                        },
                        TerminalPressureContender {
                            virtual_register: TerminalVirtualRegisterId(2),
                            exclusive_end: TerminalLiveRangePoint(3),
                            reclaimed_view: None,
                        },
                    ],
                    selected_victim: TerminalVirtualRegisterId(0),
                }),
            }],
        };
        let encoded = plan.encode();
        assert_eq!(TerminalSpillChoicePlan::decode(&encoded), Ok(plan));

        let mut identity_tamper = encoded.clone();
        identity_tamper[12] ^= 1;
        assert_eq!(
            TerminalSpillChoicePlan::decode(&identity_tamper),
            Err(TerminalSpillChoiceDecodeError::IdentityMismatch)
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert_eq!(
            TerminalSpillChoicePlan::decode(&trailing),
            Err(TerminalSpillChoiceDecodeError::TrailingBytes)
        );
        assert_eq!(
            TerminalSpillChoicePlan::decode(&encoded[..encoded.len() - 1]),
            Err(TerminalSpillChoiceDecodeError::Truncated)
        );
        let mut wrong_version = encoded;
        wrong_version[8..12].copy_from_slice(&2_u32.to_le_bytes());
        assert_eq!(
            TerminalSpillChoicePlan::decode(&wrong_version),
            Err(TerminalSpillChoiceDecodeError::UnsupportedVersion(2))
        );
    }
}
