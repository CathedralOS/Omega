//! Transparent refinement `reaches` clauses narrow the base requirement's
//! normalized reach row: every named service must already appear in every
//! covered base row, and the clause owns its binding at clause location —
//! omitted `reaches` inherits, `reaches;` is the empty row, named services
//! intern a concrete row, and `reaches _;` mints one independent abstract
//! row per covered requirement bounded by its inherited row.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn lower(source: &str) -> Result<typed_trees::TypedTrees, diagnostics::Diagnostic> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    lower_symbol_resolved_trees(&resolved)
}

#[test]
fn clause_reach_inside_base_row_compiles() {
    let source = r#"
boundary trait Handler { machine handle(); }
boundary trait EventLog { machine emit(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches Handler;
}
"#;
    lower(source).expect("a clause reach member of the base row narrows");
}

#[test]
fn clause_reach_outside_base_row_rejects() {
    let source = r#"
boundary trait Handler { machine handle(); }
boundary trait EventLog { machine emit(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches EventLog;
}
"#;
    let diagnostic = lower(source).expect_err("EventLog is not in the base row");
    let message = diagnostic.to_string();
    assert!(
        message.contains("does not reach it"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn clause_reach_unknown_service_rejects() {
    let source = r#"
boundary trait Handler { machine handle(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches Missing;
}
"#;
    let diagnostic = lower(source).expect_err("Missing names no boundary service");
    let message = diagnostic.to_string();
    assert!(
        message.contains("not a boundary service"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn wildcard_clause_reach_must_fit_every_covered_requirement() {
    let source = r#"
boundary trait Handler { machine handle(); }
boundary trait EventLog { machine emit(); }

trait Logger {
    machine write(&mut self) reaches Handler;
    machine flush(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine * reaches EventLog;
}
"#;
    let diagnostic =
        lower(source).expect_err("a `machine *` clause narrows every covered requirement");
    let message = diagnostic.to_string();
    assert!(
        message.contains("does not reach it"),
        "unexpected diagnostic: {message}"
    );
}

#[test]
fn named_clause_reach_interns_a_concrete_row() {
    let source = r#"
boundary trait Handler { machine handle(); }
boundary trait EventLog { machine emit(); }

trait Logger {
    machine write(&mut self) reaches Handler + EventLog;
}

trait LocalLogger = Logger {
    machine Logger::write reaches Handler;
}
"#;
    let typed = lower(source).expect("a subset reach narrows");
    let clause = local_logger_clause(&typed, "write");
    let typed_trees::trait_definition::TraitRefinementReach::Concrete(row) = &clause.service_reach
    else {
        panic!("named reaches bind a concrete clause-location row")
    };
    let services = typed.service_reach_rows.services(*row);
    assert_eq!(
        services.len(),
        1,
        "clause row holds exactly the named reach"
    );
}

#[test]
fn authored_empty_clause_reach_narrows_to_the_empty_row() {
    let source = r#"
boundary trait Handler { machine handle(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches;
}
"#;
    let typed = lower(source).expect("authored `reaches;` narrows to empty");
    let clause = local_logger_clause(&typed, "write");
    assert_eq!(
        clause.service_reach,
        typed_trees::trait_definition::TraitRefinementReach::Concrete(
            language_semantics::ServiceReachRowTable::EMPTY_ROW
        ),
        "authored `reaches;` is the concrete empty row, not inheritance"
    );
}

#[test]
fn omitted_clause_reach_inherits_the_base_row() {
    let source = r#"
boundary trait Handler { machine handle(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write suspends false;
}
"#;
    let typed = lower(source).expect("an omitted reaches clause inherits");
    let clause = local_logger_clause(&typed, "write");
    assert_eq!(
        clause.service_reach,
        typed_trees::trait_definition::TraitRefinementReach::Inherited,
        "omitted reaches leaves the base row in place"
    );
}

#[test]
fn wildcard_clause_reach_mints_an_abstract_row_bounded_by_the_base() {
    let source = r#"
boundary trait Handler { machine handle(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches _;
}
"#;
    let typed = lower(source).expect("`reaches _;` mints the independent abstract row");
    let clause = local_logger_clause(&typed, "write");
    let typed_trees::trait_definition::TraitRefinementReach::IndependentBounded(rows) =
        &clause.service_reach
    else {
        panic!("`reaches _;` binds per-requirement abstract rows")
    };
    let [entry] = rows.as_slice() else {
        panic!("one covered requirement mints one abstract row")
    };
    assert_eq!(entry.requirement.as_str(), "write");
    assert!(
        typed.service_reach_rows.is_abstract(entry.row),
        "the minted row is abstract"
    );
    let base = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Logger")
        .expect("Logger trait");
    let write = typed
        .trait_machine_signatures(base)
        .iter()
        .find(|machine| machine.name.as_str() == "write")
        .expect("write requirement");
    assert_eq!(
        typed.service_reach_rows.abstract_bound(entry.row),
        Some(write.service_reach_row),
        "the abstract row is bounded by the requirement's inherited row"
    );
}

#[test]
fn wildcard_star_clause_mints_independent_rows_per_requirement() {
    let source = r#"
boundary trait Handler { machine handle(); }
boundary trait EventLog { machine emit(); }

trait Logger {
    machine write(&mut self) reaches Handler;
    machine flush(&mut self) reaches EventLog;
}

trait LocalLogger = Logger {
    machine * reaches _;
}
"#;
    let typed = lower(source).expect("`*` + `reaches _;` mints one row each");
    let local = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "LocalLogger")
        .expect("LocalLogger trait");
    let typed_trees::trait_definition::TraitRefinementReach::IndependentBounded(rows) =
        &local.refinement_clauses[0].service_reach
    else {
        panic!("`machine *` + `reaches _;` binds per-requirement rows")
    };
    assert_eq!(rows.len(), 2, "each covered requirement gets its own row");
    assert_ne!(
        rows[0].row, rows[1].row,
        "abstract rows do not correlate requirements"
    );
}

#[test]
fn wildcard_reach_does_not_combine_with_named_services() {
    let source = r#"
boundary trait Handler { machine handle(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches _ + Handler;
}
"#;
    let diagnostic = lower(source).expect_err("`_` does not combine with names");
    let message = diagnostic.to_string();
    assert!(
        message.contains("independent abstract row"),
        "unexpected diagnostic: {message}"
    );
}

fn local_logger_clause<'a>(
    typed: &'a typed_trees::TypedTrees,
    requirement: &str,
) -> &'a typed_trees::trait_definition::TraitRefinementClause {
    let local = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "LocalLogger")
        .expect("LocalLogger trait");
    local
        .refinement_clauses
        .iter()
        .find(|clause| {
            clause
                .requirement
                .as_ref()
                .is_some_and(|name| name.as_str() == requirement)
        })
        .expect("clause covering the requirement")
}
