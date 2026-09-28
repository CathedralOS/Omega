use super::{
    Diagnostic, build_open_invariant_crash_sites, canonicalize_valuations,
    validate_default_domain_writes,
};
const CYCLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../tests/omega/pass/dependent/valuation_order_cycle/main.omg"
));

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    validate_default_domain_writes(&crate::front_end::typed_program(source), &mut diagnostics);
    diagnostics
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
    let program = crate::front_end::typed_program(source);
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
    validate_default_domain_writes(
        &crate::front_end::typed_program(&source.replace("crash Trap;", "")),
        &mut diagnostics,
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("in a predecessor state")),
        "{diagnostics:#?}"
    );
    assert!(
        build_open_invariant_crash_sites(&crate::front_end::typed_program(
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
        Some(symbols::SymbolHandle::from_arena_index(7)),
    );
    let second = (
        "self.second".to_owned(),
        vec![("left".to_owned(), None), ("right".to_owned(), Some(4))],
        None,
    );
    let expected = vec![first, second];
    for reverse_places in [false, true] {
        for reverse_fields in [false, true] {
            let mut valuations = expected.clone();
            if reverse_places {
                valuations.reverse();
            }
            if reverse_fields {
                for (_, fields, _) in &mut valuations {
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
fn gated_read_inside_cast_refuses_while_window_open() {
    // `left < right` makes Pair establishment-gated (zero image violates
    // it), so reads during the opened window police the consumption point.
    let source = r#"
        data Pair where left < right, { left: u64; right: u64; }
        data Main { pair: Pair; }
        machine Main::main(&mut self) {
            self.pair = Pair { left: 1, right: 2 };
            self.pair.left = 9;
            let sunk: u64 = self.pair.right as u64;
            self.pair = Pair { left: 0, right: 0 };
        }
    "#;
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("OPEN invariant window")),
        "{diagnostics:#?}"
    );
}

#[test]
fn dynamic_index_write_opens_window_until_whole_place_write() {
    // `self.maps[i]` is an unrepresentable origin: the write may hit any
    // element, so the wildcard window covers the whole region and a literal
    // element re-proof cannot close it -- the next call still refuses.
    let source = r#"
        data Map where start <= end, { start: i32; end: i32; }
        data Main { maps: [Map; 2]; }
        machine Main::main(&mut self, i: u32) {
            self.maps[0] = Map { start: 0, end: 4 };
            self.maps[i].start = 9;
            self.maps[0].start = 1;
            self.maps[0].end = 2;
            self.touch();
        }
        machine Main::touch(&mut self) {}
    "#;
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("a call")),
        "{diagnostics:#?}"
    );
}
