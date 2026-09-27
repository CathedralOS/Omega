use super::check_typed;
use typed_trees::{TypedTrees, statement::StatementNode};

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
        arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
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
                let mut typed = crate::front_end::typed_program(&source);
                if combined {
                    combine_siblings(&mut typed);
                }
                check_typed(&typed, &source, accepted);
            }
        }
    }
}
