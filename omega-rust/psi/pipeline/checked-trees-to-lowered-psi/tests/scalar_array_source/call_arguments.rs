//! Array actuals and returned parameters rejoin their exact authored sources.

use super::*;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectMachinePlan, CheckedUnitStructuralArgumentSourcePlan,
};

fn execute(source: &str, expected: &[u8]) {
    let checked = checked_source(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
        .unwrap_or_else(|error| panic!("source array arguments must lower: {error:?}\n{source}"));
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    assert_eq!(
        terminal_codec::decode_module(&semantic).unwrap(),
        lowered.semantic_module
    );
    let proof = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let TerminalExecutionResult::ScalarArray(result) =
        interpret_terminal_artifact(&semantic, &proof, &AdmissionProfile::default(), &[])
            .unwrap_or_else(|error| {
                panic!("decoded source calls must execute: {error:?}\n{source}")
            })
    else {
        panic!("source call must return its array payload");
    };
    assert_eq!(
        result.value.elements,
        expected
            .iter()
            .map(|value| TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                value: IntegerValue::Unsigned(u128::from(*value)),
            })
            .collect::<Vec<_>>()
    );
}

const HELPERS: &str = "
    machine make(value: u8) -> [u8; 2] { [value, 9] }
    machine keep(row: [u8; 2]) -> [u8; 2] { row }
    machine scalar(value: u8) -> u8 { value }
    machine pick(prefix: u8, first: [u8; 2], middle: u8, second: [u8; 2]) -> [u8; 2] { second }
";

#[test]
fn source_scalar_return_call_accepts_whole_array_arguments() {
    for (array_type, initializer) in [("[u8; 2]", "[7, 9]"), ("[u8; 0]", "[]")] {
        execute(
            &format!(
                "machine answer(row: {array_type}, value: u8) -> u8 {{ value }}
             machine selected() -> [u8; 2] {{
                 let row: {array_type} = {initializer};
                 let result: u8 = answer(row, 42u8);
                 [result, 9]
             }}"
            ),
            &[42, 9],
        );
    }
}

#[test]
fn source_mixed_array_call_requires_exact_scalar_positions_and_complete_obligations() {
    let source = "
        machine pick(first: [u8; 2], prefix: u8, second: [u8; 2], suffix: u8)
            -> [u8; 2] requires suffix != 0u8 { second }
        machine selected() -> [u8; 2] {
            let first: [u8; 2] = [7, 9];
            let second: [u8; 2] = [42, 9];
            pick(first, 0u8, second, 3u8)
        }";
    execute(source, &[42, 9]);
    let checked = checked_source(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "selected")
        .expect("mixed call requirements prove before corruption");
    for mutation in ["missing requirement", "swapped scalar arguments"] {
        let mut module = lowered.semantic_module.clone();
        let entry = module
            .machines
            .iter_mut()
            .find(|machine| machine.id == module.entry)
            .unwrap();
        let (arguments, obligations) = entry
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .find_map(|operation| match &mut operation.kind {
                terminal_psi::OperationKind::CallStructuralWithScalarArguments {
                    arguments,
                    requirement_obligations,
                    ..
                } if !requirement_obligations.is_empty() => {
                    Some((arguments, requirement_obligations))
                }
                _ => None,
            })
            .expect("array call retains its callee requirement slot");
        assert_eq!(
            arguments.len(),
            2,
            "authored positions one and three form dense scalar actuals"
        );
        match mutation {
            "missing requirement" => obligations.clear(),
            "swapped scalar arguments" => arguments.swap(0, 1),
            _ => unreachable!(),
        }
        assert!(
            terminal_verifier::verify_module(
                &module,
                &lowered.proof_bundle,
                &AdmissionProfile::default()
            )
            .is_err(),
            "{mutation} cannot retain the mixed call certificate"
        );
    }
}

