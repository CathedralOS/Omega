//! `core/nat.omg`'s `CommutativeSemiring<Nat>` conformance, judged by the
//! mathematical kernel for a program that uses the library's `Nat`.
//!
//! The program calls only `add` and `mul`. Each satisfies a requirement of
//! the trait, so the checker reads the rest of that conformance from the
//! library file as it stands: the four operations and the seven laws, with
//! the lemmas those cite. Several of them are written as machines that
//! return a value and state a law (`mul_succ_right`, `mul_distributes_right`),
//! and three of the seven laws are private to the library, reachable only
//! this way.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const LICENSED: &str =
    include_str!("../../tests/fixtures/nat_semiring/pass/library_conformance/main.omg");
const UNLICENSED: &str =
    include_str!("../../tests/fixtures/nat_semiring/fail/local_operation_unlicensed/main.omg");
const NAT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/nat.omg"
));
const ALGEBRA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/algebra.omg"
));

/// The laws `nat.omg` binds to the trait's seven law requirements.
const CONFORMANCE: [&str; 7] = [
    "add_zero_right",
    "add_comm",
    "add_assoc",
    "mul_comm",
    "mul_assoc",
    "mul_identity",
    "mul_distributes",
];

fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[
            ("nat.omg", "Nat", NAT),
            ("nat.omg", "add", NAT),
            ("nat.omg", "mul", NAT),
            ("algebra.omg", "CommutativeSemiring", ALGEBRA),
        ],
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
fn a_program_over_the_library_nat_gets_its_whole_semiring_conformance() {
    use terminal_check_tables::mathematics::operand_law;
    let verdicts = judged(LICENSED);
    let judgment = signature(&verdicts);
    let law = |machine: &str| {
        judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("the law `{machine}`: {judgment:?}"))
    };
    // The program names none of the seven laws; the kernel judges every one.
    for machine in CONFORMANCE {
        let law = law(machine);
        let MathematicalCoreDecision::Judged(receipt) = law.decision;
        assert_eq!(receipt.context_depth, 0, "{machine}");
        eprintln!(
            "{machine}: {} declarations, {} arena slots, {} conversion steps",
            receipt.declarations, receipt.arena_slots, law.conversion_steps
        );
    }
    // The lemmas written as machines that return a value are laws too.
    for machine in [
        "add_succ_law",
        "mul_zero_right",
        "mul_succ_right",
        "mul_distributes_right",
    ] {
        assert!(
            matches!(law(machine).decision, MathematicalCoreDecision::Judged(_)),
            "{machine}"
        );
    }

    // The program's own law cites nothing. Everything its proof rests on is
    // a commutativity or associativity law of the library's conformance.
    let own = law("relies_on_the_semiring");
    assert!(matches!(own.decision, MathematicalCoreDecision::Judged(_)));
    let mut checked = terminal_codec::decode_mathematical_certificate(&own.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    let closure =
        proof_admission::certificate_assumption_closure(&checked.arena, &checked.certificate);
    assert!(closure.len() >= 2, "{closure:?}");
    let mut operations = Vec::new();
    for assumed in closure {
        let (operation, _) = operand_law(
            &checked.arena,
            checked.certificate.signature[assumed as usize].ty,
        )
        .expect("each assumption states a law of one operation");
        if !operations.contains(&operation) {
            operations.push(operation);
        }
    }
    // Laws of both `add` and `mul`.
    assert_eq!(operations.len(), 2, "{operations:?}");
}

#[test]
fn an_operation_outside_the_conformance_gets_no_license_from_it() {
    let source = UNLICENSED;
    let verdicts = judged(source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "relies_on_nothing");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `relies_on_nothing`: \
         core\\Identity::<Nat>(plus_local(mul(a, b), plus_local(c, a)), \
         plus_local(a, plus_local(c, mul(b, a))))"
    );
    let site = law.site.expect("the law's ensures clause");
    assert!(
        source[site.span.start..].starts_with("core\\Identity::<|nat|::Nat>(plus_local("),
        "{site:?}"
    );
}

/// `nat.omg`'s own declarations as a program. The library is only ever
/// judged through a program that reaches it, and most of these lemmas are
/// private, so the file's text is compiled as the program's. Left out are the
/// three ranking primitives only the toolchain may declare. `mod`, `div` and
/// `gcd` are left out here too: `fuel_recursion` judges the file with them,
/// and the tests that build on this text measure what they add without them.
/// Every other declaration is the file's, unchanged, the `<=` and `-`
/// operator machines among them.
pub(super) fn library_declarations() -> String {
    let start = NAT.find("public data Nat {").expect("the data declaration");
    let mut program = String::from(
        "use {omega}/language/core/algebra.omg;\nuse omega\\language\\core\\CommutativeSemiring;\n",
    );
    let mut skipping = false;
    for line in NAT[start..].lines() {
        skipping |= ["mod", "div", "gcd"]
            .iter()
            .any(|name| line.starts_with(&format!("machine {name}(")));
        if !skipping {
            program.push_str(line);
            program.push('\n');
        } else if line == "}" {
            skipping = false;
        }
    }
    program
}

#[test]
fn every_law_of_nat_but_three_recursions_is_judged() {
    let typed = super::build_purpose::frontier_with_core(
        &library_declarations(),
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[("algebra.omg", "CommutativeSemiring", ALGEBRA)],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let verdicts: Vec<_> = crate::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect();
    let judgment = signature(&verdicts);
    let mut most = (0, "");
    for law in &judgment.laws {
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
        if law.conversion_steps > most.0 {
            most = (law.conversion_steps, law.machine.as_str());
        }
    }
    eprintln!(
        "{} laws; the largest, `{}`, takes {} conversion steps",
        judgment.laws.len(),
        most.1,
        most.0
    );
    // The laws that need what the semiring conformance did not: a premise
    // (`add_cancel`), a premise no value satisfies (`le_of_succ_le`,
    // `pred_eq_succ`), and a cited law whose premise is a hypothesis the body
    // states after it (`sub_le`).
    for machine in [
        "add_cancel",
        "le_of_succ_le",
        "pred_eq_succ",
        "pred_le_left",
        "sub_le",
        "sub_lt",
        "sub_eq_add",
        "le_add",
        "lt_of_sub_pos",
    ] {
        assert!(
            judgment.laws.iter().any(|law| law.machine == machine),
            "{machine}"
        );
    }
    assert!(judgment.laws.len() >= 34, "{}", judgment.laws.len());
}
