//! Ordinary callers preserve borrowed primitive effects and scalar results.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::{OperationKind, StructuralAccess};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

const SOURCE: &str = r#"
    machine reset(value: &mut u64) -> u64 { value = 0; 7 }
    machine enter(value: &mut u64) {
        let returned: u64 = reset(&mut value);
        value = returned;
    }
"#;

#[test]
fn borrowed_scalar_callee_and_returned_value_reach_the_callers_closure() {
    let checked = checked(SOURCE);
    let artifact = terminal_production::produce_terminal_artifact(&checked, "enter")
        .expect("borrowed scalar callee belongs to the ordinary shared call closure");
    execute(&artifact, &[], 7);
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
    expected: u128,
) {
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).unwrap();
    let caller = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &caller.structural_parameters[0];
    assert_eq!(parameter.access, StructuralAccess::MutableBorrow);
    let calls = caller
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| matches!(operation.kind, OperationKind::CallStructuralScalar { .. }))
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let stores = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), 2);
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_primitive_values(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &profile,
            arguments,
            &[TerminalStructuralValue {
                opaque_identity: 71,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(91),
            }],
        )
        .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..32 {
        let status = execution.resume(&mut meter).unwrap();
        observations.push(execution.structural_primitive_values()[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected status {other:?}"),
        }
    }
    assert!(complete);
    assert!(
        observations.contains(&unsigned(0)),
        "the callee write precedes the caller's result write"
    );
    assert_eq!(observations.last(), Some(&unsigned(expected)));
    for operation in stores
        .into_iter()
        .chain(calls.iter().map(|operation| operation.id))
    {
        assert_eq!(
            meter
                .usage()
                .at(terminal_fuel::FuelChargeSite::Operation(operation))
                .unwrap()
                .executions(),
            1
        );
    }
}

#[test]
fn scalar_parameters_and_write_only_reborrows_keep_their_authored_positions() {
    let checked = checked(
        "machine reset(value: &write u64, returned: u64) -> u64 { value = 0; returned } machine enter(value: &mut u64, returned: u64) { let replacement: u64 = reset(&write value, returned); value = replacement; }",
    );
    let artifact = terminal_production::produce_terminal_artifact(&checked, "enter").unwrap();
    execute(&artifact, &[unsigned(37)], 37);
}

#[test]
fn unrelated_structural_return_bodies_do_not_join_the_selected_call_catalog() {
    let source = format!(
        "{SOURCE} data Unused {{}} machine Unused::reset(value: &mut bool) -> bool {{ value = false; true }}"
    );
    let artifact =
        terminal_production::produce_terminal_artifact(&checked(&source), "enter").unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert_eq!(module.machines.len(), 2);
    assert_eq!(module.structural_types.len(), 1);
    execute(&artifact, &[], 7);
}

#[test]
fn attached_callee_uses_the_shared_catalogs_nested_type_and_field_identities() {
    let source = r#"
        data Payload { number: u64; }
        data Owner { payload: Payload; }
        data Earlier { first: bool; second: u64; }
        machine Owner::reset(value: &mut u64) -> u64 { value = 0; 7 }
        machine Earlier::enter(value: &mut u64) {
            let returned: u64 = Owner::reset(&mut value);
            value = returned;
        }
    "#;
    let checked = checked(source);
    let artifact =
        terminal_production::produce_terminal_artifact(&checked, "Earlier::enter").unwrap();
    execute(&artifact, &[], 7);
}

#[test]
fn borrowed_scalar_call_rejects_missing_duplicated_or_substituted_callee_custody() {
    let original = checked(SOURCE);
    for mutation in 0..5 {
        let mut changed = original.clone();
        let plans = &mut changed
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines;
        assert_eq!(plans.len(), 1);
        match mutation {
            0 => plans.clear(),
            1 => plans.push(plans[0].clone()),
            2 => plans[0].effects.clear(),
            3 => plans[0].return_statement_ordinal = 0,
            4 => {
                plans[0].structural_parameters[0].access =
                    checked_trees::CheckedStructuralAccess::SharedBorrow
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::produce_terminal_artifact(&changed, "enter").is_err(),
            "callee custody mutation {mutation}"
        );
    }
}

#[test]
fn same_typed_borrowed_parameter_cannot_replace_the_authored_actual() {
    let mut checked = checked(
        "machine reset(value: &mut u64) -> u64 { value = 0; 7 } machine enter(first: &mut u64, second: &mut u64) { let returned: u64 = reset(&mut first); }",
    );
    let _ = terminal_production::produce_terminal_artifact(&checked, "enter")
        .expect("original borrowed actual");
    let caller = checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| {
            plan.operations.iter().any(|operation| {
                matches!(
                    operation,
                    checked_trees::CheckedUnitEffectOperationPlan::ScalarCall { .. }
                )
            })
        })
        .unwrap();
    let checked_trees::CheckedUnitEffectOperationPlan::ScalarCall {
        structural_arguments,
        ..
    } = &mut caller.operations[0]
    else {
        panic!("scalar call");
    };
    structural_arguments[0].source =
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 };
    assert!(terminal_production::produce_terminal_artifact(&checked, "enter").is_err());
}

#[test]
fn caller_store_cannot_substitute_a_literal_for_the_returned_value() {
    let mut checked = checked(SOURCE);
    let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
        value: zero, ..
    } = &checked
        .facts
        .flow
        .terminal_structural_scalar_returns
        .machines[0]
        .effects[0]
    else {
        panic!("callee store");
    };
    let zero = zero.clone();
    let caller = &mut checked.facts.flow.terminal_unit_effects.machines[0];
    let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { value, .. } =
        &mut caller.operations[1]
    else {
        panic!("caller store");
    };
    *value = zero;
    assert!(terminal_production::produce_terminal_artifact(&checked, "enter").is_err());
}

#[test]
fn caller_store_roster_rejects_deleted_duplicate_or_stale_assignment_sites() {
    let original = checked(SOURCE);
    for mutation in 0..4 {
        let mut changed = original.clone();
        let operations = &mut changed.facts.flow.terminal_unit_effects.machines[0].operations;
        match mutation {
            0 => {
                operations.remove(1);
            }
            1 => operations.insert(1, operations[1].clone()),
            2 | 3 => {
                let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index,
                    ..
                } = &mut operations[1]
                else {
                    panic!("caller store");
                };
                *statement_index = if mutation == 2 { 0 } else { 99 };
            }
            _ => unreachable!(),
        }
        assert!(
            terminal_production::produce_terminal_artifact(&changed, "enter").is_err(),
            "store roster mutation {mutation}"
        );
    }
}

#[test]
fn an_unused_scalar_result_cannot_erase_its_callees_borrowed_write() {
    let mut checked = checked(
        "machine reset(value: &mut u64) -> u64 { value = 0; 7 } machine enter(value: &mut u64) { let returned: u64 = reset(&mut value); }",
    );
    let _ = terminal_production::produce_terminal_artifact(&checked, "enter")
        .expect("unused result still calls");
    checked.facts.flow.terminal_unit_effects.machines[0]
        .operations
        .remove(0);
    assert!(terminal_production::produce_terminal_artifact(&checked, "enter").is_err());
}
