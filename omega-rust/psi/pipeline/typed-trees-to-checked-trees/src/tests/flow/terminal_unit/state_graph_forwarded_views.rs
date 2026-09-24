//! A free multi-state machine carries each authored state's own structural
//! roster: a borrowed view formal forwards whole across the edges that reach
//! its state, bound by the same signature a single-state graph uses.
use super::{Multiplicity, PrimitiveType};
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{
    CheckedScalarBranchDestination, CheckedScalarStateTerminator, CheckedStructuralAccess,
    CheckedStructuralControlTransferSourcePlan,
};

const SOURCE: &str = r#"
    machine scan(line: &[u8], start: u64, end: u64) -> u64 {
        transition { _ -> step(line, start, end, start) }
        state step(line: &[u8], start: u64, end: u64, position: u64) -> u64 {
            transition position < end {
                true -> finish(line, position)
                false -> (start)
            }
        }
        state finish(line: &[u8], position: u64) -> u64 {
            transition position < line.len {
                true -> (position)
                _ -> (line.len)
            }
        }
    }
"#;

#[test]
fn free_multi_state_machine_forwards_shared_view_formals() {
    let checked = checked(SOURCE);
    let machine = machine_named(&checked, "scan");
    let plans = &checked.facts.flow.terminal_scalar_graphs;
    let graph = plans
        .for_machine(machine)
        .expect("multi-state free machine with forwarded view formals");
    assert_eq!(graph.states.len(), 3);
    for state in &graph.states {
        let [line] = state.structural_parameters.as_slice() else {
            panic!("each authored state carries its own borrowed view formal");
        };
        assert_eq!(line.access, CheckedStructuralAccess::SharedBorrow);
        assert_eq!(line.multiplicity, Multiplicity::Unrestricted);
        assert!(!line.is_self);
    }
    let [start, end, position] = graph.states[1].scalar_parameters.as_slice() else {
        panic!("step keeps its scalar roster");
    };
    assert_eq!(
        (
            start.source_position,
            start.primitive_type,
            end.source_position,
            end.primitive_type,
            position.source_position,
            position.primitive_type
        ),
        (
            1,
            PrimitiveType::U64,
            2,
            PrimitiveType::U64,
            3,
            PrimitiveType::U64
        )
    );
    let [finish_position] = graph.states[2].scalar_parameters.as_slice() else {
        panic!("finish keeps its own scalar roster");
    };
    assert_eq!(
        (
            finish_position.source_position,
            finish_position.primitive_type
        ),
        (1, PrimitiveType::U64)
    );
    let CheckedScalarStateTerminator::Jump(entry_edge) = &graph.states[0].terminator else {
        panic!("entry jumps to its first authored state");
    };
    let transfers = plans
        .structural_transfers
        .span(entry_edge.structural_transfers)
        .expect("recorded entry transfers");
    assert!(matches!(transfers, [transfer] if matches!(
            transfer.source,
            CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
        ) && transfer.target_parameter_index == 0));
    let CheckedScalarStateTerminator::Conditional {
        when_true,
        when_false,
        ..
    } = &graph.states[1].terminator
    else {
        panic!("step branches on its authored guard");
    };
    let CheckedScalarBranchDestination::Jump(step_edge) = when_true else {
        panic!("the guarded arm jumps to finish");
    };
    let transfers = plans
        .structural_transfers
        .span(step_edge.structural_transfers)
        .expect("recorded step transfers");
    assert!(matches!(transfers, [transfer] if matches!(
            transfer.source,
            CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
        ) && transfer.target_parameter_index == 0));
    assert!(matches!(
        when_false,
        CheckedScalarBranchDestination::Return { .. }
    ));
}

#[test]
fn mutable_view_formal_keeps_multi_state_machine_off_the_scalar_graph() {
    let checked = checked(
        r#"
        machine walk(view: &mut [u8], start: u64) -> u64 {
            transition { _ -> step(view, start) }
            state step(view: &mut [u8], start: u64) -> u64 {
                start
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "walk");
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .is_none()
    );
}

#[test]
fn named_view_formal_keeps_multi_state_machine_off_the_scalar_graph() {
    let checked = checked(
        r#"
        data Refs { a: u64; }
        machine walk<'r>(refs: &'r Refs, start: u64) -> u64 {
            transition { _ -> step(refs, start) }
            state step(refs: &'r Refs, start: u64) -> u64 {
                start
            }
        }
    "#,
    );
    let machine = machine_named(&checked, "walk");
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .is_none()
    );
}
