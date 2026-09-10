use super::*;

const CYCLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/dependent/valuation_order_cycle/main.omg"
));

fn typed(source: &str) -> TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
}

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&typed(source), &mut diagnostics);
    diagnostics
}

#[test]
fn newly_reached_cycle_preserves_equal_literals_in_either_field_order() {
    // The exit write requires right == 2 to survive the cycle. Losing all
    // constants would terminate too, but would refuse this valid write.
    for source in [
        CYCLE.to_owned(),
        CYCLE.replace("Pair { right: 2, left: 1 }", "Pair { left: 1, right: 2 }"),
    ] {
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        assert!(build_open_invariant_crash_sites(&typed(&source)).is_empty());
    }
}

#[test]
fn reachable_conflicts_and_missing_fields_do_not_become_constants() {
    for source in [
        CYCLE.replace("Pair { right: 2, left: 1 }", "Pair { right: 1, left: 1 }"),
        CYCLE.replace("self.pair = Pair { right: 2, left: 1 };", ""),
        CYCLE.replace("self.pair.left = 2;", "self.pair.left = 3;"),
    ] {
        let diagnostics = diagnostics(&source);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("default domain")),
            "{source}\n{diagnostics:#?}"
        );
    }
}

#[test]
fn transported_open_windows_still_require_closure_or_crash_evidence() {
    let source = r#"
        data Pair where left <= right, { left: u64; right: u64; }
        data Storage { pair: Pair; }
        machine Storage::walk(&mut self) {
            self.pair = Pair { left: 1, right: 2 };
            self.pair.left = 3;
            transition { _ -> done() }
            state done(&mut self) { crash Trap; }
        }
    "#;
    let program = typed(source);
    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&program, &mut diagnostics);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let sites = build_open_invariant_crash_sites(&program);
    assert_eq!(sites.len(), 1);
    let pair = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Pair")
        .unwrap();
    assert_eq!(sites[0].open_data(), &[pair.symbol]);
    let done = program
        .machine_states(&program.machines()[0])
        .iter()
        .find(|state| state.name.as_str() == "done")
        .unwrap();
    assert_eq!(sites[0].state(), done.symbol);

    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&typed(&source.replace("crash Trap;", "")), &mut diagnostics);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("in a predecessor state")),
        "{diagnostics:#?}"
    );
    assert!(
        build_open_invariant_crash_sites(&typed(
            &source.replace("crash Trap;", "self.pair.left = 2; crash Trap;")
        ))
        .is_empty()
    );
}

#[test]
fn valuation_publication_ignores_place_and_field_permutations() {
    let first = (
        "self.first".to_owned(),
        vec![("left".to_owned(), Some(1)), ("right".to_owned(), Some(2))],
    );
    let second = (
        "self.second".to_owned(),
        vec![("left".to_owned(), None), ("right".to_owned(), Some(4))],
    );
    let expected = vec![first, second];
    for reverse_places in [false, true] {
        for reverse_fields in [false, true] {
            let mut valuations = expected.clone();
            if reverse_places {
                valuations.reverse();
            }
            if reverse_fields {
                for (_, fields) in &mut valuations {
                    fields.reverse();
                }
            }
            canonicalize_valuations(&mut valuations);
            assert_eq!(valuations, expected);
            canonicalize_valuations(&mut valuations);
            assert_eq!(valuations, expected);
        }
    }
}

#[test]
fn canonical_meet_retains_only_shared_known_literals() {
    let left = vec![(
        "self".to_owned(),
        vec![
            ("same".to_owned(), Some(2)),
            ("conflict".to_owned(), Some(3)),
            ("missing".to_owned(), Some(4)),
            ("unknown".to_owned(), None),
        ],
    )];
    let right = vec![(
        "self".to_owned(),
        vec![
            ("unknown".to_owned(), None),
            ("conflict".to_owned(), Some(5)),
            ("same".to_owned(), Some(2)),
        ],
    )];
    for (left, right) in [(&left, &right), (&right, &left)] {
        let mut meet = meet_valuations(left, right);
        canonicalize_valuations(&mut meet);
        assert_eq!(
            meet,
            vec![("self".to_owned(), vec![("same".to_owned(), Some(2))])]
        );
    }
}
