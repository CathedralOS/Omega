use super::{
    Arc, CheckedTrees, ProviderPlan, selected_plan, settle_selected_boundary_adapter_dispatch,
};
/// A finite generic requirement whose roster is authored as one explicit
/// disjunction of value equalities. The provider's demanded tuple
/// specializations come from the direct calls in `direct`; the boundary calls
/// in `run` then select them tuple by tuple.
const FAMILY_SETTLES: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
        machine ping(value: u32) -> u32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::ping(value: u32) -> u32 satisfies Scanner::ping {
        transition { _ -> (value) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7) + self.service.scan<32>(8)) }
    }
"#;

/// Duplicates and reordering normalize to the same sorted roster.
const FAMILY_NORMALIZED: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 32 || Width == 16 || Width == 32;
        machine ping(value: u32) -> u32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::ping(value: u32) -> u32 satisfies Scanner::ping {
        transition { _ -> (value) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

/// Two-binder alternatives must write the whole tuple's correlation; the
/// roster is (16,4)/(32,8), never the Cartesian product of listed values.
const CORRELATED_FAMILY: &str = r#"
    boundary trait Table {
        machine lookup<const W: u32, const A: u32>(value: u32) -> u64 where W == 16 && A == 4 || W == 32 && A == 8;
    }
    data TableProvider {}
    machine TableProvider::lookup<const W: u32, const A: u32>(value: u32) -> u64 satisfies Table::lookup {
        transition { _ -> (value as u64) }
    }
    machine TableProvider::direct() -> u64 {
        transition { _ -> (TableProvider::lookup<16, 4>(7) + TableProvider::lookup<32, 8>(8)) }
    }
    data Client { service: Table; }
    machine Client::run(&mut self) -> u64 reaches Table {
        transition { _ -> (self.service.lookup<16, 4>(7) + self.service.lookup<32, 8>(8)) }
    }
"#;

/// An off-roster combination is a membership failure even though each value
/// appears in the family: correlations are authored per alternative.
const CORRELATED_MISMATCH: &str = r#"
    boundary trait Table {
        machine lookup<const W: u32, const A: u32>(value: u32) -> u64 where W == 16 && A == 4 || W == 32 && A == 8;
    }
    data TableProvider {}
    machine TableProvider::lookup<const W: u32, const A: u32>(value: u32) -> u64 satisfies Table::lookup {
        transition { _ -> (value as u64) }
    }
    machine TableProvider::direct() -> u64 {
        transition { _ -> (TableProvider::lookup<16, 4>(7) + TableProvider::lookup<32, 8>(8)) }
    }
    data Client { service: Table; }
    machine Client::run(&mut self) -> u64 reaches Table {
        transition { _ -> (self.service.lookup<16, 8>(7)) }
    }
"#;

/// An alternative that binds only one of two value binders is not a complete
/// tuple; the requirement stays ineligible.
const PARTIAL_TUPLE: &str = r#"
    boundary trait Table {
        machine lookup<const W: u32, const A: u32>(value: u32) -> u64 where W == 16 || W == 32;
    }
    data TableProvider {}
    machine TableProvider::lookup<const W: u32, const A: u32>(value: u32) -> u64 satisfies Table::lookup {
        transition { _ -> (value as u64) }
    }
    data Client { service: Table; }
    machine Client::run(&mut self) -> u64 reaches Table {
        transition { _ -> (self.service.lookup<16, 4>(7)) }
    }
"#;

/// An opaque predicate is never an enumeration.
const OPAQUE_FAMILY: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width > 0;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

/// An inequality range does not enumerate, no matter how small it is.
const RANGE_FAMILY: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width >= 16 && Width <= 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

/// A roster whose demanded tuples were never specialized realizes no rows;
/// the requirement is ineligible rather than dispatching to the template.
const FAMILY_WITHOUT_SPECIALIZATIONS: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

/// The provider realized (16) and (32) but never the declared (64): one
/// selected conformance must cover the complete roster, so the whole family
/// is ineligible rather than settling a table that silently drops (64).
const PARTIAL_COVERAGE: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32 || Width == 64;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

/// A three-tuple roster whose provider covers every declared tuple.
const FAMILY_THREE_TUPLES: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32 || Width == 64;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8) + ScanProvider::scan<64>(9)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7) + self.service.scan<64>(8)) }
    }
"#;

