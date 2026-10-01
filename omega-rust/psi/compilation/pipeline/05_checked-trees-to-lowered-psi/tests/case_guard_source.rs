//! Authored case equality and classifier membership retain distinct source
//! identities even when both select the same closed-case control graph.

use checked_trees::CheckedComposedUnitControlTerminatorPlan;
use checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableStructLiteralField,
};
use checked_trees::statement::{StatementNode, TransitionGuardNode};
use checked_trees_to_lowered_psi::{TerminalMachineSelection, lower_machine};

#[test]
fn explicit_case_guards_replay_value_and_classifier_identity() {
    for (first, second, membership) in [
        ("choice == Choice::First", "choice == Choice::Second", false),
        ("choice in Choice::First", "choice in Choice::Second", true),
    ] {
        let source = format!(
            r#"
            data Choice [copy] {{ case First; case Second; case Fallback; }}
            data Holder {{ value: u64; }}
            data Bag {{ holders: [Holder; 3]; }}
            machine pick(bag: &mut Bag, choice: Choice) -> &mut Holder {{
                transition {{
                    {first} -> &mut bag.holders[0]
                    {second} -> &mut bag.holders[1]
                    _ -> &mut bag.holders[2]
                }}
            }}
        "#
        );
        let checked = crate::front_end::checked_program(&source);
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "pick")
            .expect("source machine");
        let plan_index = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .position(|plan| plan.machine == machine.symbol)
            .expect("closed-case plan");
        let plan = &checked.facts.flow.terminal_unit_effects.composed_machines[plan_index];
        let state_index = plan
            .states
            .iter()
            .position(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .expect("explicit guards retain closed-case dispatch");
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == plan.states[state_index].state)
            .expect("authored state");
        let guard = checked
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| {
                let StatementNode::Transition(transition) = statement else {
                    return None;
                };
                let TransitionGuardNode::When(guard) = transition.guard else {
                    return None;
                };
                Some(unwrapped_guard(&checked, guard))
            })
            .expect("authored guard");
        let lowered = lower_machine(&checked, TerminalMachineSelection::Name("pick"))
            .unwrap_or_else(|error| panic!("{first}: {error:?}"));
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .expect("verified case return");
        for corruption in ["case receipt", "operator", "classifier or payload", "owner"] {
            if membership && corruption == "owner" {
                continue;
            }
            let mut changed = checked.clone();
            if corruption == "case receipt" {
                let CheckedComposedUnitControlTerminatorPlan::ClosedSum { cases, .. } =
                    &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
                        .states[state_index]
                        .terminator
                else {
                    unreachable!()
                };
                cases[0].case_identity = cases[1].case_identity.clone();
            } else if corruption == "operator" || membership {
                let ExpressionNode::Binary(binary) =
                    changed.typed.expression_table.expression_mut(guard)
                else {
                    unreachable!()
                };
                if corruption == "operator" {
                    binary.operator = BinaryOperator::NotEqual;
                } else {
                    std::mem::swap(&mut binary.left, &mut binary.right);
                }
            } else {
                let ExpressionNode::Binary(binary) = checked.expression_table.expression(guard)
                else {
                    unreachable!()
                };
                let literal = [binary.left, binary.right]
                    .into_iter()
                    .find(|operand| {
                        matches!(
                            checked.expression_table.expression(*operand),
                            ExpressionNode::StructLiteral(_)
                        )
                    })
                    .expect("payload-free case value, not a classifier name");
                let fields = changed
                    .typed
                    .expression_table
                    .insert_struct_fields([TableStructLiteralField::default()]);
                let wrong_owner = checked
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == "Holder")
                    .expect("different nominal declaration")
                    .symbol;
                let ExpressionNode::StructLiteral(value) =
                    changed.typed.expression_table.expression_mut(literal)
                else {
                    unreachable!()
                };
                if corruption == "owner" {
                    value.type_symbol = wrong_owner;
                } else {
                    value.fields = fields;
                }
            }
            assert!(
                lower_machine(&changed, TerminalMachineSelection::Name("pick")).is_err(),
                "{first} must reject {corruption}"
            );
        }
    }
}

fn unwrapped_guard(
    checked: &checked_trees::CheckedTrees,
    mut expression: ExpressionHandle,
) -> ExpressionHandle {
    loop {
        let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
            panic!("case guard is a binary expression")
        };
        if binary.operator != BinaryOperator::Equal {
            return expression;
        }
        if matches!(
            checked.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            expression = binary.right;
        } else if matches!(
            checked.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            expression = binary.left;
        } else {
            return expression;
        }
    }
}
