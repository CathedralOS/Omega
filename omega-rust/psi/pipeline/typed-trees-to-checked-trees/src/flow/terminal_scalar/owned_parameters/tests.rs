use super::*;

fn checked() -> checked_trees::CheckedTrees {
    let source = "data Limits { limit: u64; divisor: u64 [3..=5]; }
        machine reset(value: &mut u64) -> u64 { value = 0; 0 }
        machine inspect(marker: u64, limits: Limits) -> u64 {
            let mut scratch: u64 = marker;
            let reset_result: u64 = reset(&mut scratch); limits.limit
        }
        machine enter(spare: Limits, other: Limits, limits: Limits, marker: u64) -> u64 {
            let answer: u64 = inspect(marker, limits); answer
        }";
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    crate::lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{diagnostics:#?}"))
}

#[test]
fn affine_graph_rejoins_discard_eligibility_and_whole_transfers() {
    let checked = checked();
    for name in ["inspect", "enter"] {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let graph = checked
            .facts
            .flow
            .terminal_scalar_graphs
            .for_machine(machine.symbol)
            .expect("original affine Limits graph");
        assert!(
            graph.states[0]
                .structural_parameters
                .iter()
                .all(|parameter| parameter.multiplicity == Multiplicity::Affine)
        );
        let state = &checked.machine_states(machine)[0];
        assert!(
            validate(
                &checked,
                &checked.facts.flow.ownership,
                &checked.facts.values.scalar_computations,
                machine.symbol,
                state,
                &graph.states[0].structural_parameters
            )
            .is_some()
        );
        for kind in [
            PermissionEventKind::AffineDrop,
            PermissionEventKind::Transfer,
        ] {
            let Some((handle, _)) =
                checked
                    .facts
                    .flow
                    .ownership
                    .permissions
                    .iter()
                    .find(|(_, event)| {
                        event.machine_symbol == machine.symbol
                            && event.kind == kind
                            && event.access == PermissionAccess::Owned
                    })
            else {
                assert_eq!(name, "inspect");
                assert_eq!(kind, PermissionEventKind::Transfer);
                continue;
            };
            for mutation in 0..5 {
                let mut ownership = checked.facts.flow.ownership.clone();
                match mutation {
                    0 => {
                        ownership.permissions.get_mut(handle).machine_symbol =
                            SymbolHandle::invalid()
                    }
                    1 => {
                        ownership.permissions.get_mut(handle).root =
                            facts::PlaceRoot::Symbol(SymbolHandle::invalid())
                    }
                    2 => ownership.permissions.get_mut(handle).obligation_live = true,
                    3 => ownership.permissions.get_mut(handle).multiplicity = Multiplicity::Linear,
                    _ => {
                        ownership
                            .permissions
                            .append(ownership.permissions.get(handle).clone());
                    }
                }
                let mut retained = checked.facts.flow.terminal_scalar_graphs.clone();
                super::super::super::finalize_checked_scalar_graph_plans(
                    &checked,
                    &ownership,
                    &checked.facts.values.scalar_computations,
                    &mut retained,
                );
                assert!(
                    retained.for_machine(machine.symbol).is_none(),
                    "{name} {kind:?} mutation {mutation}"
                );
            }
        }
    }
}

#[test]
fn affine_graph_rejects_reordered_discard_eligibility() {
    let checked = checked();
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let handles = checked
        .facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol && event.kind == PermissionEventKind::AffineDrop
        })
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    assert_eq!(handles.len(), 2);
    let mut ownership = checked.facts.flow.ownership.clone();
    let first = ownership.permissions.get(handles[0]).clone();
    let second = ownership.permissions.get(handles[1]).clone();
    *ownership.permissions.get_mut(handles[0]) = second;
    *ownership.permissions.get_mut(handles[1]) = first;
    let mut retained = checked.facts.flow.terminal_scalar_graphs.clone();
    super::super::super::finalize_checked_scalar_graph_plans(
        &checked,
        &ownership,
        &checked.facts.values.scalar_computations,
        &mut retained,
    );
    assert!(retained.for_machine(machine.symbol).is_none());
}
