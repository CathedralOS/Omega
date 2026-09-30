use super::*;
use crate::effects::AcceptTerminalEffects;
use crate::execution::LiveClaim;
use crate::results::TerminalExecutionResult;
use semantic_vocabulary::{
    BlockId, ClaimId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
    OperationId,
};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    Block, CrashRouteBucket, CrashRouteGuard, MachineContract, OperationResult, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration, VocabularyMarker,
};

fn value(identity: u64) -> ValueId {
    ValueId::new(identity).unwrap()
}

fn declaration(identity: u64, format: IeeeFloatFormat) -> ValueDeclaration {
    ValueDeclaration {
        id: value(identity),
        scalar_type: ScalarType::IeeeFloat(format),
        qualifications: Default::default(),
    }
}

fn policy_operation(format: IeeeFloatFormat, rule: Option<FloatSaturationRule>) -> Operation {
    Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(4).unwrap(),
        result: OperationResult::Scalar(declaration(4, format)),
        kind: OperationKind::FloatResultPolicy {
            policy: match rule {
                Some(rule) => FloatResultPolicy::Saturating {
                    raw_result: value(3),
                    left: value(1),
                    right: value(2),
                    rule,
                },
                None => FloatResultPolicy::Trapping {
                    raw_result: value(3),
                },
            },
        },
    }
}

fn module(payloads: [IeeeFloatValue; 3], rule: Option<FloatSaturationRule>) -> TerminalModule {
    let format = payloads[2].format();
    let mut operations: Vec<_> = payloads
        .into_iter()
        .enumerate()
        .map(|(position, payload)| {
            let identity = position as u64 + 1;
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(identity).unwrap(),
                result: OperationResult::Scalar(declaration(identity, format)),
                kind: OperationKind::IeeeFloatConstant { value: payload },
            }
        })
        .collect();
    operations.push(policy_operation(format, rule));
    // This result must never be published after a policy trap.
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(5).unwrap(),
        result: OperationResult::Scalar(declaration(5, format)),
        kind: OperationKind::IeeeFloatConstant { value: payloads[0] },
    });
    TerminalModule {
        scalar_qualifications: Default::default(),
        scalar_block_invariants: Vec::new(),
        operation_crash_contracts: Vec::new(),
        vocabulary_marker: VocabularyMarker::CURRENT,
        entry: MachineId::new(7).unwrap(),
        structural_types: Vec::new(),
        structural_domains: Vec::new(),
        services: Vec::new(),
        root_service_reach: Default::default(),
        placed_view_inputs: Vec::new(),
        reborrow_root_handoffs: Vec::new(),
        reborrow_restored_call_uses: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        float_meaning_projections: Vec::new(),
        float_meaning_equalities: Vec::new(),
        proposition_declarations: Vec::new(),
        proposition_applications: Vec::new(),
        evidence_terms: Vec::new(),
        evidence_contract_lanes: Vec::new(),
        proof_output_calls: Vec::new(),
        proof_recursive_components: Vec::new(),
        closed_conformance_applications: Vec::new(),
        dynamic_dispatch: Default::default(),
        suspension_call_plan_count: 0,
        suspension_call_sites: Vec::new(),
        suspension_call_plans: Vec::new(),
        quotient_correspondences: Vec::new(),
        machines: vec![TerminalMachine {
            closed_reach_application: None,
            declared_service_reach: Vec::new(),
            id: MachineId::new(7).unwrap(),
            attachment: None,
            structural_parameters: Vec::new(),
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            parameters: Vec::new(),
            ranked_scc: None,
            result: TerminalMachineResult::Scalar(declaration(100, format)),
            structural_places: Vec::new(),
            content_entry_claims: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(9).unwrap(),
            blocks: vec![Block {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                structural_parameters: Vec::new(),
                id: BlockId::new(9).unwrap(),
                parameters: Vec::new(),
                operations,
                terminator: Terminator::Return {
                    edge: EdgeId::new(8).unwrap(),
                    value: value(4),
                    cleanup_actions: Vec::new(),
                },
            }],
            contract: MachineContract {
                erased_scalar_formals: Vec::new(),
                erased_proof_formals: Vec::new(),
                id: ContractId::new(7).unwrap(),
                crash_routes: if rule.is_none() {
                    vec![CrashRouteBucket {
                        cause: CrashCause::Trap,
                        alternatives: vec![CrashRouteGuard::Truth],
                    }]
                } else {
                    Vec::new()
                },
                requires: Vec::new(),
                ensures: Vec::new(),
                outcome_specific_ensures: Vec::new(),
            },
        }],
    }
}

