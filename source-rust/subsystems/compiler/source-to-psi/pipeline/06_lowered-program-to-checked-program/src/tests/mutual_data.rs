//! A mutual block of data from source, judged by the mathematical kernel.
//!
//! `Even` and `Odd` contain each other, so stage 05 declares them as one
//! kernel block with a recursor per member, and reads the mutually recursive
//! `half` and `half_odd` through the recursors of that block.

use super::seq_laws::{judged, signature, unproved};
use super::*;

const MUTUAL: &str = "
data Even {
    case Zero;
    case Next(odd: Odd);
}

data Odd {
    case Next(even: Even);
}

machine half(e: Even) -> Nat
terminates by e
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> Nat::Succ { prev: half_odd(odd) }
    }
}

machine half_odd(o: Odd) -> Nat
terminates by o
{
    transition o {
        Odd::Next { even } -> half(even)
    }
}

machine half_two()
ensures
    |proof|::Identity::<Nat>(
        half(Even::Next { odd: Odd::Next { even: Even::Zero } }),
        Nat::Succ { prev: Nat::Zero }
    )
{
}
";

fn program(declarations: &str) -> String {
    format!(
        "{}{declarations}",
        super::nat_semiring::library_declarations()
    )
}

#[test]
fn a_law_over_a_mutual_block_is_judged() {
    let verdicts = judged(&program(MUTUAL));
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "half_two")
        .unwrap_or_else(|| panic!("`half_two` is judged: {judgment:?}"));
    assert!(
        matches!(law.decision, MathematicalCoreDecision::Judged(_)),
        "{law:?}"
    );
    let carriers: Vec<_> = judgment
        .carriers
        .iter()
        .map(|carrier| carrier.type_identity.as_str())
        .collect();
    for member in ["Even", "Odd"] {
        assert!(
            carriers.iter().any(|carrier| carrier.contains(member)),
            "{carriers:?}"
        );
    }
}

#[test]
fn a_false_law_over_a_mutual_block_is_refused() {
    let source =
        program(MUTUAL).replace("Nat::Succ { prev: Nat::Zero }\n    )", "Nat::Zero\n    )");
    assert_ne!(source, program(MUTUAL), "the law is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "half_two", "{law:?}");
}

/// The non-accepted verdicts' refusal details; every one must be a refusal.
fn refusals(verdicts: &[RouteVerdict]) -> Vec<String> {
    verdicts
        .iter()
        .filter(|verdict| !matches!(verdict, RouteVerdict::Accepted(_)))
        .map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => refusal.detail.clone(),
            other => panic!("the part must be refused: {other:?}"),
        })
        .collect()
}

#[test]
fn a_law_by_induction_over_a_mutual_block_is_refused() {
    // Induction over one member of a block takes every member's motive; a
    // law by induction states only its own, so it is refused clearly.
    let source = program(&format!(
        "{MUTUAL}
machine half_again(e: Even) -> Nat
terminates by e
ensures
    |proof|::Identity::<Nat>(half(e), half(e))
{{
    transition e {{
        Even::Zero -> Nat::Zero,
        Even::Next {{ odd }} -> Nat::Zero
    }}
}}
"
    ));
    let refused = refusals(&judged(&source));
    assert!(
        refused
            .iter()
            .any(|detail| detail.contains("law by induction over a mutual block of data")),
        "{refused:#?}"
    );
}

#[test]
fn data_containing_itself_through_a_callable_is_refused_at_its_declaration() {
    // `Loop` reaches itself through the parameter of the callable `Test`: a
    // negative occurrence, which no inductive declares.
    let source = program(
        "
data Test = machine(value: Loop) -> bool reaches none crashes none terminates;

data Loop {
    case End;
    case Wrap(test: Test, next: Loop);
}

machine depth(l: Loop) -> Nat
terminates by l
{
    transition l {
        Loop::End -> Nat::Zero,
        Loop::Wrap { test, next } -> Nat::Succ { prev: depth(next) }
    }
}

machine depth_end()
ensures
    |proof|::Identity::<Nat>(depth(Loop::End), Nat::Zero)
{
}
",
    );
    let refused = refusals(&judged(&source));
    let declaration = "while reading the mathematical declaration of `Loop`";
    assert!(
        refused.iter().any(|detail| detail.ends_with(declaration)),
        "{refused:#?}"
    );
    // `Test` itself is refused for its record, and every machine over `Loop`
    // repeats the declaration's refusal.
    let reason = "a type other than data that contains itself";
    for machine in ["depth", "depth_end"] {
        let at = format!("while reading the mathematical declaration of `{machine}`");
        assert!(
            refused
                .iter()
                .any(|detail| detail.contains(reason) && detail.ends_with(&at)),
            "{refused:#?}"
        );
    }
}
