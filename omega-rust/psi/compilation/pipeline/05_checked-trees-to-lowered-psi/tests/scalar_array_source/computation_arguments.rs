//! Computation-owned array actuals keep exact custody and selected execution.

use super::{
    AdmissionProfile, ExpressionNode, IntegerSign, IntegerType, IntegerValue,
    TerminalExecutionResult, TerminalScalarValue, reject,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
#[path = "operation_body_callees.rs"]
mod operation_body_callees;
use checked_trees::{CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionStatus, interpret_terminal_artifact_measured,
};

fn byte(value: u8) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
        value: IntegerValue::Unsigned(u128::from(value)),
    }
}

fn execute(source: &str, arguments: &[TerminalScalarValue]) -> TerminalExecutionResult {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("selected"),
    )
    .unwrap_or_else(|error| panic!("computation array source must lower: {error:?}\n{source}"));
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    assert_eq!(
        terminal_codec::decode_module(&semantic).unwrap(),
        lowered.semantic_module
    );
    assert_eq!(
        terminal_codec::decode_proof_bundle(&proof).unwrap(),
        lowered.proof_bundle
    );
    drop(lowered);
    drop(checked);
    let profile = AdmissionProfile::default();
    let measured = interpret_terminal_artifact_measured(
        &semantic,
        &proof,
        &profile,
        arguments,
        TerminalStructuralInputs::default(),
        &mut AcceptTerminalEffects,
    )
    .unwrap_or_else(|error| panic!("decoded computation arrays must execute: {error:?}\n{source}"));
    let expected = measured.value();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &profile,
        arguments,
        TerminalStructuralInputs::default(),
    )
    .expect("start independently decoded computation arrays");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for _ in 0..measured.usage().total_units() {
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(expected.clone()),
        "one-unit resumption must retain selected computation array results: {source}",
    );
    assert_eq!(
        meter.usage(),
        measured.usage(),
        "no repeated effects: {source}"
    );
    expected
}

fn assert_array(source: &str, arguments: &[TerminalScalarValue], expected: &[TerminalScalarValue]) {
    let TerminalExecutionResult::ScalarArray(result) = execute(source, arguments) else {
        panic!("expected whole array result for {source}");
    };
    assert_eq!(result.value.elements, expected, "{source}");
}

const WRITE: &str =
    "machine write(destination: &mut u8, value: u8) -> u8 { destination = value; value }";

#[test]
fn computation_array_arguments_execute_customer_and_nested_scalar_calls() {
    for leaf in [
        "answer([7u8, 9u8], value)",
        "identity(answer([7u8, 9u8], value))",
        "answer([identity(answer([7u8, 9u8], value)), 9u8], value)",
        "answer(Sizes::ROW, answer(Sizes::ROW, value))",
    ] {
        assert_array(
            &format!(
                "data Sizes {{}}
                 const Sizes::ROW: [u8; 2] = [7u8, 9u8];
                 machine identity(value: u8) -> u8 {{ value }}
                 machine answer(row: [u8; 2], value: u8) -> u8 {{ value }}
                 machine keep(row: [u8; 2]) -> [u8; 2] {{ row }}
                 machine selected(value: u8) -> [u8; 2] {{ keep([{leaf}, 9u8]) }}"
            ),
            &[byte(42)],
            &[byte(42), byte(9)],
        );
    }
    assert_eq!(
        execute(
            "machine identity(value: u8) -> u8 { value }
             machine answer(row: [u8; 2], value: u8) -> u8 { value }
             machine selected() -> u8 { identity(answer([7u8, 9u8], 42u8)) }",
            &[],
        ),
        TerminalExecutionResult::Scalar(byte(42)),
    );
}

#[test]
fn computation_array_arguments_remain_available_through_scalar_helpers() {
    let helpers = "machine identity(value: u8) -> u8 { value }
         machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine helper(value: u8) -> u8 { identity(answer([7u8, 9u8], value)) }";
    assert_eq!(
        execute(
            &format!("{helpers} machine selected(value: u8) -> u8 {{ helper(value) }}"),
            &[byte(42)],
        ),
        TerminalExecutionResult::Scalar(byte(42)),
    );
    assert_eq!(
        execute(
            &format!(
                "{helpers}
                 machine consume(value: u8) {{}}
                 machine selected(value: u8) {{ consume(helper(value)); }}"
            ),
            &[byte(42)],
        ),
        TerminalExecutionResult::Unit,
    );
    assert_array(
        &format!(
            "{helpers}
             machine keep(row: [u8; 2]) -> [u8; 2] {{ row }}
             machine selected(value: u8) -> [u8; 2] {{ keep([helper(value), 9u8]) }}"
        ),
        &[byte(42)],
        &[byte(42), byte(9)],
    );
}

#[test]
fn computation_array_arguments_execute_only_selected_boolean_branches() {
    for (enabled, expected) in [
        (false, [false, false, false, true]),
        (true, [true, true, true, false]),
    ] {
        assert_array(
            &format!(
                "{WRITE}
                 machine test(row: [u8; 1], value: bool) -> bool {{ value }}
                 machine selected(enabled: bool) -> [bool; 4] {{
                     let mut current: u8 = 0;
                     [enabled && test([write(&mut current, 1u8)], true), current == 1u8,
                      enabled || test([write(&mut current, 2u8)], false), current == 2u8]
                 }}"
            ),
            &[TerminalScalarValue::Boolean(enabled)],
            &expected.map(TerminalScalarValue::Boolean),
        );
    }
}