fn execution(
    payloads: [IeeeFloatValue; 3],
    rule: Option<FloatSaturationRule>,
) -> TerminalExecution {
    let module = module(payloads, rule);
    terminal_verifier::verify_module(
        &module,
        &terminal_verifier::ProofBundle::default(),
        &proof_admission::AdmissionProfile::default(),
    )
    .unwrap();
    TerminalExecution::start_verified_module(module, &[], &[], &[], &[], &[], None).unwrap()
}

/// Zero, negative zero, subnormal, one, max, negative max, infinities,
/// quiet NaN and negative signaling NaN, retaining their exact payload bits.
fn payloads(format: IeeeFloatFormat) -> [IeeeFloatValue; 10] {
    match format {
        IeeeFloatFormat::Binary32 => [
            0,
            0x8000_0000,
            1,
            0x3f80_0000,
            0x7f7f_ffff,
            0xff7f_ffff,
            0x7f80_0000,
            0xff80_0000,
            0x7fc0_0042,
            0xff80_0042,
        ]
        .map(IeeeFloatValue::Binary32),
        IeeeFloatFormat::Binary64 => [
            0,
            0x8000_0000_0000_0000,
            1,
            0x3ff0_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
            0xffef_ffff_ffff_ffff,
            0x7ff0_0000_0000_0000,
            0xfff0_0000_0000_0000,
            0x7ff8_0000_0000_0042,
            0xfff0_0000_0000_0042,
        ]
        .map(IeeeFloatValue::Binary64),
    }
}

#[test]
fn float_result_policy_saturates_only_finite_operand_overflow() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let values = payloads(format);
        for rule in [
            FloatSaturationRule::FiniteOperandOverflow,
            FloatSaturationRule::FiniteDivisionOverflow,
        ] {
            for (left, right, raw, expected) in [
                (4, 3, 6, 4),
                (5, 3, 7, 5),
                (6, 3, 6, 6),
                (3, 7, 7, 7),
                (8, 3, 6, 6),
                (3, 9, 7, 7),
                (3, 3, 8, 8),
                (3, 3, 9, 9),
                (4, 3, 0, 0),
                (5, 3, 1, 1),
                (4, 3, 2, 2),
                // The finite raw value is intentionally not the operands'
                // sum/product/quotient: the adapter must never recompute it.
                (4, 4, 3, 3),
            ] {
                let mut execution =
                    execution([values[left], values[right], values[raw]], Some(rule));
                let actual = execution
                    .resume(
                        &mut TerminalFuelMeter::unbounded(),
                        &mut AcceptTerminalEffects,
                    )
                    .unwrap();
                assert_eq!(
                    actual,
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                        TerminalScalarValue::IeeeFloat(values[expected])
                    )),
                    "{format:?} {rule:?} {left}/{right}/{raw}"
                );
                assert_eq!(
                    execution.values[&value(3)],
                    TerminalScalarValue::IeeeFloat(values[raw])
                );
            }
        }
        for divisor in [0, 1] {
            for raw in [6, 7, 8, 9] {
                let mut execution = execution(
                    [values[3], values[divisor], values[raw]],
                    Some(FloatSaturationRule::FiniteDivisionOverflow),
                );
                assert_eq!(
                    execution
                        .resume(
                            &mut TerminalFuelMeter::unbounded(),
                            &mut AcceptTerminalEffects
                        )
                        .unwrap(),
                    TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                        TerminalScalarValue::IeeeFloat(values[raw])
                    ))
                );
            }
        }
    }
}

#[test]
fn float_result_policy_trapping_preserves_every_finite_payload() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let values = payloads(format);
        for raw in &values[..6] {
            let mut execution = execution([values[6], values[8], *raw], None);
            assert_eq!(
                execution
                    .resume(
                        &mut TerminalFuelMeter::unbounded(),
                        &mut AcceptTerminalEffects
                    )
                    .unwrap(),
                TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
                    TerminalScalarValue::IeeeFloat(*raw)
                ))
            );
        }
    }
}

