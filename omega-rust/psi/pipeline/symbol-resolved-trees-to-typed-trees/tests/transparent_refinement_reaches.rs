//! Transparent refinement `reaches` clauses narrow the base requirement's
//! normalized reach row: every named service must already appear in every
//! covered base row. `reaches _;` is the independent abstract row bounded by
//! the inherited row (its clause-location variant is still pending) and stays
//! retained-only here.

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
fn clause_reach_wildcard_stays_retained_pending_the_abstract_row() {
    let source = r#"
boundary trait Handler { machine handle(); }

trait Logger {
    machine write(&mut self) reaches Handler;
}

trait LocalLogger = Logger {
    machine Logger::write reaches _;
}
"#;
    lower(source).expect("`reaches _;` is the pending independent abstract row");
}
