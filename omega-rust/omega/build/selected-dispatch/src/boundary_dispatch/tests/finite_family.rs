use super::{
    Arc, CheckedTrees, ProviderPlan, selected_plan, settle_selected_boundary_adapter_dispatch,
};
use provider_planning::ProviderPlanDerivation;
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

/// A roster with no static call demand at all: selection alone commits the
/// provider to the complete family, so checking generates every tuple.
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

/// The provider's static calls demand only (16) and (32); the declared (64)
/// is still generated from selection, and deleting that retained record
/// leaves the family ineligible rather than settling a silently truncated
/// table.
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

/// A roster whose boundary calls name only the nongeneric sibling: the
/// uncalled family still supplies its complete generated roster.
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

fn family_typed(source: &str) -> (typed_trees::TypedTrees, Vec<ProviderPlan>) {
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
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    (typed, plans)
}

/// Check a fixture the way settled orchestration does: the boundary family
/// demands its selected surface commits to are carried into checking, so the
/// selected provider's complete roster is materialized before any dispatch
/// row resolution consults it.
fn family_checked(
    typed: typed_trees::TypedTrees,
    selected: &effects::SelectedProviderPlanFacts,
) -> CheckedTrees {
    let demands =
        crate::boundary_dispatch::selected_boundary_family_specializations(&typed, selected);
    typed_trees_to_checked_trees::lower_typed_trees_with_selected_generic_operator_providers(
        typed,
        &[],
        &demands,
        &[],
    )
    .expect("check finite-family fixture")
}

fn family_fixture(source: &str, schema: &str) -> (CheckedTrees, Vec<ProviderPlan>) {
    let (typed, plans) = family_typed(source);
    let selected = selected_plan(&plans, schema);
    (family_checked(typed, &selected), plans)
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
    let (checked, plans) = family_fixture(FAMILY_SETTLES, "Scanner");
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
    let (checked, plans) = family_fixture(FAMILY_NORMALIZED, "Scanner");
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
    let (checked, plans) = family_fixture(CORRELATED_FAMILY, "Table");
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
    let (checked, plans) = family_fixture(CORRELATED_MISMATCH, "Table");
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
    let (checked, plans) = family_fixture(PARTIAL_TUPLE, "Table");
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
    let (checked, plans) = family_fixture(OPAQUE_FAMILY, "Scanner");
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
    let (checked, plans) = family_fixture(RANGE_FAMILY, "Scanner");
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
fn a_selected_family_settles_without_any_static_call_demand() {
    // Nothing statically calls either roster tuple; the selected adapter row
    // commits to the complete family anyway, so checking generates both.
    let (checked, plans) = family_fixture(FAMILY_WITHOUT_SPECIALIZATIONS, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("selection alone materializes the provider's complete roster");
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
        "every declared tuple gains a dispatch row without a static call site"
    );
}

#[test]
fn the_selected_family_supplies_undemanded_roster_tuples() {
    // (64) has no static call site anywhere; selection alone materializes it.
    let (checked, plans) = family_fixture(PARTIAL_COVERAGE, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("the complete roster settles from selection alone");
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
        "the undemanded (64) tuple settles beside the statically demanded ones"
    );
}

#[test]
fn a_missing_roster_body_still_rejects_the_whole_family() {
    let (typed, plans) = family_typed(PARTIAL_COVERAGE);
    let selected = selected_plan(&plans, "Scanner");
    let mut checked = family_checked(typed, &selected);
    let scan = requirement_symbol(&checked, "scan");
    // Delete the generated (64) member: roster membership alone never fills a
    // row, and the conformance that cannot serve the whole family rejects.
    checked
        .typed
        .machine_specializations
        .retain(|specialization| {
            specialization.const_argument_identities != ["named(integer-const(64))".to_owned()]
        });
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a lost roster member cannot silently leave the family table");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("partial provider coverage")
            && diagnostic.message.contains("(64)")),
        "{diagnostics:?}"
    );
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .all(|row| row.requirement != scan),
        "no dispatch row exists for the truncated family"
    );
}

