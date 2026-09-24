//! A free multi-state machine carries each authored state's own structural
//! roster: a borrowed view formal forwards whole across the edges that reach
//! its state. That forwarding belongs to the Unit state graph; the scalar
//! graph keeps structural formals to one body.
use super::{Multiplicity, PrimitiveType};
use crate::tests::flow::terminal_unit::{checked, machine_named};
use checked_trees::{
    CheckedComposedUnitControlTerminatorPlan, CheckedStructuralAccess,
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
    assert!(
        checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine)
            .is_none(),
        "structural formals stay with one scalar-graph body"
    );
    let graph = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine)
        .expect("the state graph owns the forwarded view formals");
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
    let CheckedComposedUnitControlTerminatorPlan::Jump { successor } = &graph.states[0].terminator
    else {
        panic!("entry jumps to its first authored state");
    };
    assert!(
        matches!(successor.transfers.as_slice(), [transfer] if matches!(
            transfer.source,
            CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
        ) && transfer.target_parameter_index == 0)
    );
    let CheckedComposedUnitControlTerminatorPlan::ConditionalReturn {
        jump,
        return_when_true: false,
        ..
    } = &graph.states[1].terminator
    else {
        panic!("step branches on its authored guard and returns on the false arm");
    };
    assert_eq!(
        jump.target_state,
        checked.machine_states(
            checked
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == machine)
                .unwrap(),
        )[2]
        .symbol
    );
    assert!(matches!(jump.transfers.as_slice(), [transfer] if matches!(
            transfer.source,
            CheckedStructuralControlTransferSourcePlan::Parameter { index: 0 }
        ) && transfer.target_parameter_index == 0));
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
