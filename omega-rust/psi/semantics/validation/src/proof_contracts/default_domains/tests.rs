use super::{
    Diagnostic, build_open_invariant_crash_sites, canonicalize_valuations, meet_valuations,
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
fn newly_reached_cycle_preserves_equal_literals_in_either_field_order() {
    // The exit write requires right == 2 to survive the cycle. Losing all
    // constants would terminate too, but would refuse this valid write.
    for source in [
        CYCLE.to_owned(),
        CYCLE.replace("Pair { right: 2, left: 1 }", "Pair { left: 1, right: 2 }"),
    ] {
        let diagnostics = diagnostics(&source);
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        assert!(
            build_open_invariant_crash_sites(&crate::front_end::typed_program(&source)).is_empty()
        );
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
fn canonical_meet_retains_only_shared_known_literals() {
    let left = vec![(
        "self".to_owned(),
        vec![
            ("same".to_owned(), Some(2)),
            ("conflict".to_owned(), Some(3)),
            ("missing".to_owned(), Some(4)),
            ("unknown".to_owned(), None),
        ],
        None,
    )];
    let right = vec![(
        "self".to_owned(),
        vec![
            ("unknown".to_owned(), None),
            ("conflict".to_owned(), Some(5)),
            ("same".to_owned(), Some(2)),
        ],
        None,
    )];
    for (left, right) in [(&left, &right), (&right, &left)] {
        let mut meet = meet_valuations(left, right);
        canonicalize_valuations(&mut meet);
        assert_eq!(
            meet,
            vec![("self".to_owned(), vec![("same".to_owned(), Some(2))], None)]
        );
    }
}

// DEPENDENT-VALUES-CHECKER-COVERAGE: a call nested inside any other
// expression shape is still "a call" consumption point, a gated read nested
// inside a non-Member shape still crosses the open window, and an
// unrepresentable index position opens a wildcard window.
#[test]
fn call_inside_cast_expression_consumes_open_window() {
    let source = r#"
        data Pair where left <= right, { left: u64; right: u64; }
        data Main { pair: Pair; }
        machine Main::main(&mut self) {
            self.pair = Pair { left: 1, right: 2 };
            self.pair.left = 9;
            let sunk: u64 = self.pick() as u64;
            self.pair = Pair { left: 0, right: 0 };
        }
        machine Main::pick(&mut self) -> i32 { 1 }
    "#;
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("a call")),
        "{diagnostics:#?}"
    );
}

#[test]
fn call_inside_struct_literal_consumes_open_window() {
    let source = r#"
        data Pair where left <= right, { left: u64; right: u64; }
        data Main { pair: Pair; }
        machine Main::main(&mut self) {
            self.pair = Pair { left: 1, right: 2 };
            self.pair.left = 9;
            let sunk: Pair = Pair { left: self.pick() as u64, right: 8 };
            self.pair = Pair { left: 0, right: 0 };
        }
        machine Main::pick(&mut self) -> i32 { 1 }
    "#;
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("a call")),
        "{diagnostics:#?}"
    );
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
fn call_inside_index_expression_consumes_open_window() {
    let source = r#"
        data Pair where left <= right, { left: u64; right: u64; }
        data Main { pair: Pair; vals: [u64; 2]; }
        machine Main::main(&mut self) {
            self.pair = Pair { left: 1, right: 2 };
            self.pair.left = 9;
            let sunk: u64 = self.vals[self.pick() as u32];
            self.pair = Pair { left: 0, right: 0 };
        }
        machine Main::pick(&mut self) -> i32 { 1 }
    "#;
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("a call")),
        "{diagnostics:#?}"
    );
}

#[test]
fn call_inside_transition_argument_consumes_open_window() {
    let source = r#"
        data Pair where left <= right, { left: u64; right: u64; }
        data Main { pair: Pair; }
        machine Main::main(&mut self) {
            self.pair = Pair { left: 1, right: 2 };
            self.pair.left = 9;
            transition { _ -> done(self.pick()) }
            state done(&mut self, value: i32) {
                self.pair = Pair { left: 0, right: 0 };
            }
        }
        machine Main::pick(&mut self) -> i32 { 1 }
    "#;
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("a call")),
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

#[test]
fn canonical_meet_retains_the_active_case_only_when_both_agree() {
    // CASE-CONSTRAINTS (ch12): a constrained case's `where` facts are only
    // knowable when every predecessor agrees which case is active; a
    // disagreeing or unknown side must drop the case to `None`.
    let case_a = symbols::SymbolHandle::from_arena_index(3);
    let case_b = symbols::SymbolHandle::from_arena_index(4);
    for (left_case, right_case, expected) in [
        (Some(case_a), Some(case_a), Some(case_a)),
        (Some(case_a), Some(case_b), None),
        (Some(case_a), None, None),
        (None, Some(case_a), None),
        (None, None, None),
    ] {
        let left = vec![("self".to_owned(), Vec::new(), left_case)];
        let right = vec![("self".to_owned(), Vec::new(), right_case)];
        for (left, right) in [(&left, &right), (&right, &left)] {
            assert_eq!(
                meet_valuations(left, right),
                vec![("self".to_owned(), Vec::new(), expected)]
            );
        }
    }
}
