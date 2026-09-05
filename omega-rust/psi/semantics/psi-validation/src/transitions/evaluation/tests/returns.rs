use super::{check_typed, parse};
use psi_typed_trees::{TypedTrees, statement::StatementNode};

fn combine_siblings(typed: &mut TypedTrees) {
    let machine = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap()
        .clone();
    let nodes = typed.machine_states(&machine)[0].statement_nodes;
    let statements = typed.statement_table.statements(nodes);
    let first = statements.len() - 2;
    let StatementNode::Transition(sibling) = &statements[first + 1] else {
        panic!("false transition")
    };
    let continuation = sibling.target;
    let StatementNode::Transition(primary) =
        &mut typed.statement_table.statements_mut(nodes)[first]
    else {
        panic!("true transition")
    };
    primary.continuation = continuation;
    typed.machine_states_mut(&machine)[0].statement_nodes =
        psi_arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
}

#[test]
fn guarded_anonymous_return_lands_once_at_its_declared_destination() {
    for (destination, returned, accepted) in [
        (
            "u8",
            "18446744073709551616 + 7 - 18446744073709551616",
            true,
        ),
        (
            "u8",
            "18446744073709551616 + 256 - 18446744073709551616",
            false,
        ),
        (
            "u8 [0..=5]",
            "18446744073709551616 + 7 - 18446744073709551616",
            false,
        ),
    ] {
        for positive in [false, true] {
            let (first, second) = if positive {
                (returned, "0")
            } else {
                ("0", returned)
            };
            let source = format!(
                "machine run(flag: bool) -> {destination} {{ transition flag {{ true -> ({first}) false -> ({second}) }} }}"
            );
            for combined in [false, true] {
                let mut typed = parse(&source);
                if combined {
                    combine_siblings(&mut typed);
                }
                check_typed(&typed, &source, accepted);
            }
        }
    }
}

#[test]
fn guarded_return_uses_the_selected_arm_range_and_cast_premises() {
    for (source, accepted) in [
        (
            "machine run(value: u8) -> u8 { transition value >= 10 { true -> (0) false -> (value + 1) } }",
            true,
        ),
        (
            "machine run(value: u8) -> u8 { transition value < 10 { true -> (0) false -> (value + 1) } }",
            false,
        ),
        (
            "machine run(value: u16) -> u8 { transition value > 255 { true -> (0) false -> (value as u8) } }",
            true,
        ),
        (
            "machine run(value: u16) -> u8 { transition value <= 255 { true -> (0) false -> (value as u8) } }",
            false,
        ),
        (
            "machine run(value: u8) -> u8 [0..=10] { transition value >= 10 { true -> (0) false -> (value + 1) } }",
            true,
        ),
    ] {
        for combined in [false, true] {
            let mut typed = parse(source);
            if combined {
                combine_siblings(&mut typed);
            }
            check_typed(&typed, source, accepted);
        }
    }
}

#[test]
fn guarded_return_effects_invalidate_only_the_reached_branch_premises() {
    for (source, accepted) in [
        (
            "machine replace(target: &mut u8) -> u8 { target = 255; 0 } machine run(flag: bool) -> u8 { let mut current: u8 = 3; transition flag { true -> (replace(&mut current)) false -> (current + 1) } }",
            true,
        ),
        (
            "machine replace(target: &mut u8) -> u8 { target = 255; 0 } machine run(flag: bool) -> u8 { let mut current: u8 = 3; transition flag { true -> (0) false -> (replace(&mut current) + current) } }",
            false,
        ),
        (
            "machine replace(target: &mut u8) -> bool { target = 255; false } machine run() -> u8 { let mut current: u8 = 3; transition replace(&mut current) { true -> (0) false -> (current + 1) } }",
            false,
        ),
    ] {
        let mut typed = parse(source);
        combine_siblings(&mut typed);
        check_typed(&typed, source, accepted);
    }
}

#[test]
fn guarded_return_width_custody_does_not_bless_an_unrelated_shared_root() {
    use psi_typed_trees::statement::TransitionTargetNode;
    let source = "machine run(flag: bool) -> u8 { transition flag { true -> (18446744073709551616 + 7 - 18446744073709551616) false -> (0) } } machine unrelated() { let value: bool = false; }";
    let mut typed = parse(source);
    assert!(crate::validate_program(&typed).is_ok());
    let run = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run")
        .unwrap();
    let nodes = typed.machine_states(run)[0].statement_nodes;
    let StatementNode::Transition(transition) = &typed.statement_table.statements(nodes)[0] else {
        panic!("guarded value")
    };
    let TransitionTargetNode::Value(expression) =
        typed.statement_table.transition_target(transition.target)
    else {
        panic!("return expression")
    };
    let expression = *expression;
    let unrelated = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "unrelated")
        .unwrap();
    let nodes = typed.machine_states(unrelated)[0].statement_nodes;
    let StatementNode::LocalData(local) = &mut typed.statement_table.statements_mut(nodes)[0]
    else {
        panic!("unrelated Boolean initializer")
    };
    local.initial_value = expression;
    let diagnostics =
        crate::validate_program(&typed).expect_err("shared unrelated root retains its width gate");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("literal")),
        "{diagnostics:#?}"
    );
}
