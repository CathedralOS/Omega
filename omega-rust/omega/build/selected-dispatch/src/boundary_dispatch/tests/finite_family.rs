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