/// The same partial roster with no boundary call at all: ineligibility is
/// per-requirement, so a conformance that is never dispatched dynamically
/// still settles its nongeneric siblings without publishing family rows.
const PARTIAL_COVERAGE_UNCALLED: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32 || Width == 64;
        machine ping(value: u32) -> u32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::ping(value: u32) -> u32 satisfies Scanner::ping {
        transition { _ -> (value) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u32 reaches Scanner {
        transition { _ -> (self.service.ping(2)) }
    }
"#;

/// A settled Unit family requirement keys statement-position calls by the
/// same canonical tuple as value calls.
const STATEMENT_FAMILY: &str = r#"
    boundary trait Watcher {
        machine watch<const Width: u32>(value: u32) where Width == 8 || Width == 16;
    }
    data WatchProvider {}
    machine WatchProvider::watch<const Width: u32>(value: u32) satisfies Watcher::watch {}
    machine WatchProvider::demand() {
        WatchProvider::watch<8>(1);
        WatchProvider::watch<16>(2);
    }
    data Client { service: Watcher; }
    machine Client::run(&mut self) reaches Watcher {
        self.service.watch<16>(1);
    }
"#;

fn family_fixture(source: &str) -> (CheckedTrees, Vec<ProviderPlan>) {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize finite-family fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse finite-family fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve finite-family fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type finite-family fixture");
    let plans = provider_planning::derive_satisfies_plans(&typed, None);
    let checked = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect("check finite-family fixture");
    (checked, plans)
}

fn requirement_symbol(checked: &CheckedTrees, name: &str) -> symbols::SymbolHandle {
    checked
        .traits()
        .iter()
        .flat_map(|definition| checked.trait_machine_signatures(definition))
        .find(|signature| signature.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing requirement `{name}`"))
        .symbol
}

fn family_rows(
    checked: &CheckedTrees,
    requirement: symbols::SymbolHandle,
) -> Vec<&checked_trees::CheckedBoundaryAdapterDispatch> {
    checked
        .facts
        .boundary_adapter_dispatch
        .iter()
        .filter(|row| row.requirement == requirement)
        .collect()
}

#[test]
fn declared_family_settles_one_row_per_demanded_tuple() {
    let (checked, plans) = family_fixture(FAMILY_SETTLES);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let ping = requirement_symbol(&checked, "ping");
    let original = Arc::new(checked);
    let mut settled = Arc::clone(&original);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("the declared family settles tuple rows beside the nongeneric sibling");
    assert_eq!(settled.typed, original.typed);

    // Rows repeat per retained receiver coordinate; the distinct tuple keys
    // are the roster.
    let mut tuples = family_rows(&settled, scan)
        .iter()
        .map(|row| row.family_tuple.as_ref().to_vec())
        .collect::<Vec<_>>();
    tuples.sort();
    tuples.dedup();
    assert_eq!(
        tuples,
        vec![
            vec!["named(integer-const(16))".to_owned()],
            vec!["named(integer-const(32))".to_owned()],
        ],
        "one row per demanded tuple, keyed by canonical const identity"
    );
    // Each tuple points at its own specialized entry state, not the template.
    let mut states = family_rows(&settled, scan)
        .iter()
        .map(|row| row.realization_state)
        .collect::<Vec<_>>();
    states.sort_by_key(|state| state.arena_index());
    states.dedup();
    assert_eq!(states.len(), 2, "tuples bind distinct realizations");
    assert!(
        family_rows(&settled, ping)
            .iter()
            .all(|row| row.family_tuple.is_empty()),
        "the nongeneric sibling stays exact"
    );
}

#[test]
fn family_roster_normalizes_order_and_duplicates() {
    let (checked, plans) = family_fixture(FAMILY_NORMALIZED);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("reordered duplicate alternatives normalize to the same roster");
    let mut tuples = family_rows(&settled, scan)
        .iter()
        .map(|row| row.family_tuple.as_ref().to_vec())
        .collect::<Vec<_>>();
    tuples.sort();
    tuples.dedup();
    assert_eq!(
        tuples,
        vec![
            vec!["named(integer-const(16))".to_owned()],
            vec!["named(integer-const(32))".to_owned()],
        ],
    );
}

#[test]
fn correlated_tuples_preserve_authored_groupings() {
    let (checked, plans) = family_fixture(CORRELATED_FAMILY);
    let selected = selected_plan(&plans, "Table");
    let lookup = requirement_symbol(&checked, "lookup");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("the correlated family settles its two declared tuples");
    let mut tuples = family_rows(&settled, lookup)
        .iter()
        .map(|row| row.family_tuple.as_ref().to_vec())
        .collect::<Vec<_>>();
    tuples.sort();
    tuples.dedup();
    assert_eq!(
        tuples,
        vec![
            vec![
                "named(integer-const(16))".to_owned(),
                "named(integer-const(4))".to_owned()
            ],
            vec![
                "named(integer-const(32))".to_owned(),
                "named(integer-const(8))".to_owned()
            ],
        ],
        "the roster preserves (16,4) and (32,8), not their Cartesian product"
    );
}

#[test]
fn off_roster_combination_rejects() {
    let (checked, plans) = family_fixture(CORRELATED_MISMATCH);
    let selected = selected_plan(&plans, "Table");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("(16,8) preserves neither authored correlation");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not select a settled tuple")),
        "{diagnostics:?}"
    );
}

#[test]
fn partial_tuples_stay_ineligible() {
    let (checked, plans) = family_fixture(PARTIAL_TUPLE);
    let selected = selected_plan(&plans, "Table");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("an alternative binding only some binders is not a tuple");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not bind every value binder")),
        "{diagnostics:?}"
    );
}

#[test]
fn opaque_predicates_do_not_enumerate() {
    let (checked, plans) = family_fixture(OPAQUE_FAMILY);
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("an opaque predicate is not an enumeration");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not explicit `Binder == literal` equalities")),
        "{diagnostics:?}"
    );
}

