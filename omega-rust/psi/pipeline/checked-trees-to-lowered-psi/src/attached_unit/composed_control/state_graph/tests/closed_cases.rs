use super::*;

fn fixture() -> (CheckedTrees, CheckedComposedUnitControlMachinePlan) {
    let source = r#"
        data ByteRead { case Eof; case Byte(value: i32 [0..=255]); }
        boundary trait Console { machine read_byte() -> ByteRead reaches Console; }
        machine read_one(out: &mut [u8]) reaches Console {
            transition out.len > 0 { true -> read(out) false -> done() }
            state read(out: &mut [u8]) {
                let observed: ByteRead = Console::read_byte();
                transition observed {
                    ByteRead::Byte { value } -> store(out, value)
                    ByteRead::Eof -> done()
                }
            }
            state store(out: &mut [u8], value: i32 [0..=255]) { out[0] = value as u8; }
            state done() {}
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap();
    let plan = checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .find(|plan| {
            plan.states.iter().any(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
        })
        .expect("general source graph retains its inspected result")
        .clone();
    (checked, plan)
}

#[test]
fn closed_case_graph_rejoins_source_and_rejects_drift() {
    let (checked, original) = fixture();
    admission::admit(&checked, &original).expect("exact checked source admission");
    for mutation in 0..10 {
        let mut changed = original.clone();
        let state = changed
            .states
            .iter_mut()
            .find(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .unwrap();
        let CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases } =
            &mut state.terminator
        else {
            panic!("selected case state");
        };
        match mutation {
            0 => cases.reverse(),
            1 => cases[0].case_identity.push_str("-forged"),
            2 => cases[0].payloads[0].field_identity.push_str("-forged"),
            3 => cases[0].successor.transfers.clear(),
            4 => result.statement_index += 1,
            5 => state.operations.clear(),
            6 => cases[0].payloads[0].target_scalar_parameter_index += 1,
            7 => cases[0].successor.statement_ordinal += 1,
            8 => cases[0]
                .successor
                .trivial_affine_discard_parameter_positions
                .push(0),
            9 => {
                let payload = cases[0].payloads[0].clone();
                cases[0].payloads.push(payload);
            }
            _ => unreachable!(),
        }
        assert!(
            admission::admit(&checked, &changed).is_err(),
            "case mutation {mutation}"
        );
    }
}

#[test]
fn closed_case_graph_rejects_missing_or_substituted_local_cleanup() {
    for substitute_root in [false, true] {
        let (mut checked, plan) = fixture();
        admission::admit(&checked, &plan).expect("exact checked source admission");
        let state = plan
            .states
            .iter()
            .find(|state| {
                matches!(
                    state.terminator,
                    CheckedComposedUnitControlTerminatorPlan::ClosedSum { .. }
                )
            })
            .unwrap();
        let handles = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .filter(|(_, event)| {
                event.machine_symbol == plan.machine
                    && event.state_symbol == state.state
                    && event.source == language_semantics::PermissionEventSource::StateExit
                    && event.kind == language_semantics::PermissionEventKind::AffineDrop
            })
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        assert!(!handles.is_empty(), "result-local cleanup evidence exists");
        for handle in handles {
            let event = checked.facts.flow.ownership.permissions.get_mut(handle);
            if substitute_root {
                event.root = facts::PlaceRoot::Unknown;
            } else {
                event.source = language_semantics::PermissionEventSource::StateEntry;
            }
        }
        assert!(admission::admit(&checked, &plan).is_err());
    }
}
