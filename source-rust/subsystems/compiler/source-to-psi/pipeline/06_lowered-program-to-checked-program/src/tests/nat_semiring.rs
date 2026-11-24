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
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

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

/// The fixture's proofs, without the entry that needs the standard library.
fn proofs(source: &str) -> String {
    let end = source.find("data Main").expect("the fixture's entry state");
    source[..end].replace("use omega\\language\\core\\ProcessExit;\n", "")
}

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
fn a_program_over_the_library_nat_gets_its_whole_semiring_conformance() {
    use terminal_check_tables::mathematics::operand_law;
    let verdicts = judged(&proofs(LICENSED));
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
        let MathematicalCoreDecision::Judged(receipt) = law.decision else {
            panic!("the kernel must judge `{machine}`: {law:?}")
        };
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
    let source = proofs(UNLICENSED);
    let verdicts = judged(&source);
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
        source[site.span.start..].starts_with("core\\Identity::<Nat>(plus_local("),
        "{site:?}"
    );
}
