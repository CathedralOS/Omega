//! Premises that hold for every value.
//!
//! `let doubles_fixed(m: N): |proof|::Type<0> = (n: N) -> ...` names a
//! dependent product over data: a premise for every `n`. A law that requires
//! it uses it at each value its goal names, and a law it cites may owe a
//! premise at a value the citing law computes, which the same premise
//! supplies there. Nothing yet proves such a product as a conclusion.

use super::*;
use crate::laws::UnprovedLaw;

const NUMBERS: &str = r"
use {omega}/language/core/foundation.omg as proof;

data N {
    case Z;
    case S(p: N);
}

machine plus(a: N, b: N) -> N
terminates by a
{
    transition a {
        N::Z -> b,
        N::S { p } -> N::S { p: plus(p, b) }
    }
}

machine double(a: N) -> N {
    transition { _ -> (plus(a, a)) }
}

let doubles_fixed(m: N): |proof|::Type<0> =
    (n: N) -> |proof|::Identity::<N>(double(n), n);
";

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

fn unproved(verdicts: &[RouteVerdict]) -> Vec<&UnprovedLaw> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => Some(laws.iter()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn a_premise_for_every_value_holds_at_the_value_the_goal_names() {
    let verdicts = judged(&format!(
        "{NUMBERS}
machine at_the_formal(m: N)
requires doubles_fixed(m)
ensures |proof|::Identity::<N>(double(m), m)
{{
}}
"
    ));
    assert_eq!(judged_laws(&verdicts), ["at_the_formal"]);
    // Without the premise nothing equates the two.
    let verdicts = judged(&format!(
        "{NUMBERS}
machine at_the_formal(m: N)
ensures |proof|::Identity::<N>(double(m), m)
{{
}}
"
    ));
    let [law] = unproved(&verdicts)[..] else {
        panic!("one unproved law: {verdicts:?}")
    };
    assert_eq!(law.machine, "at_the_formal");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_cited_law_owes_its_premise_at_the_value_it_is_cited_at() {
    // `fact` requires the equation at `S(m)`, a value no formal is: the
    // premise for every value supplies it there.
    let fact = "
machine fact(m: N)
requires |proof|::Identity::<N>(double(N::S { p: m }), N::S { p: m })
ensures |proof|::Identity::<N>(plus(m, N::S { p: m }), m)
{
}
";
    let verdicts = judged(&format!(
        "{NUMBERS}{fact}
machine cites_fact(m: N)
requires doubles_fixed(m)
ensures |proof|::Identity::<N>(plus(m, N::S {{ p: m }}), m)
{{
    fact(m);
}}
"
    ));
    assert_eq!(judged_laws(&verdicts), ["fact", "cites_fact"]);
}

#[test]
fn a_premise_for_every_pair_holds_at_each_pair_the_goal_names() {
    let verdicts = judged(&format!(
        "{NUMBERS}
let absorbs(m: N): |proof|::Type<0> =
    (a: N) -> (b: N) -> |proof|::Identity::<N>(plus(a, b), a);

machine at_two_formals(m: N, k: N)
requires absorbs(m)
ensures |proof|::Identity::<N>(plus(k, m), k)
{{
}}
"
    ));
    assert_eq!(judged_laws(&verdicts), ["at_two_formals"]);
}

#[test]
fn a_premise_for_every_value_proves_only_its_own_equation() {
    // At `m` the premise equates `double(m)` and `m`, not `S(m)`.
    let verdicts = judged(&format!(
        "{NUMBERS}
machine other_value(m: N)
requires doubles_fixed(m)
ensures |proof|::Identity::<N>(double(m), N::S {{ p: m }})
{{
}}
"
    ));
    let [law] = unproved(&verdicts)[..] else {
        panic!("one unproved law: {verdicts:?}")
    };
    assert_eq!(law.machine, "other_value");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_conclusion_for_every_value_is_not_proved_yet() {
    // Its index has no binder in the body. How source proves one is an
    // owner question (`OWNER_QUESTIONS.md`, "How does a theorem prove a
    // for-every conclusion?").
    let verdicts = judged(&format!(
        "{NUMBERS}
machine concludes_every(m: N)
ensures doubles_fixed(m)
{{
}}
"
    ));
    let [law] = unproved(&verdicts)[..] else {
        panic!("one unproved law: {verdicts:?}")
    };
    assert_eq!(law.machine, "concludes_every");
}
