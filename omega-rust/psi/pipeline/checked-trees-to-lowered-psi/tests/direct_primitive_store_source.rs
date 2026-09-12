//! Primitive replacement completes scalar evaluation before its ordered store.

use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralPrimitiveValue, TerminalStructuralValue,
};
use terminal_psi::OperationKind;

fn signed(value: i128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        value: IntegerValue::Signed(value),
    }
}

#[test]
fn computed_integer_replacement_executes_before_its_store() {
    execute(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write i32, replacement: i32) {
            destination = replacement ^ 1i32;
        }
    "#,
        &[signed(6)],
        &[signed(91), signed(7)],
    );
}

#[test]
fn computed_boolean_replacement_executes_before_its_store() {
    execute(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write bool) { destination = !true; }
    "#,
        &[],
        &[
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ],
    );
}

#[test]
fn multiple_primitive_stores_preserve_order_across_suspension() {
    execute(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write i32) {
            destination = 2;
            destination = 3;
        }
    "#,
        &[],
        &[signed(91), signed(2), signed(3)],
    );
}

#[test]
fn primitive_assignment_evaluates_selected_match_call_before_mutation() {
    for choose_first in [false, true] {
        execute(
            r#"
            data Sink {}
            machine value(input: i32) -> i32 { input }
            machine Sink::fill(destination: &write i32, choose_first: bool) {
                destination = match choose_first { true -> value(7), false -> value(9) };
            }
            "#,
            &[TerminalScalarValue::Boolean(choose_first)],
            &[signed(91), signed(if choose_first { 7 } else { 9 })],
        );
    }
}

#[test]
fn primitive_assignment_expands_short_circuit_value_before_mutation() {
    execute(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write bool, choose: bool) {
            destination = choose && false;
        }
        "#,
        &[TerminalScalarValue::Boolean(true)],
        &[
            TerminalScalarValue::Boolean(true),
            TerminalScalarValue::Boolean(false),
        ],
    );
}

#[test]
fn computed_primitive_store_rejects_replaced_roots_and_literal_meaning() {
    use checked_trees::{
        CheckedCallScalarArgument, CheckedScalarComputationKind, CheckedScalarExpression,
    };
    let original = checked_source(
        r#"
        data Sink {}
        machine Sink::fill(destination: &write i32, choose: bool) {
            destination = match choose { true -> 7, false -> 9 };
        }
    "#,
    );
    let _original_artifact =
        terminal_production::TerminalProductionRequest::new(&original, "Sink::fill")
            .produce_artifact()
            .expect("original computed assignment");
    for mutation in 0..3 {
        let mut changed = original.clone();
        if mutation == 0 {
            let operation =
                changed
                    .facts
                    .flow
                    .terminal_unit_effects
                    .machines
                    .iter_mut()
                    .flat_map(|plan| &mut plan.operations)
                    .find(|operation| {
                        matches!(operation,
                    checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { .. })
                    })
                    .unwrap();
            let checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                value,
                ..
            } = operation
            else {
                unreachable!()
            };
            *value = CheckedCallScalarArgument::Pure(CheckedScalarExpression::IntegerLiteral {
                literal: numerics::literals::IntegerLiteral::from_value(7),
            });
        } else if mutation == 1 {
            let (handle, _) = changed
                .facts
                .values
                .scalar_computations
                .roots
                .iter()
                .next()
                .unwrap();
            changed
                .facts
                .values
                .scalar_computations
                .roots
                .get_mut(handle)
                .machine = symbols::SymbolHandle::invalid();
        } else {
            let handle = changed
                .facts
                .values
                .scalar_computations
                .nodes
                .iter()
                .find_map(|(handle, node)| {
                    matches!(
                        node.kind,
                        CheckedScalarComputationKind::Value(
                            CheckedScalarExpression::IntegerLiteral { .. }
                        )
                    )
                    .then_some(handle)
                })
                .unwrap();
            let CheckedScalarComputationKind::Value(CheckedScalarExpression::IntegerLiteral {
                literal,
                ..
            }) = &mut changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(handle)
                .kind
            else {
                unreachable!()
            };
            *literal = numerics::literals::IntegerLiteral::from_value(42);
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "Sink::fill")
                .produce_artifact()
                .is_err(),
            "store mutation {mutation}"
        );
    }
}

fn checked_source(source: &str) -> checked_trees::CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let resolved =
        syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).expect("resolve");
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed).expect("check")
}

fn execute(source: &str, arguments: &[TerminalScalarValue], expected: &[TerminalScalarValue]) {
    let checked = checked_source(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Sink::fill")
        .produce_artifact()
        .expect("publish exact store sequence");
    let module =
        terminal_codec::decode_module(artifact.semantic_bytes()).expect("reload semantics");
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).expect("reload proof");
    let profile = proof_admission::AdmissionProfile::default();
    terminal_verifier::verify_module(&module, &proof, &profile).expect("verify reloaded artifact");
    let machine = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let parameter = &machine.structural_parameters[0];
    let stores = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                OperationKind::WriteOnlyPrimitiveStore { .. }
            )
        })
        .map(|operation| operation.id)
        .collect::<Vec<_>>();
    assert_eq!(stores.len(), expected.len() - 1);
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
            &[TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: expected[0],
            }],
        )
        .expect("start with original referent");
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(0);
    let mut observations = Vec::new();
    let mut complete = false;
    for _ in 0..64 {
        let status = execution
            .resume(&mut meter)
            .expect("execute store sequence");
        observations.push(execution.structural_primitive_values()[0].value);
        match status {
            TerminalExecutionStatus::Complete(result) => {
                assert_eq!(result, TerminalExecutionResult::Unit);
                complete = true;
                break;
            }
            TerminalExecutionStatus::SponsorExhausted(_) => meter.replenish(1).unwrap(),
            other => panic!("unexpected execution status {other:?}"),
        }
    }
    assert!(complete, "bounded direct stores must complete");
    observations.dedup();
    assert_eq!(
        observations, expected,
        "caller-visible writes preserve source order"
    );
    for store in stores {
        let usage = meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(store))
            .unwrap();
        assert_eq!(usage.executions(), 1, "committed store must not replay");
        assert_eq!(usage.units(), 1);
    }
}
