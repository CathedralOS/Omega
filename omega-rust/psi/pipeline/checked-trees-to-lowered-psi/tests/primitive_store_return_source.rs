//! Primitive-reference effects complete before their scalar result exists.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::OperationKind;
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

#[test]
fn primitive_reference_write_then_scalar_return_reaches_terminal() {
    let checked = checked("machine reset(value: &mut u64) -> u64 { value = 0; 0 }");
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "reset")
        .produce_artifact()
        .expect("the operand callee must execute its store before returning");
    let zero = unsigned(0);
    execute(&artifact, &[], unsigned(91), zero, zero);
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
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 91,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[initial],
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    assert!(matches!(
        execution.resume(&mut meter).unwrap(),
        TerminalExecutionStatus::SponsorExhausted(_)
    ));
    assert_eq!(execution.structural_primitive_values(), vec![initial]);
    let mut completed = false;
    for _ in 0..16 {
        meter.replenish(1).unwrap();
        match execution.resume(&mut meter).unwrap() {
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
fn write_only_parameter_delivery_and_distinct_result_keep_dense_scalar_order() {
    let checked = checked(
        "machine replace(destination: &write u64, replacement: u64, result_value: u64) -> u64 { destination = replacement; result_value }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "replace")
        .produce_artifact()
        .unwrap();
    execute(
        &artifact,
        &[unsigned(27), unsigned(43)],
        unsigned(91),
        unsigned(27),
        unsigned(43),
    );
}

#[test]
fn boolean_store_and_return_keep_separate_values() {
    let checked = checked(
        "machine replace(destination: &mut bool, replacement: bool) -> bool { destination = replacement; false }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "replace")
        .produce_artifact()
        .unwrap();
    execute(
        &artifact,
        &[TerminalScalarValue::Boolean(true)],
        TerminalScalarValue::Boolean(false),
        TerminalScalarValue::Boolean(true),
        TerminalScalarValue::Boolean(false),
    );
}

#[test]
fn attached_store_return_retains_its_exact_owner() {
    let mut checked = checked(
        "data First {} data Second {} machine First::reset(value: &mut u64) -> u64 { value = 0; 0 } machine Second::reset(value: &mut u64) -> u64 { value = 0; 0 }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "First::reset")
        .produce_artifact()
        .unwrap();
    execute(&artifact, &[], unsigned(91), unsigned(0), unsigned(0));
    let plans = &mut checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines;
    assert_eq!(plans.len(), 2);
    plans[0].attachment_type_identity = plans[1].attachment_type_identity.clone();
    assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "First::reset").is_err());
}

#[test]
fn missing_duplicate_redirected_and_changed_store_plans_reject() {
    for mutation in 0..8 {
        let mut checked = checked("machine reset(value: &mut u64) -> u64 { value = 0; 0 }");
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
                *value = checked_trees::CheckedScalarExpression::Parameter {
                    position: 0,
                    primitive_type: checked_trees::types::PrimitiveType::U64,
                };
            }
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "reset").is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn unsupported_authored_contracts_cannot_disappear_from_store_return_bodies() {
    for contract in ["requires true;", "ensures result == 0;", "crashes Trap"] {
        let mut checked = checked(&format!(
            "machine reset(value: &mut u64) -> u64 {contract} {{ value = 0; 0 }}"
        ));
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .machines
                .is_empty(),
            "{contract}"
        );
        assert!(checked_trees_to_lowered_psi::lower_machine(&checked, "reset").is_err());
        // Producer evidence may be incomplete or substituted. The consumer must
        // inspect the authored contract even after its proof rows disappear.
        let plain = self::checked("machine reset(value: &mut u64) -> u64 { value = 0; 0 }");
        let mut forged = plain.facts.flow.terminal_structural_scalar_returns.machines[0].clone();
        let machine = &checked.machines()[0];
        forged.machine = machine.symbol;
        forged.state = checked.machine_states(machine)[0].symbol;
        checked.facts.proof.contract_facts = Default::default();
        checked.facts.flow.terminal_structural_scalar_returns =
            plain.facts.flow.terminal_structural_scalar_returns.clone();
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines = vec![forged];
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "reset").is_err(),
            "forged {contract}"
        );
    }
}

#[test]
fn constrained_referents_scalar_inputs_and_results_cannot_lose_their_ranges() {
    for (referent, input, result) in [
        ("u64 [0..=5]", "u64", "u64"),
        ("u64", "u64 [0..=5]", "u64"),
        ("u64", "u64", "u64 [0..=5]"),
    ] {
        let mut checked = checked(&format!(
            "machine reset(value: &mut {referent}, seed: {input}) -> {result} {{ value = 0; 0 }}"
        ));
        assert!(
            checked
                .facts
                .flow
                .terminal_structural_scalar_returns
                .machines
                .is_empty()
        );
        let plain =
            self::checked("machine reset(value: &mut u64, seed: u64) -> u64 { value = 0; 0 }");
        let mut forged = plain.facts.flow.terminal_structural_scalar_returns.machines[0].clone();
        let machine = &checked.machines()[0];
        forged.machine = machine.symbol;
        forged.state = checked.machine_states(machine)[0].symbol;
        checked.facts.proof.contract_facts = Default::default();
        checked.facts.flow.terminal_structural_scalar_returns =
            plain.facts.flow.terminal_structural_scalar_returns.clone();
        checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines = vec![forged];
        let error = checked_trees_to_lowered_psi::lower_machine(&checked, "reset").unwrap_err();
        assert!(
            format!("{error:?}").contains("unsupported constrained types"),
            "{error:?}"
        );
    }
}

#[test]
fn ordinary_store_completion_replays_retained_effects_without_legacy_return_rows() {
    let mut original = checked("machine reset(value: &mut u64) -> u64 { value = 0; 0 }");
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
    let artifact = terminal_production::TerminalProductionRequest::new(&original, "reset")
        .produce_artifact()
        .expect("ordinary operation body independently retains the store and scalar result");
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
                        *value = checked_trees::CheckedScalarExpression::IntegerLiteral {
                            literal: numerics::literals::IntegerLiteral::from_value(9),
                        };
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "reset")
                .produce_artifact()
                .is_err(),
            "{mutation}"
        );
    }
}