#[test]
fn complete_roster_settles_every_declared_tuple() {
    let (checked, plans) = family_fixture(FAMILY_THREE_TUPLES, "Scanner");
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
fn an_uncalled_family_still_supplies_its_complete_roster() {
    let (checked, plans) = family_fixture(PARTIAL_COVERAGE_UNCALLED, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let ping = requirement_symbol(&checked, "ping");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("the uncalled family settles its complete generated roster");
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
        "selection supplies every declared tuple even with no family call"
    );
    assert!(
        family_rows(&settled, ping)
            .iter()
            .all(|row| row.family_tuple.is_empty()),
        "the nongeneric sibling keeps its exact row"
    );
}

#[test]
fn statement_calls_select_the_settled_tuple() {
    let (checked, plans) = family_fixture(STATEMENT_FAMILY, "Watcher");
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

/// The roster bounds only the dynamic family: `direct` may still specialize
/// the provider at `scan<64>` for ordinary static use, but a boundary call
/// demanding a width outside the declared roster selects no row.
const OFF_ROSTER_WIDTH: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
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
        transition { _ -> (self.service.scan<64>(7)) }
    }
"#;

/// Two providers conform to the same requirement. ScanProvider's family is
/// generated from selection while ReserveProvider's static calls retain
/// same-tuple records of their own. A family row is filled only by the
/// selected conformance's retained specializations — a sibling provider's
/// record at the identical tuple can never lend coverage.
const SIBLING_PROVIDERS: &str = r#"
    boundary trait Scanner {
        machine scan<const Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7)) }
    }
    data ReserveProvider {}
    machine ReserveProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ReserveProvider::direct() -> u64 {
        transition { _ -> (ReserveProvider::scan<16>(7) + ReserveProvider::scan<32>(8)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

#[test]
fn off_roster_width_selects_no_settled_tuple() {
    let (checked, plans) = family_fixture(OFF_ROSTER_WIDTH, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a width outside the declared roster selects no row");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not select a settled tuple")),
        "{diagnostics:?}"
    );
}

/// Mutating the retained specialization record exercises the row-selection
/// guards against drifted evidence: the record's const identities are the
/// only witness that the instance was realized at the roster tuple, so a
/// record claiming a different width can never fill the row.
#[test]
fn wrong_width_specialization_never_fills_a_roster_row() {
    let (mut checked, plans) = family_fixture(FAMILY_SETTLES, "Scanner");
    checked
        .typed
        .machine_specializations
        .iter_mut()
        .find(|specialization| {
            specialization.const_argument_identities == ["named(integer-const(32))".to_owned()]
        })
        .expect("the (32) specialization")
        .const_argument_identities = vec!["named(integer-const(64))".to_owned()];
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a record realized at width 64 cannot fill the (32) row");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("partial provider coverage")
            && diagnostic.message.contains("(32)")),
        "{diagnostics:?}"
    );
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .all(|row| row.requirement != scan),
        "no dispatch row exists for the wrong-width family"
    );
}

/// A specialization record carrying anything beside the bare value tuple —
/// type, machine, or conformance arguments, or an over-long const tuple —
/// is a substituted shape, not a roster member: the row stays unfilled and
/// the family rejects rather than keying dispatch on a partial identity.
#[test]
fn shape_substituted_specializations_never_fill_roster_rows() {
    for substitution in [
        "type argument",
        "machine argument",
        "conformance argument",
        "inferred conformance argument",
        "conformance application",
        "extra const argument",
    ] {
        let (mut checked, plans) = family_fixture(FAMILY_SETTLES, "Scanner");
        let specialization = checked
            .typed
            .machine_specializations
            .iter_mut()
            .find(|specialization| {
                specialization.const_argument_identities == ["named(integer-const(32))".to_owned()]
            })
            .expect("the (32) specialization");
        match substitution {
            "type argument" => specialization
                .type_argument_identities
                .push("named(type(u32))".to_owned()),
            "machine argument" => specialization
                .machine_arguments
                .push(symbols::SymbolHandle::from_parts(7, 0)),
            "conformance argument" => specialization
                .conformance_arguments
                .push(symbols::SymbolHandle::from_parts(7, 0)),
            "inferred conformance argument" => specialization
                .inferred_conformance_arguments
                .push(symbols::SymbolHandle::from_parts(7, 0)),
            "conformance application" => specialization
                .conformance_applications
                .push(typed_trees::typed_trees::ClosedConformanceApplication::default()),
            "extra const argument" => specialization
                .const_argument_identities
                .push("named(integer-const(1))".to_owned()),
            _ => unreachable!(),
        }
        let selected = selected_plan(&plans, "Scanner");
        let mut settled = Arc::new(checked);
        let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
            .expect_err("a substituted shape cannot fill the (32) row");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("partial provider coverage")
                && diagnostic.message.contains("(32)")),
            "{substitution}: {diagnostics:?}"
        );
    }
}

