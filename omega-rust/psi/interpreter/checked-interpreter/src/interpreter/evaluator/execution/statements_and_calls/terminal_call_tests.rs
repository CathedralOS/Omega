use crate::front_end::typed_program;
use crate::interpreter::evaluator::{Evaluator, Halt};

#[test]
fn machine_heads_and_internal_states_use_resolved_identity() {
    let program = typed_program(
        r#"
        machine mark(count: &mut u64, value: i32) -> i32 {
            count = ((count as u64 in Wrapping) + 1) as u64;
            value
        }
        machine choose(count: &mut u64, selector: u64) -> i32 {
            transition {
                selector == 0 -> mark(count, 20)
                selector == 1 -> local(count)
                _ -> 40
            }
            state local(count: &mut u64) -> i32 { transition { _ -> mark(count, 30) } }
        }
        machine main() -> i32 {
            let mut count: u64 = 0;
            let first: i32 = choose(&mut count, 0);
            let second: i32 = choose(&mut count, 1);
            let third: i32 = choose(&mut count, 2);
            transition first == 20 && second == 30 && third == 40 && count == 2 {
                true -> 70
                _ -> 1
            }
        }
        "#,
    );
    assert!(matches!(
        Evaluator::new(&program, &[]).run_const_machine("main"),
        Ok(70)
    ));
}

#[test]
fn stale_target_identity_does_not_fall_back_to_a_matching_name() {
    let mut program = typed_program(
        "machine main() -> i32 { transition { _ -> done() } state done() -> i32 { 70 } }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "main")
        .unwrap();
    let entry = &program.machine_states(machine)[0];
    let typed_trees::statement::StatementNode::Transition(transition) =
        &program.statement_table.statements(entry.statement_nodes)[0]
    else {
        panic!("entry transfers to done");
    };
    let target = transition.target;
    let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target_mut(target)
    else {
        panic!("named state target");
    };
    path.symbol = Default::default();
    assert!(
        matches!(Evaluator::new(&program, &[]).run_const_machine("main"),
        Err(Halt::Unsupported(message)) if message.contains("no resolved machine or state"))
    );
}
