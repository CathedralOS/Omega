//! Machines that recurse on something other than a field of a case, and the
//! rationals that stand on three of them.
//!
//! The kernel recurses only on the structure of a value. A machine whose
//! recursive call is at a computed value is defined by recursion on fuel
//! taken from the formal it is ranked on, and a law about its own result is
//! proved for every amount of fuel. A dispatch on a computed value is a
//! definition by cases of its own. A call of a machine that has a premise
//! owes that premise where it is made.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const DRAIN: &str =
    include_str!("../../tests/fixtures/fuel_recursion/pass/drain_reaches_zero/main.omg");
const STOPS_AT_ONE: &str =
    include_str!("../../tests/fixtures/fuel_recursion/fail/drain_stops_at_one/main.omg");
const ALGEBRA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/algebra.omg"
));

fn proofs(source: &str) -> String {
    let start = source.find("data Peano").expect("the fixture's data");
    let end = source.find("data Main").expect("the fixture's entry state");
    source[start..end].to_owned()
}

fn judged(source: &str, core: &[(&str, &str, &str)]) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        core,
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    crate::checking::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect()
}

fn signature(verdicts: &[RouteVerdict]) -> &SignatureJudgment {
    let mut laws = None;
    for verdict in verdicts {
        match verdict {
            RouteVerdict::Accepted(_) => {}
            RouteVerdict::Laws(judgment) => {
                assert!(laws.replace(&**judgment).is_none(), "one signature");
            }
            other => panic!("every part must be accepted: {other:?}"),
        }
    }
    laws.expect("the signature's judgment")
}

fn unproved(verdicts: &[RouteVerdict]) -> &UnprovedLaw {
    let mut laws = verdicts.iter().filter_map(|verdict| match verdict {
        RouteVerdict::UnprovedLaws(laws) => Some(laws),
        RouteVerdict::Laws(judgment) => panic!("the law must not be accepted: {judgment:?}"),
        _ => None,
    });
    let (Some([law]), None) = (laws.next().map(Vec::as_slice), laws.next()) else {
        panic!("expected exactly one unproved law: {verdicts:?}")
    };
    law
}

#[test]
fn a_law_about_a_machine_that_recurses_on_a_computed_value_is_judged() {
    let verdicts = judged(&proofs(DRAIN), &[]);
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "drain")
        .expect("the law");
    let MathematicalCoreDecision::Judged(receipt) = law.decision else {
        panic!("the kernel must judge the law: {law:?}")
    };
    // Nothing is assumed: the machine is a definition over fuel the kernel
    // checked, and the law at one unit less is the induction's hypothesis.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
    eprintln!("drain: {} conversion steps", law.conversion_steps);
}

#[test]
fn a_false_law_about_such_a_machine_is_refused() {
    let source = proofs(STOPS_AT_ONE);
    assert_eq!(
        source,
        proofs(DRAIN).replace(
            "        Peano::Zero -> Peano::Zero,\n        Peano::Succ { pred as rest }",
            "        Peano::Zero -> Peano::Succ { pred: Peano::Zero },\n        Peano::Succ { pred as rest }"
        )
    );
    let verdicts = judged(&source, &[]);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "drain");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `drain`: \
         core\\Identity::<Peano>(drain(a), Peano::Zero)"
    );
}

/// `nat.omg` in full but for its two operator machines, then each named
/// library file's text after it.
fn library(files: &[&str]) -> String {
    let nat = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source/library/core/nat.omg"
    ));
    let first = nat.find("public data Nat {").expect("the data declaration");
    let operators = nat
        .find("// Natural order and exact subtraction")
        .expect("the operator machines");
    let ring = nat
        .find("// The Peano ring surface")
        .expect("the operations");
    let mut program = String::from("use omega\\language\\core\\CommutativeSemiring;\n");
    program.push_str(&nat[first..operators]);
    program.push_str(&nat[ring..]);
    for file in files {
        program.push_str(file);
    }
    program
}

#[test]
fn mod_div_and_gcd_of_nat_are_defined_and_mod_keeps_its_bound() {
    // The three machines of `core/nat.omg` whose recursion is on
    // `saturating_sub(a, b)` or on `mod(a, b)`. `mod` states that its result
    // is below the divisor; `gcd` calls `mod`, which has a premise, and owes
    // it at that call.
    let verdicts = judged(
        &library(&[]),
        &[("algebra.omg", "CommutativeSemiring", ALGEBRA)],
    );
    let judgment = signature(&verdicts);
    let judged = |machine: &str| {
        judgment
            .laws
            .iter()
            .filter(|law| law.machine == machine)
            .inspect(|law| {
                assert!(
                    matches!(law.decision, MathematicalCoreDecision::Judged(_)),
                    "{machine}"
                )
            })
            .count()
    };
    assert_eq!(judged("mod"), 1, "its own bound");
    assert!(judged("gcd") >= 1, "the premise of `mod` at its call");
    for law in &judgment.laws {
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
}

#[test]
fn every_law_of_rat_is_judged() {
    // `core/rat.omg` builds the rationals from `int.omg`'s pairs over
    // `nat.omg`'s naturals, with `nat_metric.omg`'s order and distance. As
    // with those files, the library is judged only through a program, so
    // the four files' declarations are compiled as the program's.
    let program = library(&[
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/core/nat_metric.omg"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/core/int.omg"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../../../../source/library/core/rat.omg"
        )),
    ]);
    let verdicts = judged(&program, &[("algebra.omg", "CommutativeSemiring", ALGEBRA)]);
    let judgment = signature(&verdicts);
    let mut largest = (0, "");
    for law in &judgment.laws {
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
        if law.conversion_steps > largest.0 {
            largest = (law.conversion_steps, law.machine.as_str());
        }
    }
    for machine in [
        "rat_gap_reflexive",
        "rat_gap_symmetric",
        "rat_close_reflexive",
        "rat_close_symmetric",
        "rat_gap_scaled_with_common",
        "rat_gap_triangle_coordinate_alignment",
        // Thirteen rewrites over large terms: past what the search of every
        // order can do, and proved by rewriting both sides to one form.
        "rat_gap_triangle_scaled",
        "rat_close_triangle_factorization",
        "rat_close_triangle_split",
    ] {
        assert!(
            judgment.laws.iter().any(|law| law.machine == machine),
            "{machine}"
        );
    }
    eprintln!(
        "{} laws; the largest, `{}`, takes {} conversion steps",
        judgment.laws.len(),
        largest.1,
        largest.0
    );
}
