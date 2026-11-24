//! Laws about a record whose fields are mathematical values.
//!
//! Such a record has no storage either, so it is read into the mathematical
//! signature as the product of its fields' types: a record written from its
//! fields is a tuple, and a field is a projection. An equality of two records
//! is proved by rewriting inside their fields.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const PAIR: &str = include_str!("../../tests/fixtures/record_law/pass/pair_of_peano/main.omg");
const UNCITED: &str =
    include_str!("../../tests/fixtures/record_law/fail/pair_field_uncited/main.omg");

fn proofs(source: &str) -> String {
    let end = source.find("data Main").expect("the fixture's entry state");
    source[..end].replace("use omega\\language\\core\\ProcessExit;\n", "")
}

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
fn laws_about_a_record_of_mathematical_values_are_judged() {
    use proof_admission::Term;
    let verdicts = judged(&proofs(PAIR));
    let judgment = signature(&verdicts);
    for (machine, assumptions) in [
        ("plus_zero_right", 0),
        // Pure computation and the record of a value's own fields.
        ("swap_twice", 0),
        // One citation per field.
        ("add_pair_zero", 1),
        ("swap_left", 0),
    ] {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("the law `{machine}`: {judgment:?}"));
        let MathematicalCoreDecision::Judged(receipt) = law.decision else {
            panic!("the kernel must judge `{machine}`: {law:?}")
        };
        assert_eq!(
            (receipt.context_depth, receipt.assumption_closure),
            (0, assumptions),
            "{machine}"
        );
        eprintln!("{machine}: {} conversion steps", law.conversion_steps);
    }
    // The record is the product of its two fields' types, and the law about
    // it is an equality at that type.
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "swap_twice")
        .unwrap();
    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    let arena = &checked.arena;
    let Term::Pi { domain, codomain } = arena.get(checked.certificate.expected) else {
        panic!("one formal")
    };
    let Term::Id { ty, .. } = arena.get(codomain) else {
        panic!("the law must denote kernel Id")
    };
    assert!(arena.structurally_equal(ty, domain));
    let Term::Constant { declaration, .. } = arena.get(ty) else {
        panic!("the record is a declared constant")
    };
    let Some(Term::Sigma { domain, codomain }) = checked.certificate.signature
        [declaration as usize]
        .body
        .map(|body| arena.get(body))
    else {
        panic!("the record is the product of its fields")
    };
    assert!(arena.structurally_equal(domain, codomain));
}

#[test]
fn a_record_field_left_uncited_is_what_stays_unproved() {
    let source = proofs(UNCITED);
    let passing = proofs(PAIR);
    assert_eq!(
        source[source.find("data Peano").unwrap()..],
        passing.replace("    plus_zero_right(p.right);\n", "")
            [passing.find("data Peano").unwrap()..]
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "add_pair_zero");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `add_pair_zero`: \
         core\\Identity::<Pair>(add_pair(p, zero_pair), p)"
    );
}

#[test]
fn every_law_of_int_is_judged() {
    // `core/int.omg` builds the integers as pairs of naturals. As with
    // `nat.omg`, the library is judged only through a program, and these
    // machines are private, so the two files' declarations are compiled as
    // the program's.
    let mut program = super::nat_semiring::library_declarations();
    program.push_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source/library/core/int.omg"
    )));
    let typed = super::build_purpose::frontier_with_core(
        &program,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[(
            "algebra.omg",
            "CommutativeSemiring",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../../../../../source/library/core/algebra.omg"
            )),
        )],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let verdicts: Vec<_> = crate::checking::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect();
    let judgment = signature(&verdicts);
    for machine in [
        "add_int_comm",
        "add_int_assoc",
        "add_int_neg",
        "mul_int_comm",
        "mul_int_assoc",
        "mul_int_distributes",
        "int_eq_symmetric",
        "add_int_identity",
        "mul_int_identity",
        "neg_int_involution",
        // Each of these uses a hypothesis on part of a longer sum.
        "neg_int_respects_eq",
        "add_int_respects_eq",
        "mul_int_respects_eq",
        "int_eq_transitive",
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
        eprintln!("{machine}: {} conversion steps", law.conversion_steps);
    }
}
