use super::{close_storage_places_over_aliases, close_storage_places_over_aliases_with_resolver};
use crate::flow::CanonicalPlace;
use crate::tests::front_end::typed_program;
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;
use validation::CallFrameResolver;

fn place(root: SymbolHandle, segments: &[PlaceSegment]) -> CanonicalPlace {
    CanonicalPlace {
        root: PlaceRoot::Symbol(root),
        segments: segments.to_vec(),
    }
}

#[test]
fn missing_resolver_preserves_only_complete_empty_writes() {
    for duplicate_symbols in [false, true] {
        let mut program = typed_program(
            "machine probe(value: &mut u64) {
                 let alias: &mut u64 = value;
                 alias = 1;
             }
             machine unrelated() {}",
        );
        if duplicate_symbols {
            // Corrupt before borrowing: this exercises constructor failure,
            // never reuse across a mutation of the typed program.
            let duplicate_name = program.machines()[0].name.clone();
            program.machines_mut()[1].name = duplicate_name;
        }
        let resolver = CallFrameResolver::new(&program);
        assert_eq!(resolver.is_none(), duplicate_symbols);
        let machine = &program.machines()[0];
        let state = &program.machine_states(machine)[0];
        let value = program.state_parameters(state)[0].symbol;
        for input in [Vec::new(), vec![place(value, &[])]] {
            let missing = close_storage_places_over_aliases_with_resolver(
                &program,
                machine.symbol,
                state.symbol,
                1,
                input.clone(),
                None,
            );
            assert_eq!(missing, input.is_empty().then(Vec::new));
            let fresh = close_storage_places_over_aliases(
                &program,
                machine.symbol,
                state.symbol,
                1,
                input.clone(),
            );
            if duplicate_symbols || input.is_empty() {
                assert_eq!(missing, fresh);
            } else {
                let StatementNode::LocalData(alias) =
                    &program.statement_table.statements(state.statement_nodes)[0]
                else {
                    panic!("alias declaration");
                };
                assert_eq!(
                    fresh,
                    Some(vec![place(value, &[]), place(alias.symbol, &[])])
                );
            }
        }
    }
}
