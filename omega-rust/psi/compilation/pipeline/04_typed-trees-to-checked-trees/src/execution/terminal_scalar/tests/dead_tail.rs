use crate::tests::front_end::checked_program_result;
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};

fn checked(source: &str) -> checked_trees::CheckedTrees {
    checked_program_result(source)
        .unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

#[test]
fn exhaustive_guarded_pairs_deduce_trailing_dead_arms() {
    let checked = checked(
        r#"
machine three_arm(x: u64) -> u64 {
    transition x < 10 {
        true -> 1
        false -> 2
        _ -> 3
    }
}
machine double_fallback(x: u64) -> u64 {
    transition x < 10 {
        true -> 1
        _ -> 2
        _ -> 3
    }
}
machine swapped(x: u64) -> u64 {
    transition x < 10 {
        false -> 1
        true -> 2
        _ -> 3
    }
}
"#,
    );
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    for name in ["three_arm", "double_fallback", "swapped"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let graph = plans
            .for_machine(machine.symbol)
            .unwrap_or_else(|| panic!("{name} admits a scalar control graph"));
        let [state] = graph.states.as_slice() else {
            panic!("{name} produces a single-state graph");
        };
        let CheckedScalarStateTerminator::Conditional {
            guard_statement_ordinal: 0,
            when_true,
            when_false,
        } = &state.terminator
        else {
            panic!(
                "{name} selects a conditional terminator: {:?}",
                state.terminator
            );
        };
        assert!(matches!(
            when_true,
            CheckedScalarBranchDestination::Return {
                statement_ordinal: 0,
                is_continuation: false,
            }
        ));
        assert!(matches!(
            when_false,
            CheckedScalarBranchDestination::Return {
                statement_ordinal: 1,
                is_continuation: false,
            }
        ));
    }
}

#[test]
fn non_exhausted_tails_still_decline_a_scalar_graph() {
    let checked = checked(
        r#"
machine all_fallback(x: u64) -> u64 {
    transition x < 10 {
        _ -> 1
        _ -> 2
    }
}
"#,
    );
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "all_fallback")
        .unwrap();
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .is_none()
    );
}
