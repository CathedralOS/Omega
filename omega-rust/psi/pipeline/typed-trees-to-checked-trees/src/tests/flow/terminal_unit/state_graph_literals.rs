//! Literal byte arguments retain their source order across authored graph edges.

use super::*;
use checked_trees::{CheckedStructuralAccess, CheckedUnitStructuralArgumentSourcePlan};

fn retains_literal_graph(boundary: bool) {
    let declaration = if boundary {
        "boundary trait Host { machine emit(first: &[u8], marker: u8, second: &[u8]); }"
    } else {
        "data Host {} machine Host::emit(first: &[u8], marker: u8, second: &[u8]) {}"
    };
    let checked = checked(&format!(
        r#"
        {declaration}
        data Root {{ value: u8; }}
        machine Root::run(&mut self, again: bool) {{
            Host::emit("entry", 1u8, "");
            self.value = 7;
            Host::emit("after store", 2u8, "é");
            transition {{ _ -> body(again) }}
            state body(&mut self, again: bool) {{
                Host::emit("body", 3u8, "backedge");
                transition again {{ true -> body(false) _ -> done() }}
            }}
            state done(&mut self) {{ Host::emit("exit", 4u8, "tail"); }}
        }}
    "#
    ));
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_for_machine(machine_named(&checked, "Root::run"))
        .expect("literal arguments preserve the complete authored graph");
    assert_eq!(plan.states.len(), 3);
    let expected = [
        (0, 0, [b"entry".as_slice(), b"".as_slice()]),
        (0, 2, [b"after store".as_slice(), "é".as_bytes()]),
        (1, 0, [b"body".as_slice(), b"backedge".as_slice()]),
        (2, 0, [b"exit".as_slice(), b"tail".as_slice()]),
    ];
    let calls = plan
        .states
        .iter()
        .enumerate()
        .flat_map(|(state, plan)| {
            plan.operations
                .iter()
                .filter_map(move |operation| match operation {
                    CheckedUnitEffectOperationPlan::BoundaryCall {
                        coordinate,
                        structural_arguments,
                        ..
                    } if boundary => Some((state, coordinate, structural_arguments)),
                    CheckedUnitEffectOperationPlan::CallUnit {
                        coordinate,
                        structural_arguments,
                        ..
                    } if !boundary => Some((state, coordinate, structural_arguments)),
                    _ => None,
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), expected.len());
    for ((state, coordinate, arguments), (expected_state, statement, bytes)) in
        calls.into_iter().zip(expected)
    {
        assert_eq!(
            (state, coordinate.statement_index, coordinate.call_ordinal),
            (expected_state, statement, 0)
        );
        assert_eq!(arguments.len(), 2);
        for (argument, expected_bytes) in arguments.iter().zip(bytes) {
            assert_eq!(argument.access, CheckedStructuralAccess::SharedBorrow);
            assert!(argument.path.is_empty());
            assert!(
                matches!(&argument.source, CheckedUnitStructuralArgumentSourcePlan::ByteSequenceLiteral { bytes }
                if bytes.as_slice() == expected_bytes)
            );
        }
    }
    assert!(matches!(&plan.states[1].terminator,
        checked_trees::CheckedComposedUnitControlTerminatorPlan::Conditional { when_true, when_false, .. }
            if when_true.target_state == plan.states[1].state && when_false.target_state == plan.states[2].state));
}

#[test]
fn state_graph_retains_boundary_literal_arguments_across_backedges() {
    retains_literal_graph(true);
}

#[test]
fn state_graph_retains_ordinary_literal_arguments_across_backedges() {
    retains_literal_graph(false);
}
