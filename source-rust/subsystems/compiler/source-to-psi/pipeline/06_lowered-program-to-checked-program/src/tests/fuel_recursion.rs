//! Machines that recurse on something other than a field of a case, and the
//! rationals that stand on three of them.
//!
//! The kernel recurses only on the structure of a value. A machine whose
//! recursive call is at a computed value is defined by recursion on fuel
//! taken from the formal it is ranked on, and a law about its own result is
//! proved for every amount of fuel. Each such call owes its descent: the
//! argument strictly below the caller's in core `Nat`'s natural order, a
//! statement proved where the call is made. Data other than core `Nat` has no
//! such order, so a machine over it is refused. A dispatch on a computed
//! value is a definition by cases of its own. A call of a machine that has a
//! premise owes that premise where it is made.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const DRAIN: &str =
    include_str!("../../tests/fixtures/fuel_recursion/fail/drain_local_data/main.omg");
const ALGEBRA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/algebra.omg"
));

fn proofs(source: &str) -> String {
    let start = source.find("data Peano").expect("the fixture's data");
    let end = source.find("data Main").expect("the fixture's entry state");
    format!(
        "use {{omega}}/language/core/foundation.omg as proof;\n{}",
        &source[start..end]
    )
}

fn judged(source: &str, core: &[(&str, &str, &str)]) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        core,
    );
    judged_frontier(typed)
}

fn judged_frontier(typed: crate::source_state::TypedTrees) -> Vec<RouteVerdict> {
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
fn a_machine_that_recurses_on_a_computed_value_of_local_data_is_refused() {
    // `drain` calls itself at `before(a)` over the program's own `Peano`.
    // Only core `Nat` has the natural order a computed argument's descent
    // is checked under, so the machine is refused where it is read, not
    // defined on fuel with its descent unchecked.
    let verdicts = judged(&proofs(DRAIN), &[]);
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            RouteVerdict::Laws(judgment) => panic!("`drain` must not be judged: {judgment:?}"),
            _ => None,
        })
        .collect();
    assert!(
        refused.iter().any(|detail| detail
            .contains("recursive call at a computed value whose data has no natural order")),
        "{verdicts:?}"
    );
}

/// Judge the application-owned proof files separately, including their private
/// laws. Nat's ranking declarations remain toolchain-only.
fn judged_library(files: &[(&str, &str)]) -> Vec<RouteVerdict> {
    let nat = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source/library/core/nat.omg"
    ));
    let first = nat.find("public data Nat {").expect("the data declaration");
    let nat = format!(
        "use {{omega}}/language/core/algebra.omg;\nuse {{omega}}/language/core/foundation.omg as proof;\n{}",
        &nat[first..]
    );
    judged_library_files(&nat, files)
}

pub(super) fn judged_library_files(nat: &str, files: &[(&str, &str)]) -> Vec<RouteVerdict> {
    let files: Vec<_> = files
        .iter()
        .map(|&(name, text)| {
            (
                name,
                text.replacen(
                    "use algebra.omg;",
                    "use {omega}/language/core/algebra.omg;",
                    1,
                ),
            )
        })
        .collect();
    let mut product = vec![("nat.omg", nat)];
    product.extend(files.iter().map(|(name, text)| (*name, text.as_str())));
    let typed = super::build_purpose::frontier_with_product_files(
        &product,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[("algebra.omg", "CommutativeSemiring", ALGEBRA)],
    );
    judged_frontier(typed)
}

#[test]
fn core_division_and_gcd_are_structural() {
    // `div` and `mod` of `core/nat.omg` project one structural helper,
    // `divide_step`, so neither recurses on fuel nor owes a descent. `mod`
    // states its strict bound and `div_mod_reconstruction` the equation
    // `div(a, b) * b + mod(a, b) = a`, each from the helper's invariant.
    // `gcd` is `gcd_within` at its divisor as the bound on Euclid's steps,
    // which recurses on that bound alone. `gcd_within_agrees` proves every
    // sufficient bound gives the same answer, and `gcd_step` states
    // Euclid's step `gcd(a, b) = gcd(b, mod(a, b))` from it. No machine of
    // the file is defined on fuel.
    let verdicts = judged_library(&[]);
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
    assert_eq!(judged("div"), 0, "no descent: the helper is structural");
    assert_eq!(judged("div_mod_reconstruction"), 1, "reconstruction");
    assert_eq!(judged("divide_reconstructs"), 1, "the helper's invariant");
    assert_eq!(judged("divide_bounds"), 1, "the helper's remainder bound");
    assert_eq!(judged("gcd"), 0, "no descent: the bound is structural");
    assert_eq!(judged("gcd_within"), 0, "no premise owed at its call");
    assert_eq!(judged("le_trans"), 1, "transitivity of the order");
    assert_eq!(
        judged("gcd_within_agrees"),
        1,
        "every sufficient bound agrees"
    );
    assert_eq!(judged("gcd_step"), 1, "Euclid's step");
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
    let verdicts = judged_library(&[
        (
            "nat_metric.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/nat_metric.omg"
            )),
        ),
        (
            "int.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/int.omg"
            )),
        ),
        (
            "rat.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/rat.omg"
            )),
        ),
    ]);
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