#[test]
fn source_array_arguments_compose_named_nested_mixed_and_repeated_uses() {
    for body in [
        "let row: [u8; 2] = [42, 9]; keep(row)",
        "let row: [u8; 2] = make(42u8); keep(row)",
        "keep(make(42u8))",
        "keep(keep(make(42u8)))",
        "let row: [u8; 2] = make(42u8); let saved: [u8; 2] = keep(row); keep(row)",
        "let row: [u8; 2] = make(42u8); pick(3u8, row, 4u8, row)",
        "pick(3u8, make(7u8), 4u8, make(42u8))",
        "pick(scalar(3u8), make(7u8), scalar(4u8), make(42u8))",
        "let first: [u8; 2] = make(7u8); let second: [u8; 2] = make(42u8); pick(3u8, first, 4u8, second)",
    ] {
        execute(
            &format!("{HELPERS} machine selected() -> [u8; 2] {{ {body} }}"),
            &[42, 9],
        );
    }
    execute(
        &format!(
            "{HELPERS} machine selected() -> [u8; 2] {{ pick(3u8, make(42u8), 4u8, make(7u8)) }}"
        ),
        &[7, 9],
    );
}

#[test]
fn source_array_arguments_preserve_nested_and_zero_dimensions() {
    for (array_type, value, expected) in [
        ("[[u8; 2]; 2]", "[[7, 9], [11, 13]]", vec![7, 9, 11, 13]),
        ("[u8; 0]", "[]", vec![]),
        ("[[u8; 0]; 1]", "[[]]", vec![]),
        ("[[u8; 2]; 0]", "[]", vec![]),
    ] {
        execute(
            &format!(
                "machine make() -> {array_type} {{ {value} }}
             machine keep(row: {array_type}) -> {array_type} {{ row }}
             machine selected() -> {array_type} {{
                 let row: {array_type} = make();
                 let saved: {array_type} = keep(row);
                 keep(keep(saved))
             }}"
            ),
            &expected,
        );
    }
}

fn custody_fixture() -> CheckedTrees {
    checked_source(&format!(
        "{HELPERS} machine selected() -> [u8; 2] {{
             let first: [u8; 2] = make(7u8);
             let second: [u8; 2] = make(42u8);
             pick(3u8, first, 4u8, second)
         }}"
    ))
}

fn plan<'checked>(
    checked: &'checked mut CheckedTrees,
    name: &str,
) -> &'checked mut CheckedUnitEffectMachinePlan {
    let symbol = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap()
        .symbol;
    checked
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|machine| machine.machine == symbol)
        .unwrap()
}

#[test]
fn source_array_parameter_return_rejects_slot_symbol_type_and_access_substitution() {
    let original = custody_fixture();
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("parameter return and mixed positions lower before corruption");
    for mutation in [
        "returned slot",
        "unknown slot",
        "return type",
        "parameter access",
        "parameter position",
        "source symbol",
    ] {
        let mut changed = original.clone();
        if mutation == "source symbol" {
            let machine = changed
                .typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "pick")
                .unwrap();
            let state = &changed.typed.machine_states(machine)[0];
            let first_symbol = changed.typed.state_parameters(state)[1].symbol;
            let StatementNode::Expression(expression) = changed
                .typed
                .statement_table
                .statements(state.statement_nodes)
                .last()
                .unwrap()
            else {
                unreachable!()
            };
            let expression = *expression;
            let ExpressionNode::Name(path) =
                changed.typed.expression_table.expression_mut(expression)
            else {
                unreachable!()
            };
            path.symbol = first_symbol;
        } else {
            let plan = plan(&mut changed, "pick");
            let result = plan.structural_result.as_mut().unwrap();
            assert_eq!(
                result.source,
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
            );
            match mutation {
                "returned slot" => {
                    result.source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
                }
                "unknown slot" => {
                    result.source = CheckedUnitStructuralArgumentSourcePlan::Parameter {
                        parameter_index: u32::MAX,
                    }
                }
                "return type" => result.type_identity = "[u16; 2]".into(),
                "parameter access" => {
                    plan.structural_parameters[1].access = CheckedStructuralAccess::SharedBorrow
                }
                "parameter position" => plan.structural_parameters[1].position = 1,
                _ => unreachable!(),
            }
        }
        reject(&changed, mutation);
    }
}