#[test]
fn float_result_policy_traps_at_its_own_site_before_publishing_or_advancing() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let values = payloads(format);
        for raw in &values[6..] {
            let mut execution = execution([values[3], values[3], *raw], None);
            // Seed existing frame custody to test the runtime crash report,
            // independently of the verifier's structural claim construction.
            for identity in [17, 12] {
                execution.live_claims.insert(
                    ClaimId::new(identity).unwrap(),
                    LiveClaim {
                        place: None,
                        path: Vec::new(),
                        multiplicity: None,
                    },
                );
            }
            let expected = TerminalExecutionStatus::Crashed(TerminalCrash {
                site: TerminalCrashSite::Operation {
                    machine: MachineId::new(7).unwrap(),
                    block: BlockId::new(9).unwrap(),
                    operation: OperationId::new(4).unwrap(),
                },
                cause: CrashCause::Trap,
                site_guard: Vec::new(),
                frontier_lower_bound: vec![ClaimId::new(12).unwrap(), ClaimId::new(17).unwrap()],
            });
            let mut fuel = TerminalFuelMeter::unbounded();
            for _ in 0..2 {
                assert_eq!(
                    execution
                        .resume(&mut fuel, &mut AcceptTerminalEffects)
                        .unwrap(),
                    expected
                );
                assert!(!execution.values.contains_key(&value(4)));
                assert!(!execution.values.contains_key(&value(5)));
                assert_eq!(execution.next_operation, 3);
                assert_eq!(fuel.usage().total_units(), 4);
                assert_eq!(
                    fuel.usage()
                        .at(terminal_fuel::FuelChargeSite::Operation(
                            OperationId::new(4).unwrap()
                        ))
                        .unwrap()
                        .executions(),
                    1,
                    "resuming a completed trap does not execute or charge the policy again"
                );
            }
        }
    }
}

#[test]
fn float_result_policy_rejects_stale_runtime_operands_and_result_shapes() {
    for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
        let values = payloads(format);
        let other_format = match format {
            IeeeFloatFormat::Binary32 => IeeeFloatValue::Binary64(0),
            IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary32(0),
        };
        for rule in [
            None,
            Some(FloatSaturationRule::FiniteOperandOverflow),
            Some(FloatSaturationRule::FiniteDivisionOverflow),
        ] {
            let operation = policy_operation(format, rule);
            for missing in if rule.is_some() {
                vec![1, 2, 3]
            } else {
                vec![3]
            } {
                for replacement in [
                    None,
                    Some(TerminalScalarValue::Boolean(false)),
                    Some(TerminalScalarValue::IeeeFloat(other_format)),
                    Some(TerminalScalarValue::Integer {
                        scalar_type: IntegerType::new(IntegerSign::Unsigned, 32).unwrap(),
                        value: IntegerValue::Unsigned(0),
                    }),
                ] {
                    let mut execution = execution([values[3]; 3], rule);
                    for identity in 1..=3 {
                        execution
                            .values
                            .insert(value(identity), TerminalScalarValue::IeeeFloat(values[3]));
                    }
                    execution.values.remove(&value(missing));
                    if let Some(replacement) = replacement {
                        execution.values.insert(value(missing), replacement);
                    }
                    let actual = execution.execute_float_result_policy(&operation);
                    match replacement {
                        None => assert!(
                            matches!(actual, Err(TerminalInterpretError::VerifiedValueMissing(missing_value)) if missing_value == value(missing))
                        ),
                        Some(_) => assert!(matches!(
                            actual,
                            Err(TerminalInterpretError::VerifiedOperationMalformed)
                        )),
                    }
                    assert!(execution.crash.is_none());
                    assert!(!execution.values.contains_key(&value(4)));
                }
            }
            for result in [
                OperationResult::Unit,
                OperationResult::Scalar(ValueDeclaration {
                    id: value(4),
                    scalar_type: ScalarType::Boolean,
                    qualifications: Default::default(),
                }),
            ] {
                let mut execution = execution([values[3]; 3], rule);
                let mut malformed = operation.clone();
                malformed.result = result;
                assert!(matches!(
                    execution.execute_float_result_policy(&malformed),
                    Err(TerminalInterpretError::VerifiedOperationMalformed)
                ));
            }
        }
    }
}
