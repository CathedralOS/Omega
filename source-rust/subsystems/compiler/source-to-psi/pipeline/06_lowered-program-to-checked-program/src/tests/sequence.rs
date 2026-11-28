//! Proof data that pairs a state with a static sampling machine.
//!
//! `data Sequence<S, const At: Sampler<S>> { state: S; }` is the core
//! rational sequence's shape (quotients.md, "Cauchy sequences and the Real
//! quotient"): each value supplies its state, and the type's machine
//! argument says how a sample is drawn from it. A machine binder gives the
//! data no layout, so every application is proof-only, and the machine an
//! application selects must have exactly its parameter's formals and result.

use super::*;

fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    crate::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect()
}

/// The machines whose laws were judged, when every part is accepted.
fn judged_laws(verdicts: &[RouteVerdict]) -> Vec<String> {
    let mut machines = Vec::new();
    for verdict in verdicts {
        match verdict {
            RouteVerdict::Accepted(_) => {}
            RouteVerdict::Laws(judgment) => {
                for law in &judgment.laws {
                    assert!(
                        matches!(law.decision, MathematicalCoreDecision::Judged(_)),
                        "{law:?}"
                    );
                    machines.push(law.machine.clone());
                }
            }
            other => panic!("every part must be accepted: {other:?}"),
        }
    }
    machines
}

fn violations(verdicts: &[RouteVerdict]) -> Vec<String> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::Rejected { violations, .. } => Some(violations.iter()),
            _ => None,
        })
        .flatten()
        .map(|violation| violation.statement.clone())
        .collect()
}

/// A sequence of `N` whose sampler is `sampler`, with the program's
/// `claims` about it.
fn sequences(sampler: &str, claims: &str) -> String {
    format!(
        r"use {{omega}}/language/core/foundation.omg as proof;

data N {{
    case Z;
    case S(p: N);
}}

data M {{
    case Z;
    case S(p: M);
}}

data Sampler<S> = machine(state: S, n: N) -> N
    reaches none crashes none terminates;

data Sequence<S, const At: Sampler<S>> {{
    state: S;
}}

{sampler}

{claims}
"
    )
}

const REPEAT: &str = "machine repeat(state: N, n: N) -> N {
    state
}";

const CONSTANT: &str = "machine constant(q: N) -> Sequence<N, repeat> {
    Sequence { state: q }
}

machine constant_sample(q: N, n: N)
ensures |proof|::Identity::<N>(repeat(constant(q).state, n), q)
{
}

machine any_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(repeat(s.state, n), s.state)
{
}";

#[test]
fn a_sequence_is_sampled_by_its_machine_at_its_state() {
    // `constant(q)` holds `q`, and `repeat` returns its state at every
    // index, so each sample of it is `q`, as is each sample of any sequence
    // with that sampler at its own state.
    let laws = judged_laws(&judged(&sequences(REPEAT, CONSTANT)));
    assert!(laws.iter().any(|law| law == "constant_sample"), "{laws:?}");
    assert!(laws.iter().any(|law| law == "any_sample"), "{laws:?}");
}

#[test]
fn a_sample_is_only_what_the_sampler_computes() {
    let verdicts = judged(&sequences(
        REPEAT,
        "machine first_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(repeat(s.state, n), N::Z)
{
}",
    ));
    assert!(
        verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::UnprovedLaws(_))),
        "{verdicts:?}"
    );
}

#[test]
fn a_sampler_must_take_the_state_and_an_index() {
    // `Sampler<N>` takes a state and an index; a machine of the state
    // alone selects no sampler, whatever it computes.
    let verdicts = judged(&sequences(
        "machine repeat(state: N) -> N {
    state
}",
        "machine any_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(repeat(s.state), s.state)
{
}",
    ));
    let violations = violations(&verdicts);
    assert!(
        violations
            .iter()
            .any(|rule| rule.contains("a static machine argument must have its parameter's formals and result")),
        "{verdicts:?}"
    );
}

#[test]
fn a_sampler_must_read_the_sequence_s_own_state() {
    // `Sequence<N, repeat>` holds an `N`, so its sampler is a
    // `Sampler<N>`; one over `M` samples no state this sequence holds.
    let verdicts = judged(&sequences(
        "machine repeat(state: M, n: N) -> N {
    N::Z
}",
        "machine any_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(N::Z, N::Z)
{
}",
    ));
    let violations = violations(&verdicts);
    assert!(
        violations
            .iter()
            .any(|rule| rule.contains("a static machine argument must have its parameter's formals and result")),
        "{verdicts:?}"
    );
}
