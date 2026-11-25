//! A law proved by cases inside cases.
//!
//! A case of a law's induction may call a nested machine that dispatches on
//! another formal. Its cases are a split inside that case: the law's goal
//! there, for each case of the second value, with the first already taken
//! apart. Only the induction gives the law as a hypothesis.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const TRANSITIVE: &str =
    include_str!("../../tests/fixtures/nested_cases/pass/order_is_transitive/main.omg");
const UNCITED: &str =
    include_str!("../../tests/fixtures/nested_cases/fail/innermost_case_uncited/main.omg");
const NAT_METRIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/nat_metric.omg"
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
fn a_law_is_proved_by_cases_inside_cases() {
    let verdicts = judged(&proofs(TRANSITIVE), &[]);
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "order_is_transitive")
        .expect("the law");
    let MathematicalCoreDecision::Judged(receipt) = law.decision else {
        panic!("the kernel must judge the law: {law:?}")
    };
    // It stands on the two lemmas it cites and on nothing assumed: the two
    // impossible cases are refuted from the law's own premises.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 2));
    eprintln!(
        "order_is_transitive: {} conversion steps",
        law.conversion_steps
    );
}

#[test]
fn the_innermost_case_left_uncited_is_what_stays_unproved() {
    let source = proofs(UNCITED);
    assert_eq!(
        source,
        proofs(TRANSITIVE).replace("        succ_monus_succ(pa, pc);\n", "")
    );
    let verdicts = judged(&source, &[]);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "order_is_transitive");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    // The failure is reported under the case of the induction it is in.
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `order_is_transitive` in the case `Peano::Succ`: \
         core\\Identity::<Peano>(monus(Peano::Succ(pred), c), Peano::Zero)"
    );
}

#[test]
fn every_law_of_nat_metric_is_judged() {
    // `core/nat_metric.omg` orders and measures the naturals of `nat.omg`.
    // As with that file, its machines are private and the library is judged
    // only through a program, so the two files' declarations are compiled
    // as the program's.
    let mut program = super::nat_semiring::library_declarations();
    program.push_str(NAT_METRIC);
    let verdicts = judged(
        &program,
        &[(
            "algebra.omg",
            "CommutativeSemiring",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/algebra.omg"
            )),
        )],
    );
    let judgment = signature(&verdicts);
    let mut largest = (0, "");
    for machine in [
        "nat_gap_reflexive",
        "nat_gap_symmetric",
        "nat_gap_zero_left",
        "nat_gap_zero_right",
        "nat_gap_succ",
        "add_le_add_right",
        // Three splits deep.
        "le_trans",
        "add_le_add",
        "mul_le_mul_right",
        "mul_le_cancel_right",
        "le_add_gap",
        "nat_gap_le_add",
        "nat_gap_triangle",
        "sub_add_cancel_left",
        "nat_gap_add_common",
        "nat_gap_mul_right",
        "mul_swap_trailing",
        "le_add_right_self",
        "le_max_left",
        "le_max_right",
        "nat_max_le",
        "nat_max_self",
    ] {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("the law `{machine}`"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{machine}"
        );
        if law.conversion_steps > largest.0 {
            largest = (law.conversion_steps, machine);
        }
    }
    eprintln!(
        "the largest, `{}`, takes {} conversion steps",
        largest.1, largest.0
    );
}
