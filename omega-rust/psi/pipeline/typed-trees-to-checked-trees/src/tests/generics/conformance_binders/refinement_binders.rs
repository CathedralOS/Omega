//! A static evidence binder may require a transparent refinement and receive
//! an explicitly selected base conformance "whose complete contract fits"
//! (`wiki/spec/language/conformances.md`, Transparent refinements). The
//! carrier resolves through `refines`; the selected conformance's rows are
//! then checked against the refinement's clauses.

use crate::CheckingRequest;
use crate::tests::{Lexer, lower_symbol_resolved_trees, lower_typed_trees, parse_syntax_trees};
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};

fn check_source(source: &str) -> Result<checked_trees::CheckedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize should succeed");
    let syntax = parse_syntax_trees(&tokens).expect("parse should succeed");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolution should succeed");
    let typed = lower_symbol_resolved_trees(&resolved).expect("typing should succeed");
    lower_typed_trees(typed, &CheckingRequest::settled())
}

fn rejection(source: &str, expectation: &str) -> Vec<String> {
    match check_source(source) {
        Ok(_) => panic!("{expectation}"),
        Err(diagnostics) => diagnostics
            .iter()
            .map(|diagnostic| diagnostic.to_string())
            .collect(),
    }
}

/// `suspends`/`blocks` narrowing: the base permits both, the refinement
/// removes both, and the selected realization uses neither.
fn suspending_base_source(realization_axes: &str) -> String {
    format!(
        r#"
        trait Logger {{
            machine write(&mut self) suspends; blocks;
        }}

        trait LocalLogger = Logger {{
            machine * suspends false; blocks false;
        }}

        data Sink {{ pending: i32; }}

        machine Sink::write(&mut self) {realization_axes} {{
            self.pending = 0;
        }}

        SinkLogger: Sink satisfies Logger {{
            Logger::write = Sink::write;
        }}

        machine run<Element, Log: Element satisfies LocalLogger>(value: &mut Element) {{
            Log::write(value);
        }}

        machine caller(sink: &mut Sink) {{
            run<Sink, SinkLogger>(sink);
        }}
    "#
    )
}

#[test]
fn refinement_binder_admits_an_explicitly_selected_base_conformance_that_fits() {
    check_source(&suspending_base_source(""))
        .expect("a `Logger` conformance whose rows neither suspend nor block fits `LocalLogger`");
}

#[test]
fn refinement_binder_rejects_a_realization_that_suspends() {
    let diagnostics = rejection(
        &suspending_base_source("suspends;"),
        "a realization that suspends does not fit a refinement that removed `suspends`",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` removes `suspends` from `write`")
                && diagnostic.contains("`SinkLogger`")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn refinement_binder_rejects_a_realization_that_blocks() {
    let diagnostics = rejection(
        &suspending_base_source("blocks;"),
        "a realization that blocks does not fit a refinement that removed `blocks`",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` removes `blocks` from `write`")
        }),
        "{diagnostics:#?}"
    );
}

/// `reaches;` is the authored-empty row. A realization that still reaches a
/// boundary service escapes it.
fn narrowed_reach_source(realization_reach: &str, refinement_reach: &str) -> String {
    format!(
        r#"
        pub boundary trait Console {{
            machine emit(code: i32) reaches Console;
        }}

        trait Logger {{
            machine write(&mut self) reaches Console;
        }}

        trait LocalLogger = Logger {{
            machine * {refinement_reach}
        }}

        data Sink {{ pending: i32; }}

        machine Sink::write(&mut self) {realization_reach} {{
            Console::emit(7);
            self.pending = 0;
        }}

        SinkLogger: Sink satisfies Logger {{
            Logger::write = Sink::write;
        }}

        machine run<Element, Log: Element satisfies LocalLogger>(value: &mut Element) {{
            Log::write(value);
        }}

        machine caller(sink: &mut Sink) {{
            run<Sink, SinkLogger>(sink);
        }}
    "#
    )
}

