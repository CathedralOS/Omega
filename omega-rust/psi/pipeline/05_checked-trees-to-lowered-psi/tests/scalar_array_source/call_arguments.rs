//! Array actuals and returned parameters rejoin their exact authored sources.
use super::{
    AdmissionProfile, CheckedTrees, CheckedUnitEffectOperationPlan, ExpressionNode, IntegerSign,
    IntegerType, IntegerValue, StatementNode, TerminalExecutionResult, TerminalScalarValue,
    interpret_terminal_artifact, reject,
};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitEffectMachinePlan, CheckedUnitStructuralArgumentSourcePlan,
};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

fn execute(source: &str, expected: &[u8]) {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("selected"),
    )
    .unwrap_or_else(|error| panic!("source array arguments must lower: {error:?}\n{source}"));
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    assert_eq!(
        terminal_codec::decode_module(&semantic).unwrap(),
        lowered.semantic_module
    );
    let proof =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
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
fn source_array_forwarding_without_scalar_parameters_uses_the_ordinary_signature() {
    let source = "
        machine keep(row: [u8; 1]) -> [u8; 1] { row }
        machine forward(first: [u8; 1], second: [u8; 1]) -> [u8; 1] { keep(second) }
        machine selected() -> [u8; 1] {
            let first: [u8; 1] = [7u8];
            let row: [u8; 1] = [255u8];
            let copied: [u8; 1] = keep(row);
            forward(first, copied)
        }";
    execute(source, &[255]);
    let original = crate::front_end::checked_program(source);
    for mutation in [
        "same typed parameter",
        "borrowed parameter",
        "affine parameter",
        "borrowed actual",
    ] {
        let mut changed = original.clone();
        let forward = plan(&mut changed, "forward");
        match mutation {
            "borrowed parameter" => {
                forward.structural_parameters[1].access = CheckedStructuralAccess::SharedBorrow
            }
            "affine parameter" => {
                forward.structural_parameters[1].multiplicity =
                    language_semantics::Multiplicity::Affine
            }
            _ => {
                let arguments = forward
                    .operations
                    .iter_mut()
                    .find_map(|operation| match operation {
                        CheckedUnitEffectOperationPlan::StructuralCall {
                            structural_arguments,
                            ..
                        } => Some(structural_arguments),
                        _ => None,
                    })
                    .expect("forward retains its ordinary call");
                if mutation == "same typed parameter" {
                    arguments[0].source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 };
                } else {
                    arguments[0].access = CheckedStructuralAccess::SharedBorrow;
                }
            }
        }
        reject(&changed, mutation);
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
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("selected"),
    )
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

fn custody_fixture() -> CheckedTrees {
    crate::front_end::checked_program(&format!(
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
    checked_trees_to_lowered_psi::lower_machine(
        &original,
        TerminalMachineSelection::Name("selected"),
    )
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
