//! The foundation's `Box` and `Squash` in source.
//!
//! `|proof|::Squash::<P>` is the strict proposition that `P` holds and
//! `|proof|::Box::<S>` the relevant type of a strict `S`. A boxed squash is
//! introduced from a proof of the proposition and opened only toward a
//! strict goal, so its witness never leaves it.

use super::*;
use crate::laws::UnprovedLaw;

const NUMBERS: &str = r"
use {omega}/language/core/foundation.omg as proof;

data N {
    case Z;
    case S(p: N);
}

proposition same(a: N, b: N) = |proof|::Identity::<N>(a, b);

proposition squashed(a: N, b: N) = |proof|::Squash::<same(a, b)>;

proposition boxed(a: N, b: N) = |proof|::Box::<|proof|::Squash::<same(a, b)>>;
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
fn a_squash_and_its_box_prove_each_other() {
    // Both are at the strict level of `same`, so the box is introduced from
    // the squash and opened back into it.
    let verdicts = judged(&format!(
        "{NUMBERS}
machine boxes(a: N, b: N)
requires squashed(a, b)
ensures boxed(a, b)
{{
}}

machine unboxes(a: N, b: N)
requires boxed(a, b)
ensures squashed(a, b)
{{
}}
"
    ));
    assert_eq!(judged_laws(&verdicts), ["boxes", "unboxes"]);
}

#[test]
fn a_boxed_squash_is_proved_from_the_proposition_and_opened_toward_one() {
    let verdicts = judged(&format!(
        "{NUMBERS}
machine squashes(a: N, b: N)
requires same(a, b)
ensures boxed(b, a)
{{
}}

machine symmetric(a: N, b: N)
requires boxed(a, b)
ensures boxed(b, a)
{{
}}
"
    ));
    assert_eq!(judged_laws(&verdicts), ["squashes", "symmetric"]);
}

#[test]
fn a_boxed_squash_does_not_give_back_its_witness() {
    // `same` is relevant: eliminating the squash toward it would extract
    // the proof the squash hides.
    let verdicts = judged(&format!(
        "{NUMBERS}
machine extracts(a: N, b: N)
requires boxed(a, b)
ensures same(a, b)
{{
}}
"
    ));
    let [law] = unproved(&verdicts)[..] else {
        panic!("one unproved law: {verdicts:?}")
    };
    assert_eq!(law.machine, "extracts");
}

#[test]
fn a_box_of_a_relevant_proposition_is_not_formed() {
    let verdicts = judged(&format!(
        "{NUMBERS}
proposition relevant_box(a: N, b: N) = |proof|::Box::<same(a, b)>;

machine boxes_relevant(a: N, b: N)
requires same(a, b)
ensures relevant_box(a, b)
{{
}}
"
    ));
    let [law] = unproved(&verdicts)[..] else {
        panic!("one unproved law: {verdicts:?}")
    };
    assert_eq!(law.machine, "boxes_relevant");
    assert!(law.detail.contains("NonStrictBoxDomain"), "{law:?}");
}
