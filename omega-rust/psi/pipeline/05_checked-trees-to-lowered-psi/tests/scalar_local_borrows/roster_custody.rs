use super::{TWO_LOCALS, publish_original, reject};

#[test]
fn primitive_local_roster_rejects_missing_duplicate_and_crossed_identities() {
    let original = publish_original(TWO_LOCALS);
    let root = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap()
        .symbol;
    for mutation in [
        "missing first local",
        "missing all locals",
        "duplicate local row",
        "swapped whole rows",
        "swapped symbols at fixed declaration ordinals",
    ] {
        let mut changed = original.clone();
        let graph = changed
            .facts
            .flow
            .terminal_scalar_graphs
            .machines
            .iter_mut()
            .find(|graph| graph.machine == root)
            .expect("scalar root graph");
        assert_eq!(graph.states.len(), 1);
        let locals = &mut graph.states[0].primitive_locals;
        assert_eq!(locals.len(), 2);
        assert_eq!(locals[0].statement_ordinal, 0);
        assert_eq!(locals[1].statement_ordinal, 1);
        assert_ne!(locals[0].symbol, locals[1].symbol);
        assert_eq!(locals[0].primitive_type, locals[1].primitive_type);
        assert_eq!(locals[0].type_identity, locals[1].type_identity);
        match mutation {
            "missing first local" => {
                locals.remove(0);
            }
            "missing all locals" => locals.clear(),
            "duplicate local row" => locals.push(locals[0].clone()),
            "swapped whole rows" => locals.swap(0, 1),
            "swapped symbols at fixed declaration ordinals" => {
                let first = locals[0].symbol;
                locals[0].symbol = locals[1].symbol;
                locals[1].symbol = first;
            }
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}