#[test]
fn source_array_direct_parameter_return_rejects_changed_parameter_flags() {
    let original = checked_source("machine selected(row: [u8; 2]) -> [u8; 2] { row }");
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("direct array parameter return lowers before source flag corruption");
    let machine = original
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "selected")
        .unwrap();
    let parameters = original.typed.machine_states(machine)[0].parameters;
    for mutation in ["constant source", "mutable source", "self source"] {
        let mut changed = original.clone();
        let parameter = &mut changed.typed.state_parameters.span_mut_or_empty(parameters)[0];
        match mutation {
            "constant source" => parameter.is_const = true,
            "mutable source" => parameter.is_mutable = true,
            "self source" => parameter.is_self = true,
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn source_array_local_return_requires_the_exact_whole_name() {
    let original =
        checked_source("machine selected() -> [u8; 2] { let row: [u8; 2] = [7, 9]; row }");
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("whole local array return lowers before source path corruption");
    let (_, expressions) = selected_source(&original);
    let expression = *expressions.last().unwrap();
    for mutation in ["changed head symbol", "missing name member"] {
        let mut changed = original.clone();
        let ExpressionNode::Name(path) = changed.typed.expression_table.expression_mut(expression)
        else {
            panic!("whole returned local name")
        };
        match mutation {
            "changed head symbol" => path.head_symbol = symbols::SymbolHandle::invalid(),
            "missing name member" => path.members = Default::default(),
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn source_array_arguments_reject_same_typed_binding_and_authored_operand_substitution() {
    let original = custody_fixture();
    checked_trees_to_lowered_psi::lower_machine(&original, "selected")
        .expect("array actual custody lowers before corruption");
    for mutation in [
        "swapped arguments",
        "duplicate binding",
        "unknown binding",
        "borrowed actual",
        "authored operand",
    ] {
        let mut changed = original.clone();
        if mutation == "authored operand" {
            let (coordinate, source_site) = plan(&mut changed, "selected")
                .operations
                .iter()
                .find_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        coordinate,
                        source_site,
                        structural_arguments,
                        ..
                    } if structural_arguments.len() == 2 => Some((*coordinate, *source_site)),
                    _ => None,
                })
                .expect("picked call retains its source occurrence");
            let machine = changed
                .typed
                .machines()
                .iter()
                .find(|machine| machine.name.as_str() == "selected")
                .unwrap();
            let state = &changed.typed.machine_states(machine)[0];
            let statement = &changed
                .typed
                .statement_table
                .statements(state.statement_nodes)[coordinate.statement_index as usize];
            // Normalization may bind the returned call to a local before its
            // final name use. The call coordinate still owns its initializer.
            let mut expression = match statement {
                StatementNode::LocalData(local) => local.initial_value,
                StatementNode::Expression(expression) => *expression,
                other => panic!("retained array call has no initializer: {other:?}"),
            };
            let call = loop {
                match changed.typed.expression_table.expression(expression) {
                    ExpressionNode::Call(call) => break call,
                    ExpressionNode::Cast(cast) => expression = cast.value,
                    ExpressionNode::Atomic(atomic) => expression = atomic.value,
                    other => {
                        panic!("expected authored array call under return wrappers, got {other:?}")
                    }
                }
            };
            assert_eq!(
                source_site,
                Some(checked_trees::NominalMachineUseSite::Expression(expression)),
                "mutation follows the exact captured call rather than the normalized return name",
            );
            let arguments = changed
                .typed
                .expression_table
                .expression_handles(call.arguments);
            let (first, second) = (arguments[1], arguments[3]);
            let replacement = changed.typed.expression_table.expression(first).clone();
            *changed.typed.expression_table.expression_mut(second) = replacement;
        } else {
            let plan = plan(&mut changed, "selected");
            let arguments = plan
                .operations
                .iter_mut()
                .rev()
                .find_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::StructuralCall {
                        structural_arguments,
                        ..
                    } if structural_arguments.len() == 2 => Some(structural_arguments),
                    _ => None,
                })
                .expect("mixed call owns two dense structural actuals");
            match mutation {
                "swapped arguments" => arguments.swap(0, 1),
                "duplicate binding" => arguments[1].source = arguments[0].source.clone(),
                "unknown binding" => {
                    arguments[1].source =
                        CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: u32::MAX,
                        }
                }
                "borrowed actual" => arguments[1].access = CheckedStructuralAccess::SharedBorrow,
                _ => unreachable!(),
            }
        }
        reject(&changed, mutation);
    }
}