/// Two retained records claiming the same template's same tuple would make
/// the row ambiguous; the family rejects rather than silently publishing
/// whichever record happens to sort first.
#[test]
fn duplicate_tuple_specializations_reject() {
    let (mut checked, plans) = family_fixture(FAMILY_SETTLES, "Scanner");
    let duplicate = checked
        .typed
        .machine_specializations
        .iter()
        .find(|specialization| {
            specialization.const_argument_identities == ["named(integer-const(32))".to_owned()]
        })
        .expect("the (32) specialization")
        .clone();
    checked.typed.machine_specializations.push(duplicate);
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("two records for one tuple can never settle a row");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("multiple retained specializations")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_sibling_providers_specialization_never_fills_the_row() {
    let (typed, plans) = family_typed(SIBLING_PROVIDERS);
    let scan_provider = plans
        .iter()
        .find(|plan| plan.provider_type == "ScanProvider")
        .expect("ScanProvider plan");
    let selected =
        effects::SelectedProviderPlanFacts::from_selected_plans(vec![scan_provider.clone()])
            .expect("select ScanProvider's plan");
    let mut checked = family_checked(typed, &selected);
    // ScanProvider's own family was generated from selection; removing its
    // (32) record leaves only ReserveProvider's same-tuple record, which can
    // never lend coverage to the selected conformance.
    let scan_template = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ScanProvider::scan")
        .expect("ScanProvider::scan template")
        .symbol;
    checked
        .typed
        .machine_specializations
        .retain(|specialization| {
            !(specialization.template == scan_template
                && specialization.const_argument_identities
                    == ["named(integer-const(32))".to_owned()])
        });
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("the sibling's (32) record cannot lend ScanProvider coverage");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("partial provider coverage")
            && diagnostic.message.contains("(32)")),
        "{diagnostics:?}"
    );
}

/// A runtime-capable `Value`-binder requirement spells `<Width: u32>` instead
/// of `<const Width: u32>`. Its roster enumerates the same canonical const
/// tuples, and static boundary calls select exactly the same settled rows —
/// the requirement binder's runtime capability never relaxes a static
/// application into a different family.
const FAMILY_VALUE_BINDER_SETTLES: &str = r#"
    boundary trait Scanner {
        machine scan<Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
        machine ping(value: u32) -> u32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
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

/// A `Value`-binder requirement called with a runtime argument names no
/// roster tuple: the boundary keeps only tuple-keyed rows, so the call
/// rejects instead of dispatching every width to an unbound selection.
const FAMILY_VALUE_BINDER_RUNTIME_ARGUMENT: &str = r#"
    boundary trait Scanner {
        machine scan<Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self, width: u32) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<width>(7)) }
    }
"#;

