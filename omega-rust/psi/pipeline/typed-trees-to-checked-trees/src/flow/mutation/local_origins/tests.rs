use super::{close_storage_places_over_aliases, close_storage_places_over_aliases_with_resolver};
use crate::flow::CanonicalPlace;
use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;
use validation::CallFrameResolver;

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize alias closure fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse alias closure fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
        .expect("resolve alias closure fixture");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type alias closure fixture")
}

fn place(root: SymbolHandle, segments: &[PlaceSegment]) -> CanonicalPlace {
    CanonicalPlace {
        root: PlaceRoot::Symbol(root),
        segments: segments.to_vec(),
    }
}

#[test]
fn shared_alias_closure_uses_each_statement_prefix_across_rebinding() {
    let program = typed(
        "data Pair { value: u64; }
         machine probe(first: &mut Pair, second: &mut Pair) {
             let mut selected: &mut Pair = first;
             let prior: &mut Pair = selected;
             selected = second;
             prior.value = 1;
             selected.value = 2;
         }",
    );
    let machine = &program.machines()[0];
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(selected) = &statements[0] else {
        panic!("selected declaration");
    };
    let StatementNode::LocalData(prior) = &statements[1] else {
        panic!("saved alias declaration");
    };
    assert!(matches!(statements[2], StatementNode::Assignment(_)));
    let parameters = program.state_parameters(state);
    let first = parameters[0].symbol;
    let second = parameters[1].symbol;
    let typed_trees::data::DataMember::Field(field) =
        &program.data_members(&program.data_definitions()[0])[0]
    else {
        panic!("Pair.value field");
    };
    let segments = [PlaceSegment::Field {
        symbol: field.symbol,
    }];

    for sites in [[2, 3, 4, 2], [4, 3, 2, 4]] {
        let resolver = CallFrameResolver::new(&program).expect("valid symbol table");
        for statement_index in sites {
            for root in [first, second] {
                let input = vec![place(root, &segments)];
                let actual = close_storage_places_over_aliases_with_resolver(
                    &program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    input.clone(),
                    Some(&resolver),
                );
                let fresh = close_storage_places_over_aliases(
                    &program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    input,
                );
                assert_eq!(actual, fresh, "site {statement_index}, root {root:?}");

                let expected_roots = match (statement_index == 2, root == first) {
                    (true, true) => vec![first, selected.symbol, prior.symbol],
                    (true, false) => vec![second],
                    (false, true) => vec![first, prior.symbol],
                    (false, false) => vec![second, selected.symbol],
                };
                let actual = actual.expect("complete prefix origins");
                assert_eq!(actual.len(), expected_roots.len());
                for expected_root in expected_roots {
                    assert!(
                        actual.contains(&place(expected_root, &segments)),
                        "site {statement_index}, root {root:?}: {actual:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn opaque_prefix_queries_do_not_poison_earlier_alias_closure() {
    let program = typed(
        "data Pair { value: u64; }
         machine Pair::opaque(&mut self) { unknown(Pair { value: 0 }); }
         machine probe(pair: &mut Pair) {
             let receiver: &mut Pair = pair;
             receiver.opaque();
             unknown(Pair { value: 0 });
             receiver.opaque();
         }",
    );
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "probe")
        .expect("probe");
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let StatementNode::LocalData(receiver) = &statements[0] else {
        panic!("receiver declaration");
    };
    let pair = program.state_parameters(state)[0].symbol;
    for sites in [[1, 3, 1, 3], [3, 1, 3, 1]] {
        let resolver = CallFrameResolver::new(&program).expect("valid symbol table");
        for statement_index in sites {
            let actual = close_storage_places_over_aliases_with_resolver(
                &program,
                machine.symbol,
                state.symbol,
                statement_index,
                vec![place(pair, &[])],
                Some(&resolver),
            );
            assert_eq!(
                actual,
                close_storage_places_over_aliases(
                    &program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    vec![place(pair, &[])],
                ),
            );
            assert_eq!(
                actual,
                (statement_index == 1).then(|| vec![place(pair, &[]), place(receiver.symbol, &[])]),
                "site {statement_index}",
            );
            assert_eq!(
                close_storage_places_over_aliases_with_resolver(
                    &program,
                    machine.symbol,
                    state.symbol,
                    statement_index,
                    Vec::new(),
                    Some(&resolver),
                ),
                Some(Vec::new()),
                "complete empty writes need no prefix evidence",
            );
        }
    }
}

#[test]
fn missing_resolver_preserves_only_complete_empty_writes() {
    for duplicate_symbols in [false, true] {
        let mut program = typed(
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
