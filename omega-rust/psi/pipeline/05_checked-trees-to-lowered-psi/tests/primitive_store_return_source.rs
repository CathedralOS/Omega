//! Primitive-reference effects complete before their scalar result exists.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::OperationKind;

#[test]
fn primitive_reference_reads_under_operators_preserve_pre_store_values() {
    let checked = crate::front_end::checked_program(
        "machine change(value: &mut u64, mask: u64) -> u64 { value = value ^ mask; value }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("change"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    execute(
        &artifact,
        &[unsigned(0x81)],
        unsigned(u64::MAX.into()),
        unsigned((u64::MAX ^ 0x81).into()),
        unsigned((u64::MAX ^ 0x81).into()),
    );
}

fn unsigned(value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn execute(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    arguments: &[TerminalScalarValue],
    initial: TerminalScalarValue,
    written: TerminalScalarValue,
    result: TerminalScalarValue,
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    assert_eq!(
        terminal_codec::encode_module(&module).unwrap(),
        artifact.semantic_bytes()
    );
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &machine.structural_parameters[0];
    let store = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .unwrap();
    let initial = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: initial,
    };
    let written = TerminalStructuralPrimitiveValue {
        argument_index: 0,
        value: written,
    };
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &profile,
        arguments,
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 91,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            primitive_values: &[initial],
            ..Default::default()
        },
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    let mut completed = false;
    for _ in 0..16 {
        meter.replenish(1).unwrap();
        match execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap()
        {
            TerminalExecutionStatus::Complete(actual) => {
                assert_eq!(actual, TerminalExecutionResult::Scalar(result));
                completed = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => {}
            other => panic!("unexpected execution status: {other:?}"),
        }
    }
    assert!(completed, "bounded store and return must finish");
    assert_eq!(execution.structural_primitive_values(), vec![written]);
    assert_eq!(
        meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(store.id))
            .unwrap()
            .executions(),
        1
    );
}

#[test]
fn attached_store_return_retains_its_exact_owner() {
    let mut checked = crate::front_end::checked_program(
        "data First {} data Second {} machine First::reset(value: &mut u64) -> u64 { value = 0; 0 } machine Second::reset(value: &mut u64) -> u64 { value = 0; 0 }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("First::reset"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    execute(&artifact, &[], unsigned(91), unsigned(0), unsigned(0));
    let plans = &mut checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines;
    assert_eq!(plans.len(), 2);
    plans[0].attachment_type_identity = plans[1].attachment_type_identity.clone();
    assert!(
        checked_trees_to_lowered_psi::lower_machine(
            &checked,
            TerminalMachineSelection::Name("First::reset")
        )
        .is_err()
    );
}

#[test]
fn missing_duplicate_redirected_and_changed_store_plans_reject() {
    for mutation in 0..8 {
        let mut checked = crate::front_end::checked_program(
            "machine reset(value: &mut u64) -> u64 { value = 0; 0 }",
        );
        let plan = checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter_mut()
            .find(|plan| !plan.effects.is_empty())
            .expect("store plan");
        match mutation {
            0 => plan.effects.clear(),
            1 => plan.effects.push(plan.effects[0].clone()),
            2 | 3 => {
                let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index,
                    destination,
                    ..
                } = &mut plan.effects[0]
                else {
                    panic!("store")
                };
                if mutation == 2 {
                    *statement_index = 1;
                } else {
                    *destination = checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                        parameter_index: 1,
                    };
                }
            }
            4 => plan.return_statement_ordinal = 0,
            5 => {
                plan.structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            6 => plan
                .cleanup_actions
                .push(checked_trees::CheckedStructuralScalarReturnCleanupAction::DiscardRoot(0)),
            7 => {
                let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    value,
                    ..
                } = &mut plan.effects[0]
                else {
                    panic!("store")
                };
                *value = checked_trees::CheckedCallScalarArgument::Pure(
                    checked_trees::CheckedScalarExpression::Parameter {
                        position: 0,
                        primitive_type: checked_trees::types::PrimitiveType::U64,
                    },
                );
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &checked,
                TerminalMachineSelection::Name("reset")
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn ordinary_store_completion_replays_retained_effects_without_legacy_return_rows() {
    let mut original =
        crate::front_end::checked_program("machine reset(value: &mut u64) -> u64 { value = 0; 0 }");
    let target = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "reset")
        .unwrap()
        .symbol;
    original
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines
        .retain(|plan| plan.machine != target);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &original,
        TerminalMachineSelection::Name("reset"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("ordinary operation body independently retains the store and scalar result")
    .into_artifact();
    execute(&artifact, &[], unsigned(91), unsigned(0), unsigned(0));
    for mutation in [
        "missing body",
        "duplicate body",
        "missing store",
        "duplicate store",
        "destination",
        "access",
        "value",
        "completion",
        "order",
    ] {
        let mut changed = original.clone();
        let plans = &mut changed.facts.flow.terminal_unit_effects.machines;
        let position = plans
            .iter()
            .position(|plan| plan.machine == target)
            .unwrap();
        if mutation == "missing body" {
            plans.remove(position);
        } else if mutation == "duplicate body" {
            plans.push(plans[position].clone());
        } else {
            let plan = &mut plans[position];
            let store =
                plan.operations
                    .iter()
                    .position(|operation| {
                        matches!(operation,
                checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. })
                    })
                    .unwrap();
            match mutation {
                "missing store" => {
                    plan.operations.remove(store);
                }
                "duplicate store" => {
                    plan.operations
                        .insert(store, plan.operations[store].clone());
                }
                "access" => {
                    plan.structural_parameters[0].access =
                        checked_trees::CheckedStructuralAccess::SharedBorrow
                }
                "completion" => plan.scalar_result.as_mut().unwrap().statement_index = 0,
                "order" => plan.operations.swap(store, store + 1),
                "destination" | "value" => {
                    let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                        destination,
                        value,
                        ..
                    } = &mut plan.operations[store]
                    else {
                        unreachable!();
                    };
                    if mutation == "destination" {
                        *destination = checked_trees::CheckedPrimitiveStoreDestination::Parameter {
                            parameter_index: 1,
                        };
                    } else {
                        *value = checked_trees::CheckedCallScalarArgument::Pure(
                            checked_trees::CheckedScalarExpression::IntegerLiteral {
                                literal: numerics::literals::IntegerLiteral::from_value(9),
                            },
                        );
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(
                &changed,
                TerminalMachineSelection::Name("reset")
            )
            .produce(TerminalProductionCustody::artifact_only(
                &mut TerminalProductionTimings::default()
            ))
            .is_err(),
            "{mutation}"
        );
    }
}