#[test]
fn inequality_ranges_do_not_enumerate() {
    let (checked, plans) = family_fixture(RANGE_FAMILY);
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("an inequality range is not an enumeration");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("not explicit `Binder == literal` equalities")),
        "{diagnostics:?}"
    );
}

#[test]
fn family_without_demanded_specializations_stays_ineligible() {
    let (checked, plans) = family_fixture(FAMILY_WITHOUT_SPECIALIZATIONS);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a roster with no demanded specialization supplies no rows");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("no checked provider specialization")),
        "{diagnostics:?}"
    );
    assert!(
        settled.facts.boundary_adapter_dispatch.is_empty()
            || settled
                .facts
                .boundary_adapter_dispatch
                .iter()
                .all(|row| row.requirement != scan),
        "no dispatch row exists for the unsettled family"
    );
}

#[test]
fn partial_provider_coverage_rejects_the_whole_family() {
    let (checked, plans) = family_fixture(PARTIAL_COVERAGE);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let original = Arc::new(checked);
    let mut rejected = Arc::clone(&original);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut rejected, &selected)
        .expect_err("an unrealized roster tuple cannot silently leave the family table");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("partial provider coverage")
            && diagnostic.message.contains("(64)")),
        "{diagnostics:?}"
    );
    // Even the realized tuple's call rejects: roster membership alone never
    // selects a row when the conformance cannot serve the whole family.
    assert!(Arc::ptr_eq(&original, &rejected));
    assert!(scan.is_valid());
}

#[test]
fn complete_roster_settles_every_declared_tuple() {
    let (checked, plans) = family_fixture(FAMILY_THREE_TUPLES);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a fully covered three-tuple roster settles");
    let mut tuples = family_rows(&settled, scan)
        .iter()
        .map(|row| row.family_tuple.as_ref().to_vec())
        .collect::<Vec<_>>();
    tuples.sort();
    tuples.dedup();
    assert_eq!(
        tuples,
        vec![
            vec!["named(integer-const(16))".to_owned()],
            vec!["named(integer-const(32))".to_owned()],
            vec!["named(integer-const(64))".to_owned()],
        ],
        "one row per declared tuple, each with its own specialization"
    );
    let mut states = family_rows(&settled, scan)
        .iter()
        .map(|row| row.realization_state)
        .collect::<Vec<_>>();
    states.sort_by_key(|state| state.arena_index());
    states.dedup();
    assert_eq!(states.len(), 3, "each tuple binds its own realization");
}

#[test]
fn uncalled_partial_family_supplies_no_rows_but_siblings_settle() {
    let (checked, plans) = family_fixture(PARTIAL_COVERAGE_UNCALLED);
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let ping = requirement_symbol(&checked, "ping");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("an uncalled ineligible family does not block its siblings");
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .all(|row| row.requirement != scan),
        "partial coverage publishes no truncated family rows"
    );
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .any(|row| row.requirement == ping),
        "the nongeneric sibling keeps its exact row"
    );
}

#[test]
fn statement_calls_select_the_settled_tuple() {
    let (checked, plans) = family_fixture(STATEMENT_FAMILY);
    let selected = selected_plan(&plans, "Watcher");
    let watch = requirement_symbol(&checked, "watch");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("the statement-call family settles its declared roster");
    let mut tuples = family_rows(&settled, watch)
        .iter()
        .map(|row| row.family_tuple.as_ref().to_vec())
        .collect::<Vec<_>>();
    tuples.sort();
    tuples.dedup();
    assert_eq!(
        tuples,
        vec![
            vec!["named(integer-const(16))".to_owned()],
            vec!["named(integer-const(8))".to_owned()],
        ],
        "statement calls see the same normalized roster as value calls"
    );
}