#[test]
fn refinement_binder_rejects_a_realization_that_escapes_a_narrowed_reach() {
    let diagnostics = rejection(
        &narrowed_reach_source("reaches Console", "reaches;"),
        "a realization reaching `Console` does not fit `reaches;`",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` narrows `reaches` on `write`")
                && diagnostic.contains("reaches `Console`")
        }),
        "{diagnostics:#?}"
    );
}

#[test]
fn an_inherited_reach_axis_imposes_no_obligation_at_the_concrete_site() {
    // "Omission here means inheritance": a clause that authors no `reaches`
    // leaves the base row in force and binds nothing extra here.
    check_source(&narrowed_reach_source("reaches Console", "suspends false;"))
        .expect("an omitted `reaches` clause inherits the base row");
}

/// "A targeted clause names one exact requirement. Unmentioned requirements
/// and contract axes inherit the base." A targeted clause therefore replaces
/// the wildcard for its requirement rather than composing with it.
fn targeted_clause_source(write_axes: &str, flush_axes: &str) -> String {
    format!(
        r#"
        trait Logger {{
            machine write(&mut self) suspends;
            machine flush(&mut self) suspends;
        }}

        trait LocalLogger = Logger {{
            machine * suspends false;
            machine Logger::flush blocks false;
        }}

        data Sink {{ pending: i32; }}

        machine Sink::write(&mut self) {write_axes} {{ self.pending = 0; }}
        machine Sink::flush(&mut self) {flush_axes} {{ self.pending = 1; }}

        SinkLogger: Sink satisfies Logger {{
            Logger::write = Sink::write;
            Logger::flush = Sink::flush;
        }}

        machine run<Element, Log: Element satisfies LocalLogger>(value: &mut Element) {{
            Log::write(value);
            Log::flush(value);
        }}

        machine caller(sink: &mut Sink) {{
            run<Sink, SinkLogger>(sink);
        }}
    "#
    )
}

#[test]
fn a_targeted_clause_replaces_the_wildcard_for_its_own_requirement() {
    // `flush` is covered by its targeted clause, which authors only `blocks`;
    // its `suspends` axis inherits the base, which permits suspension.
    check_source(&targeted_clause_source("", "suspends;"))
        .expect("the targeted `flush` clause inherits the base `suspends` axis");
    // `write` is covered only by the wildcard, which removed `suspends`.
    let diagnostics = rejection(
        &targeted_clause_source("suspends;", ""),
        "the wildcard still covers `write`",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` removes `suspends` from `write`")
        }),
        "{diagnostics:#?}"
    );
}

/// An authored `terminates` strengthens the guarantee, and only a PUBLISHED
/// promise carries it. There is no `terminates false` spelling; omission is
/// inheritance.
fn terminating_source(realization_axes: &str) -> String {
    format!(
        r#"
        trait Logger {{
            machine write(&mut self);
        }}

        trait LocalLogger = Logger {{
            machine * terminates;
        }}

        data Sink {{ pending: i32; }}

        machine Sink::write(&mut self) {realization_axes} {{ self.pending = 0; }}

        SinkLogger: Sink satisfies Logger {{
            Logger::write = Sink::write;
        }}

        machine run<Element, Log: Element satisfies LocalLogger>(value: &mut Element) {{
            Log::write(value);
        }}

        machine caller(sink: &mut Sink) {{
            run<Sink, SinkLogger>(sink);
        }}
    "#
    )
}

#[test]
fn refinement_binder_requires_a_published_termination_guarantee() {
    let diagnostics = rejection(
        &terminating_source(""),
        "a privately derived body publishes no termination guarantee",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` requires `terminates` on `write`")
        }),
        "{diagnostics:#?}"
    );
    check_source(&terminating_source("terminates;"))
        .expect("a realization publishing `terminates` fits the strengthened guarantee");
}