#[test]
fn every_law_of_cauchy_is_judged() {
    // `core/cauchy.omg` states what it is for a sequence of rationals to be
    // Cauchy, for any sequence and any modulus: its machines are generic
    // over machines. It is judged with the four files it stands on.
    let verdicts = judged_library(&[
        (
            "nat_metric.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/nat_metric.omg"
            )),
        ),
        (
            "int.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/int.omg"
            )),
        ),
        (
            "rat.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/rat.omg"
            )),
        ),
        (
            "cauchy.omg",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/cauchy.omg"
            )),
        ),
    ]);
    let judgment = signature(&verdicts);
    for law in &judgment.laws {
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
    for machine in [
        "cauchy_at_reflexive",
        "converges_together_at_symmetric",
        "converges_together_at_triangle_split",
        "doubled_nat_max_threshold",
    ] {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("the law `{machine}`"));
        eprintln!("{machine}: {} conversion steps", law.conversion_steps);
    }
    eprintln!("{} laws", judgment.laws.len());
}

/// Natural order over local data, decided as `core/nat.omg` decides it: by
/// cases on the computed `saturating_sub(left, right)`, answering `bool`.
const ORDER: &str = "use {omega}/language/core/foundation.omg as proof;

data Peano {
    case Zero;
    case Succ(prev: Peano);
}

machine pred(a: Peano) -> Peano
{
    transition a {
        Peano::Zero -> Peano::Zero,
        Peano::Succ { prev } -> (prev)
    }
}

machine saturating_sub(a: Peano, b: Peano) -> Peano
terminates by b
{
    transition b {
        Peano::Zero -> (a),
        Peano::Succ { prev } -> (pred(saturating_sub(a, prev)))
    }
}

machine less_or_equal(left: Peano, right: Peano) -> RESULT
{
    transition (saturating_sub(left, right)) {
        Peano::Zero -> YES,
        Peano::Succ { prev } -> NO
    }
}
";

fn order(result: &str, yes: &str, no: &str, law: &str) -> String {
    format!(
        "{}\nmachine {law}\n{{\n}}\n",
        ORDER
            .replace("RESULT", result)
            .replace("YES", yes)
            .replace("NO", no)
    )
}

#[test]
fn a_law_that_a_boolean_definition_is_true_is_judged() {
    // A `bool` result is the kernel's `Two`, and a clause that uses the
    // machine as a proposition states that its result is `true`. Zero is at
    // most one: the kernel computes the definition to `true`.
    let verdicts = judged(
        &order(
            "bool",
            "true",
            "false",
            "zero_is_at_most_one(a: Peano)\n\
             ensures less_or_equal(Peano::Zero, Peano::Succ { prev: Peano::Zero })",
        ),
        &[],
    );
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "zero_is_at_most_one")
        .expect("the law");
    assert!(
        matches!(law.decision, MathematicalCoreDecision::Judged(_)),
        "{law:?}"
    );
}

#[test]
fn a_law_that_a_false_boolean_definition_is_true_is_refused() {
    // The same law with the operands swapped: one is not at most zero, so
    // the definition computes to `false` and the law has no proof.
    let verdicts = judged(
        &order(
            "bool",
            "true",
            "false",
            "zero_is_at_most_one(a: Peano)\n\
             ensures less_or_equal(Peano::Succ { prev: Peano::Zero }, Peano::Zero)",
        ),
        &[],
    );
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "zero_is_at_most_one");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn a_mathematical_machine_with_another_scalar_result_is_refused() {
    // Only `bool` joins mathematical data as a result: a `u8` has no kernel
    // type here, so the machine is refused where it is read.
    let verdicts = judged(
        &order(
            "u8",
            "1",
            "0",
            "zero_is_at_most_one(a: Peano)
\
             ensures |proof|::Identity::<u8>(less_or_equal(Peano::Zero, Peano::Zero), 1)",
        ),
        &[],
    );
    let refused: Vec<_> = verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
            _ => None,
        })
        .collect();
    assert!(
        refused.iter().any(|detail| detail
            .contains("mathematical declaration over a type other than data that contains itself")),
        "{verdicts:?}"
    );
}
