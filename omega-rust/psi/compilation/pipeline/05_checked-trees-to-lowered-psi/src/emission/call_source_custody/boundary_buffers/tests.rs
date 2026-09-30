//! Owned byte arguments keep source custody independently of their view ABI.
use super::*;

#[test]
fn owned_byte_boundary_loan_replays_whole_local_and_shared_access() {
    for actual in ["line", "&line"] {
        let source = format!(
            r#"
            boundary trait Output {{ machine write(bytes: &[u8]) reaches Output; }}
            domain [u8; 8]::Utf8 requires valid_utf8(self);
            machine send() reaches Output {{
                transition {{ _ -> render() }}
                state render() {{
                    let mut line: [u8; 8] in Utf8 = "ab";
                    let other: [u8; 8] in Utf8 = "cd";
                    line[0] = 0;
                    Output::write({actual});
                    Output::write(other);
                }}
            }}
        "#
        );
        let checked = crate::front_end::checked_program(&source);
        let state = checked
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter()
            .flat_map(|machine| &machine.states)
            .find(|state| {
                state
                    .operations
                    .iter()
                    .filter(|operation| operation.call().is_some_and(|call| call.is_boundary()))
                    .count()
                    == 2
            })
            .expect("two boundary uses retain their local owner bindings");
        let calls = state
            .operations
            .iter()
            .filter(|operation| operation.call().is_some_and(|call| call.is_boundary()))
            .collect::<Vec<_>>();
        let planned = calls[0].call().unwrap();
        let authored = authored::locate_source(&checked, state.state, planned.coordinate).unwrap();
        validate(
            &checked,
            state.state,
            &state.structural_parameters,
            planned.coordinate,
            &authored,
            calls[0],
        )
        .unwrap();
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("send"))
            .expect("the complete owned-to-view boundary call lowers after a byte mutation");

        for replace_owner in [false, true] {
            let mut changed = checked.clone();
            let argument = &mut changed
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter_mut()
                .flat_map(|machine| &mut machine.states)
                .find(|candidate| candidate.state == state.state)
                .unwrap()
                .operations
                .iter_mut()
                .filter_map(|operation| operation.call_mut())
                .find(|call| call.coordinate == planned.coordinate)
                .unwrap()
                .structural_arguments[0];
            if replace_owner {
                argument.source = calls[1].call().unwrap().structural_arguments[0]
                    .source
                    .clone();
            } else {
                argument.access = CheckedStructuralAccess::MutableBorrow;
            }
            assert!(
                crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("send"),)
                    .is_err(),
                "forged boundary owner/access must fail full replay"
            );
        }

        let mut changed = calls[0].clone();
        changed.call_mut().unwrap().structural_arguments[0].access =
            CheckedStructuralAccess::MutableBorrow;
        assert!(
            validate(
                &checked,
                state.state,
                &state.structural_parameters,
                planned.coordinate,
                &authored,
                &changed
            )
            .is_err()
        );

        let mut changed_source =
            authored::locate_source(&checked, state.state, planned.coordinate).unwrap();
        let other_source =
            authored::locate_source(&checked, state.state, calls[1].call().unwrap().coordinate)
                .unwrap();
        changed_source.structural_arguments = other_source.structural_arguments;
        assert!(
            validate(
                &checked,
                state.state,
                &state.structural_parameters,
                planned.coordinate,
                &changed_source,
                calls[0]
            )
            .is_err()
        );
    }
}