#[test]
fn computation_array_arguments_reject_wrong_call_formal_type_and_leaf_custody() {
    for empty in [false, true] {
        let (array_type, first, second) = if empty {
            ("[u8; 0]", "[]", "[]")
        } else {
            ("[u8; 2]", "[7u8, 9u8]", "[11u8, 13u8]")
        };
        let original = crate::front_end::checked_program(&format!(
            "machine pick(prefix: u8, first: {array_type}, middle: u8, second: {array_type}, suffix: u8) -> u8 {{ suffix }}
             machine selected() -> [u8; 2] {{
                 [pick(3u8, {first}, 4u8, {second}, 42u8),
                  pick(3u8, {second}, 4u8, {first}, 9u8)]
             }}"
        ));
        checked_trees_to_lowered_psi::lower_machine(
            &original,
            TerminalMachineSelection::Name("selected"),
        )
        .expect("computation constructors lower before custody corruption");
        let plans = &original.facts.values.scalar_computations;
        let calls = plans
            .nodes
            .iter()
            .filter_map(|(handle, node)| match node.kind {
                CheckedScalarComputationKind::Call {
                    source_call,
                    structural_arguments,
                    target_state,
                    ..
                } if structural_arguments.count() == 2 => {
                    Some((handle, source_call, structural_arguments, target_state))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let first_call = calls
            .first()
            .copied()
            .expect("first computation call with array actuals");
        let other_call = calls
            .iter()
            .copied()
            .find(|call| call.1 != first_call.1)
            .expect("independent same-signature call occurrence");
        let arguments = plans.structural_arguments.span(first_call.2).unwrap();
        let (other_expression, other_elements) = match &arguments[1] {
            CheckedScalarComputationStructuralArgument::Array {
                expression,
                elements,
                ..
            } => (*expression, *elements),
            _ => panic!("second formal array constructor"),
        };
        let scalar_type = original
            .machines()
            .iter()
            .flat_map(|machine| original.machine_states(machine))
            .find(|state| state.symbol == first_call.3)
            .unwrap()
            .return_type;
        for mutation in [
            "formal",
            "expression",
            "type",
            "call source",
            "call ordinal",
            "leaf",
            "deleted leaves",
        ] {
            if empty && matches!(mutation, "leaf" | "deleted leaves") {
                continue;
            }
            let mut changed = original.clone();
            let plans = &mut changed.facts.values.scalar_computations;
            match mutation {
                "formal" => {
                    let replacement =
                        plans.structural_arguments.span(first_call.2).unwrap()[1].clone();
                    *plans.structural_arguments.get_mut(first_call.2.start()) = replacement;
                }
                "call source" | "call ordinal" => {
                    let CheckedScalarComputationKind::Call {
                        source_call,
                        call_ordinal,
                        ..
                    } = &mut plans.nodes.get_mut(first_call.0).kind
                    else {
                        unreachable!()
                    };
                    if mutation == "call source" {
                        *source_call = other_call.1;
                    } else {
                        *call_ordinal = u32::MAX;
                    }
                }
                _ => {
                    let CheckedScalarComputationStructuralArgument::Array {
                        expression,
                        type_reference,
                        elements,
                    } = plans.structural_arguments.get_mut(first_call.2.start())
                    else {
                        unreachable!()
                    };
                    match mutation {
                        "expression" => *expression = other_expression,
                        "type" => *type_reference = scalar_type,
                        "leaf" => *elements = other_elements,
                        "deleted leaves" => *elements = Default::default(),
                        _ => unreachable!(),
                    }
                }
            }
            reject(&changed, mutation);
        }
    }
}

#[test]
fn computation_array_constant_projection_keeps_builtin_operator_custody() {
    let mut original = crate::front_end::checked_program(
        "data Sizes {}
         const Sizes::ROWS: [[u8; 2]; 1] = [[7, 9]];
         machine answer(row: [u8; 2], value: u8) -> u8 { value }
         machine selected() -> [u8; 2] { [answer(Sizes::ROWS[0], 42u8), 9u8] }",
    );
    let expression = original
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .find_map(|(_, argument)| match argument {
            CheckedScalarComputationStructuralArgument::Array { expression, .. }
                if matches!(
                    original.expression_table.expression(*expression),
                    ExpressionNode::Indexed(_)
                ) =>
            {
                Some(*expression)
            }
            _ => None,
        })
        .expect("computation array constant projection");
    let existing = original
        .facts
        .operators
        .uses
        .iter()
        .find_map(|(handle, selected)| (selected.expression == expression).then_some(handle));
    let operator = existing.unwrap_or_else(|| {
        original
            .facts
            .operators
            .uses
            .append(checked_trees::CheckedOperatorUseFact {
                expression,
                spelling: language_core::OperatorSpelling::Index,
                status: checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback,
                ..Default::default()
            })
    });
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
    .expect("builtin constant projection lowers before selection corruption");
    for mutation in ["operator spelling", "operator candidate count"] {
        let mut changed = original.clone();
        let selected = changed.facts.operators.uses.get_mut(operator);
        if mutation == "operator spelling" {
            selected.spelling = language_core::OperatorSpelling::Add;
        } else {
            selected.candidate_count += 1;
        }
        reject(&changed, mutation);
    }
}
