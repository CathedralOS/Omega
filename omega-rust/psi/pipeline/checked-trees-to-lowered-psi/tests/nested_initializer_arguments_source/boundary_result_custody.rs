use super::*;

const SOURCE: &str = r#"
    pub data Token { flag: bool; }
    boundary trait Factory { machine create() -> Token reaches Factory; }
    data Main {}
    machine Main::consume(token: Token) {}
    machine Main::main() reaches Factory {
        let first: Token = Factory::create();
        let second: Token = Factory::create();
        Main::consume(first);
    }
"#;

#[test]
fn boundary_result_move_rejects_a_conflicting_authored_name_head() {
    let original = checked(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&original, "Main::main")
        .expect("valid boundary result move");
    let machine = main_machine(&original);
    let [state] = original.machine_states(machine) else {
        panic!("one source state")
    };
    let [
        StatementNode::LocalData(first),
        StatementNode::LocalData(second),
        StatementNode::Call(call),
    ] = original.statement_table.statements(state.statement_nodes)
    else {
        panic!("two results and one consuming call")
    };
    let [argument] = original.statement_table.expression_handles(call.arguments) else {
        panic!("one owned operand")
    };
    let ExpressionNode::Name(name) = original.expression_table.expression(*argument) else {
        panic!("bare result name")
    };
    assert_eq!(name.symbol, first.symbol);
    assert_eq!(name.head_symbol, first.symbol);
    assert_eq!(
        original
            .expression_table
            .name_path_members(name.members)
            .len(),
        1
    );
    let mut changed = original.clone();
    let ExpressionNode::Name(name) = changed.typed.expression_table.expression_mut(*argument)
    else {
        unreachable!()
    };
    name.head_symbol = second.symbol;
    assert!(
        checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
        "a live terminal symbol cannot hide a different authored root"
    );
}

#[test]
fn boundary_result_moves_reject_same_type_substitution_and_conflicting_cleanup() {
    let original = checked(SOURCE);
    checked_trees_to_lowered_psi::lower_machine(&original, "Main::main")
        .expect("valid boundary result move");
    let machine = main_machine(&original);
    for mutation in 0..3 {
        let mut changed = original.clone();
        let plan = changed
            .facts
            .flow
            .terminal_unit_effects
            .machines
            .iter_mut()
            .find(|plan| plan.machine == machine.symbol)
            .unwrap();
        for operation in &mut plan.operations {
            match operation {
                CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                    result,
                    discard_result_on_return,
                    ..
                } => {
                    if mutation == 0 {
                        *discard_result_on_return = result.binding_ordinal == 0;
                    } else if mutation == 1 && result.binding_ordinal == 0 {
                        *discard_result_on_return = true;
                    } else if mutation == 2 && result.binding_ordinal == 1 {
                        *discard_result_on_return = false;
                    }
                }
                CheckedUnitEffectOperationPlan::CallUnit {
                    structural_arguments,
                    ..
                } if mutation == 0 => {
                    let [argument] = structural_arguments.as_mut_slice() else {
                        unreachable!()
                    };
                    argument.source =
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: 1,
                        };
                }
                _ => {}
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "Main::main").is_err(),
            "same-type substitution or conflicting cleanup mutation {mutation}"
        );
    }
}
