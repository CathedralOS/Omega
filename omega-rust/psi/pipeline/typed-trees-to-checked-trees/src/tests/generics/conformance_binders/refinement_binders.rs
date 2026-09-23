//! A static evidence binder may require a transparent refinement and receive
//! an explicitly selected base conformance "whose complete contract fits"
//! (`wiki/spec/language/conformances.md`, Transparent refinements). The
//! carrier resolves through `refines`; the selected conformance's rows are
//! then checked against the refinement's clauses.

use crate::tests::front_end::checked_program_result;

fn rejection(source: &str, expectation: &str) -> Vec<String> {
    match checked_program_result(source) {
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
    checked_program_result(&suspending_base_source(""))
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
    checked_program_result(&narrowed_reach_source("reaches Console", "suspends false;"))
        .expect("an omitted `reaches` clause inherits the base row");
}

/// "`machine *` applies to every present and future base requirement. A
/// targeted clause names one exact requirement." Both therefore bound
/// `flush`: the wildcard removes `suspends` from every requirement and the
/// targeted clause additionally removes `blocks`. "Multiple refinements
/// combine by an order-independent meet" — a targeted clause narrows
/// alongside the wildcard rather than replacing it.
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
fn a_targeted_clause_narrows_alongside_the_wildcard_it_does_not_replace_it() {
    // Neither realization suspends or blocks, so both clauses are satisfied.
    checked_program_result(&targeted_clause_source("", ""))
        .expect("a realization violating no covering clause fits");

    // `flush` carries its targeted clause AND the wildcard. Suspending
    // violates the wildcard's `suspends false`, which a targeted clause
    // naming another axis cannot discard.
    let diagnostics = rejection(
        &targeted_clause_source("", "suspends;"),
        "the wildcard still covers a requirement that has its own clause",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` removes `suspends` from `flush`")
        }),
        "{diagnostics:#?}"
    );

    // The targeted clause's own axis still binds.
    let diagnostics = rejection(
        &targeted_clause_source("", "blocks;"),
        "the targeted `flush` clause removes `blocks`",
    );
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.contains("refinement `LocalLogger` removes `blocks` from `flush`")
        }),
        "{diagnostics:#?}"
    );

    // `write` is covered only by the wildcard.
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
    checked_program_result(&terminating_source("terminates;"))
        .expect("a realization publishing `terminates` fits the strengthened guarantee");
}

/// A refinement head may reorder or partially apply its base's arguments.
/// The binder names the REFINEMENT's parameters while the selected
/// conformance satisfies the BASE, so `refines.arguments` has to be
/// instantiated before the two are compared.
fn reordered_head_source(conformance_arguments: &str) -> String {
    format!(
        r#"
        trait Pair<A, B> {{
            machine write(&mut self);
        }}

        trait Flipped<X, Y> = Pair<Y, X> {{
            machine * terminates;
        }}

        data Sink {{ pending: i32; }}

        machine Sink::write(&mut self) terminates {{ self.pending = 0; }}

        SinkPair: Sink satisfies Pair<{conformance_arguments}> {{
            Pair::write = Sink::write;
        }}

        machine run<Element, P: Element satisfies Flipped<bool, i32>>(value: &mut Element) {{
            P::write(value);
        }}

        machine caller(sink: &mut Sink) {{
            run<Sink, SinkPair>(sink);
        }}
    "#
    )
}

#[test]
fn a_reordered_refinement_head_instantiates_before_comparing() {
    // `Flipped<bool, i32>` binds X=bool, Y=i32, so `= Pair<Y, X>` demands
    // `Pair<i32, bool>`. Comparing the binder's own arguments instead would
    // look for `Pair<bool, i32>` and reject this fitting conformance.
    checked_program_result(&reordered_head_source("i32, bool"))
        .expect("the instantiated head selects `Pair<i32, bool>`");

    // The instantiation must still discriminate: the unflipped arguments are
    // exactly what the old unexpanded comparison would have accepted.
    let diagnostics = rejection(
        &reordered_head_source("bool, i32"),
        "an unflipped conformance does not satisfy the instantiated head",
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.contains("cannot bind `P` to conformance `SinkPair`") }),
        "{diagnostics:#?}"
    );
}

/// "A targeted clause names one exact requirement" (spec,
/// `language/conformances.md`, Transparent refinements). A base trait may
/// carry same-named overloads, and a clause naming one by bare name selects
/// them all; refining every one would narrow requirements the author never
/// named. The rejection belongs to typed-tree lowering, where the clause is
/// resolved against the base's signatures.
#[test]
fn a_targeted_clause_naming_overloads_is_ambiguous() {
    let source = r#"
        trait Logger {
            machine write(&mut self) suspends;
            machine write(&mut self, value: i32) suspends;
        }

        trait LocalLogger = Logger {
            machine Logger::write suspends false;
        }
    "#;
    let diagnostics = crate::tests::front_end::typed_program_result(source)
        .err()
        .expect("an ambiguous targeted clause rejects at typing");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .to_string()
                .contains("names 2 overloads of base trait `Logger`; name one exact requirement")
        }),
        "{diagnostics:#?}"
    );
}

/// A single-overload target still resolves, so the ambiguity check does not
/// simply reject every targeted clause.
#[test]
fn a_targeted_clause_naming_one_requirement_still_resolves() {
    let source = r#"
        trait Logger {
            machine write(&mut self) suspends;
            machine flush(&mut self) suspends;
        }

        trait LocalLogger = Logger {
            machine Logger::flush suspends false;
        }
    "#;
    checked_program_result(source).expect("a clause naming one exact requirement resolves");
}
