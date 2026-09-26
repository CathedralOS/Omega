//! A dispatch arm whose guard is closed and true, ending its run, is the run's
//! `_`: `transition true { true -> done(v) }` lowers to one unconditional
//! transition. An earlier closed-true arm keeps its guard, since arms follow
//! it, and a guard that reads a place is never decided here.
use typed_trees::TypedTrees;
use typed_trees::statement::{StatementNode, TransitionGuardNode};

fn guards(program: &TypedTrees, machine_name: &str) -> Vec<bool> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .expect("machine by name");
    let entry = program
        .machine_states(machine)
        .first()
        .expect("entry state");
    program
        .statement_table
        .statements(entry.statement_nodes)
        .iter()
        .filter_map(|statement| match statement {
            StatementNode::Transition(transition) => {
                Some(transition.guard == TransitionGuardNode::Always)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn a_run_closing_closed_true_arm_is_unconditional() {
    let program = crate::front_end::typed_program_result(
        "machine single(v: u64) -> u64 {
            transition true { true -> done(v) }
            state done(v: u64) -> u64 { v }
        }
        machine literal(v: u64) -> u64 {
            transition { true -> done(v) }
            state done(v: u64) -> u64 { v }
        }
        machine followed(v: u64) -> u64 {
            transition true {
                true -> done(v)
                false -> done(0)
            }
            state done(v: u64) -> u64 { v }
        }
        machine read(v: u64, flag: bool) -> u64 {
            transition flag {
                true -> done(v)
                false -> done(0)
            }
            state done(v: u64) -> u64 { v }
        }",
    )
    .expect("lowering succeeds");
    assert_eq!(guards(&program, "single"), vec![true]);
    assert_eq!(guards(&program, "literal"), vec![true]);
    // A true/false pair already closes with its `false` arm as the fallback;
    // the leading closed-true arm keeps its guard because that arm follows it.
    assert_eq!(guards(&program, "followed"), vec![false, true]);
    assert_eq!(guards(&program, "read"), vec![false, true]);
}
