//! Owned byte arguments keep source custody independently of their view ABI.
use super::*;

#[test]
fn range_arguments_delegate_to_exact_descriptor_replay() {
    for boundary in [false, true] {
        let declaration = if boundary {
            "boundary trait Output { machine write(bytes: &[u8]) reaches Output; }"
        } else {
            "boundary trait Output { machine count(value: u64) reaches Output; }
             data Root {}
             machine Root::report(bytes: &[u8]) reaches Output {
                 Output::count(bytes.len);
             }"
        };
        let (storage, call) = if boundary {
            (
                "let bytes: [u8; 2] = [7, 9];",
                "Output::write(&bytes[0..2]);",
            )
        } else {
            (
                "let rows: [[u8; 2]; 2] = [[3, 5], [7, 9]];",
                "Root::report(&rows[1][0..2]);",
            )
        };
        let checked = crate::front_end::checked_program(&format!(
            "{declaration} machine send() reaches Output {{ {storage} {call} }}"
        ));
        crate::lower_machine(&checked, crate::TerminalMachineSelection::Name("send"))
            .unwrap_or_else(|error| {
                panic!("a derived descriptor is not a direct backing loan (boundary: {boundary}): {error:?}")
            });

        for mutation in ["root", "path", "access", "endpoint", "expression"] {
            let mut changed = checked.clone();
            let plans = &mut changed.facts.flow.terminal_unit_effects;
            let argument = plans
                .machines
                .iter_mut()
                .flat_map(|machine| &mut machine.operations)
                .chain(
                    plans
                        .composed_machines
                        .iter_mut()
                        .flat_map(|machine| &mut machine.states)
                        .flat_map(|state| &mut state.operations),
                )
                .filter_map(|operation| operation.call_mut())
                .flat_map(|call| &mut call.structural_arguments)
                .find(|argument| crate::unit::attached_unit::view_ranges::range(argument).is_some())
                .expect("the call retains a derived range descriptor");
            match mutation {
                "access" => argument.access = CheckedStructuralAccess::MutableBorrow,
                "path" => {
                    argument
                        .path
                        .push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                            0,
                        ))
                }
                _ => {
                    let CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
                        root,
                        expression,
                        start,
                        ..
                    } = &mut argument.source
                    else {
                        panic!("the authored byte range retains its byte descriptor");
                    };
                    match mutation {
                        "root" => {
                            *root = checked_trees::CheckedStorageRoot::OwnedLocal {
                                symbol: SymbolHandle::invalid(),
                            };
                        }
                        "endpoint" => *start = None,
                        "expression" => {
                            *expression = checked_trees::expression::ExpressionHandle::invalid();
                        }
                        _ => unreachable!(),
                    }
                }
            }
            assert!(
                crate::lower_machine(&changed, crate::TerminalMachineSelection::Name("send"))
                    .is_err(),
                "{mutation} must fail authoritative range replay (boundary: {boundary})"
            );
        }
    }
}

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