/// The runtime-capable spelling pairs only with a runtime-capable provider
/// binder: a `const` provider under a `Value` requirement is a kind mismatch,
/// never a compatible specialization.
const FAMILY_VALUE_REQUIREMENT_CONST_PROVIDER: &str = r#"
    boundary trait Scanner {
        machine scan<Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<const Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
"#;

#[test]
fn value_binder_requirement_settles_tuple_rows() {
    let (checked, plans) = family_fixture(FAMILY_VALUE_BINDER_SETTLES, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let scan = requirement_symbol(&checked, "scan");
    let ping = requirement_symbol(&checked, "ping");
    let mut settled = Arc::new(checked);
    settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect("a Value-binder requirement settles one row per declared tuple");
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
        "Value and const spellings key the roster identically"
    );
    assert!(
        family_rows(&settled, ping)
            .iter()
            .all(|row| row.family_tuple.is_empty()),
        "the nongeneric sibling stays exact"
    );
}

#[test]
fn value_binder_requirement_rejects_a_runtime_argument() {
    let (checked, plans) = family_fixture(FAMILY_VALUE_BINDER_RUNTIME_ARGUMENT, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a runtime width names no declared tuple to dispatch");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("not a closed const value")),
        "{diagnostics:?}"
    );
}

/// A static application outside the declared roster is a membership failure
/// under either binder spelling.
const FAMILY_VALUE_BINDER_OFF_ROSTER: &str = r#"
    boundary trait Scanner {
        machine scan<Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::direct() -> u64 {
        transition { _ -> (ScanProvider::scan<16>(7) + ScanProvider::scan<32>(8) + ScanProvider::scan<64>(9)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<64>(7)) }
    }
"#;

/// A runtime-bound provider specialization is not roster evidence: its record
/// carries no closed const tuple, so deleting a generated roster member
/// leaves the family ineligible even though the provider was exercised at
/// every listed width shape.
const FAMILY_VALUE_BINDER_RUNTIME_DEMAND: &str = r#"
    boundary trait Scanner {
        machine scan<Width: u32>(value: u32) -> u64 where Width == 16 || Width == 32;
    }
    data ScanProvider {}
    machine ScanProvider::scan<Width: u32>(value: u32) -> u64 satisfies Scanner::scan {
        transition { _ -> (value as u64) }
    }
    machine ScanProvider::direct(width: u32) -> u64 {
        transition { _ -> (ScanProvider::scan<width>(7)) }
    }
    data Client { service: Scanner; }
    machine Client::run(&mut self) -> u64 reaches Scanner {
        transition { _ -> (self.service.scan<16>(7)) }
    }
"#;

#[test]
fn value_binder_requirement_rejects_an_off_roster_static_argument() {
    let (checked, plans) = family_fixture(FAMILY_VALUE_BINDER_OFF_ROSTER, "Scanner");
    let selected = selected_plan(&plans, "Scanner");
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("a static width outside the declared roster selects no row");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("does not select a settled tuple")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_runtime_bound_record_never_fills_a_roster_row() {
    let (typed, plans) = family_typed(FAMILY_VALUE_BINDER_RUNTIME_DEMAND);
    let selected = selected_plan(&plans, "Scanner");
    let mut checked = family_checked(typed, &selected);
    let scan = requirement_symbol(&checked, "scan");
    let scan_template = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "ScanProvider::scan")
        .expect("ScanProvider::scan template")
        .symbol;
    // Removing the generated (32) roster member leaves only the runtime-bound
    // record, which carries no closed const tuple and cannot fill the row.
    checked
        .typed
        .machine_specializations
        .retain(|specialization| {
            !(specialization.template == scan_template
                && specialization.const_argument_identities
                    == ["named(integer-const(32))".to_owned()])
        });
    let mut settled = Arc::new(checked);
    let diagnostics = settle_selected_boundary_adapter_dispatch(&mut settled, &selected)
        .expect_err("the runtime-bound record is not roster evidence");
    assert!(
        diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("partial provider coverage")
            && diagnostic.message.contains("(32)")),
        "{diagnostics:?}"
    );
    assert!(
        settled
            .facts
            .boundary_adapter_dispatch
            .iter()
            .all(|row| row.requirement != scan),
        "no dispatch row exists for the runtime-bound family"
    );
}

#[test]
fn value_requirement_rejects_a_const_provider_binder() {
    let tokens = source_files_to_tokens::Lexer::new(FAMILY_VALUE_REQUIREMENT_CONST_PROVIDER)
        .tokenize()
        .expect("tokenize kind-mismatch fixture");
    let syntax =
        tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse kind-mismatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve kind-mismatch fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type kind-mismatch fixture");
    let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
        .expect_err("a const provider binder cannot satisfy a Value requirement");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("incompatible kinds")),
        "{diagnostics:?}"
    );
}

#[test]
fn one_boundary_slot_admits_one_selected_conformance() {
    let (_typed, plans) = family_typed(SIBLING_PROVIDERS);
    let scanner_plans = plans
        .iter()
        .filter(|plan| plan.schema.trait_name == "Scanner")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        scanner_plans.len(),
        2,
        "each provider type derives its own plan for the trait"
    );
    let diagnostic = effects::SelectedProviderPlanFacts::from_selected_plans(scanner_plans)
        .expect_err("two conformances for one boundary slot must reject");
    assert!(
        diagnostic.contains("more than one selected provider plan"),
        "{diagnostic}"
    );
}
