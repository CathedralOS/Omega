//! The Peano addition associativity law of the recovered
//! `ring_law_conformance` program, judged by the mathematical kernel.
//!
//! `Peano` contains itself, so it and the machines over it exist only in the
//! compilation's mathematical signature. `plus` is defined by cases on its
//! first parameter and `plus_assoc` proves associativity by induction on its
//! first: the `Succ` case cites the law at the case's own field.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const CONFORMING: &str =
    include_str!("../../tests/fixtures/ring_law/pass/ring_law_conformance/main.omg");
const LAWLESS: &str =
    include_str!("../../tests/fixtures/ring_law/fail/ring_law_unproven_rejected/main.omg");

/// The declarations a fixture proves with. Its entry and exit status need
/// the standard library, which the command-level test supplies.
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
fn the_kernel_judges_peano_addition_associative_by_induction() {
    use proof_admission::Term;
    let verdicts = judged(&proofs(CONFORMING));
    let judgment = signature(&verdicts);
    let [law] = judgment.laws.as_slice() else {
        panic!("one law: {judgment:?}")
    };
    assert_eq!((law.machine.as_str(), law.clause), ("plus_assoc", 0));
    let MathematicalCoreDecision::Judged(receipt) = law.decision else {
        panic!("the kernel must judge the law: {law:?}")
    };
    // Closed, and resting on no assumption: the data, its constructors and
    // `plus` are definitions.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
    eprintln!(
        "plus_assoc: {} declarations, {} arena slots, {} conversion steps",
        receipt.declarations, receipt.arena_slots, law.conversion_steps
    );

    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    assert!(
        proof_admission::certificate_assumption_closure(&checked.arena, &checked.certificate)
            .is_empty()
    );
    // The law under proof is not among its own evidence: nothing the proof
    // is checked against is an assumption, and none states the law.
    let arena = &checked.arena;
    let expected = checked.certificate.expected;
    for declaration in &checked.certificate.signature {
        assert!(!declaration.is_assumption());
        assert!(!arena.structurally_equal(declaration.ty, expected));
    }

    // The proposition is the associativity equality over three formals of
    // one data type: plus(plus(a, b), c) = plus(a, plus(b, c)).
    let mut statement = expected;
    let mut formals = Vec::new();
    while let Term::Pi { domain, codomain } = arena.get(statement) {
        formals.push(domain);
        statement = codomain;
    }
    let [peano, second, third] = formals.as_slice() else {
        panic!("three formals")
    };
    assert!(arena.structurally_equal(*peano, *second) && arena.structurally_equal(*peano, *third));
    let Term::Id { ty, left, right } = arena.get(statement) else {
        panic!("the law must denote kernel Id")
    };
    assert!(arena.structurally_equal(ty, *peano));
    let Term::Constant {
        declaration: data, ..
    } = arena.get(ty)
    else {
        panic!("the carrier is the declared data")
    };
    assert!(matches!(
        checked
            .certificate
            .signature
            .get(data as usize)
            .and_then(|declaration| declaration.body)
            .map(|body| arena.get(body)),
        Some(Term::W { .. })
    ));
    let applied = |term| {
        let Term::Apply {
            function,
            argument: second,
        } = arena.get(term)
        else {
            panic!("second operand")
        };
        let Term::Apply {
            function: plus,
            argument: first,
        } = arena.get(function)
        else {
            panic!("first operand")
        };
        (plus, first, second)
    };
    let variable = |term, index| assert_eq!(arena.get(term), Term::Variable(index));
    let (plus, inner, c) = applied(left);
    let (inner_plus, a, b) = applied(inner);
    let (outer, right_a, right_inner) = applied(right);
    let (right_plus, right_b, right_c) = applied(right_inner);
    for other in [inner_plus, outer, right_plus] {
        assert!(arena.structurally_equal(plus, other));
    }
    assert!(matches!(arena.get(plus), Term::Constant { .. }));
    for (term, index) in [
        (a, 2),
        (b, 1),
        (c, 0),
        (right_a, 2),
        (right_b, 1),
        (right_c, 0),
    ] {
        variable(term, index);
    }

    // The proof is an induction on the first formal.
    let mut proof = checked.certificate.term;
    for _ in 0..3 {
        let Term::Lambda { body, .. } = arena.get(proof) else {
            panic!("one binder per formal")
        };
        proof = body;
    }
    while let Term::Apply { function, .. } = arena.get(proof) {
        proof = function;
    }
    let Term::IndW { tree, .. } = arena.get(proof) else {
        panic!("the proof must be an induction")
    };
    variable(tree, 2);
}

#[test]
fn a_law_cannot_cite_itself_outside_a_field_of_the_dispatched_case() {
    // The step cites the law at `b` in the ranked position. That is not a
    // smaller value of the case, so it would be the law's own evidence.
    let source = proofs(CONFORMING).replace("plus_assoc(pred, b, c);", "plus_assoc(b, pred, c);");
    assert_ne!(source, proofs(CONFORMING));
    let verdicts = judged(&source);
    assert!(
        verdicts.iter().any(|verdict| matches!(verdict,
        RouteVerdict::FallBack(refused) if refused.detail.contains(
            "law cites itself at an argument that is not a field of the dispatched case"
        ))),
        "{verdicts:?}"
    );
    assert!(
        !verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::Laws(_))),
        "{verdicts:?}"
    );
}

/// Whether the compilation is refused as not supported with this reason and
/// no law is accepted.
fn unread(source: &str, reason: &str) -> bool {
    let verdicts = judged(source);
    verdicts.iter().any(
        |verdict| matches!(verdict, RouteVerdict::FallBack(refused) if refused.detail.contains(reason)),
    ) && !verdicts
        .iter()
        .any(|verdict| matches!(verdict, RouteVerdict::Laws(_)))
}

#[test]
fn every_declaration_the_ordinary_reader_skips_is_read_or_refused() {
    // The ordinary reader leaves data that contains itself, and the machines
    // over it, to the signature. Whatever the signature cannot state must be
    // refused there; nothing may pass because neither reader looked at it.
    let conforming = proofs(CONFORMING);
    let helper = conforming.replace(
        "    machine base() {\n    }\n",
        "    machine base() {\n    }\n    machine idle() {\n    }\n",
    );
    assert_ne!(helper, conforming);
    assert!(
        unread(
            &helper,
            "mathematical machine that states no law and that no law calls"
        ),
        "{:?}",
        judged(&helper)
    );
    let unused =
        format!("{conforming}\ndata Tagged {{ case Empty; case More(tag: u64, rest: Tagged); }}\n");
    assert!(
        unread(
            &unused,
            "data that contains itself with a field of another type"
        ),
        "{:?}",
        judged(&unused)
    );
}

#[test]
fn a_satisfier_that_proves_another_equality_does_not_conform() {
    // The machine proves a true equality, by computation alone, and claims
    // the trait's associativity law with it. Reflexivity is not that law.
    let source = proofs(CONFORMING).replace(
        "core\\Identity::<Peano>((plus(plus(a, b), c)), (plus(a, plus(b, c))))",
        "core\\Identity::<Peano>((plus(a, plus(b, c))), (plus(a, plus(b, c))))",
    );
    assert_ne!(source, proofs(CONFORMING));
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "plus_assoc");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.detail,
        "the proved equality is not the law its requirement declares"
    );
}

#[test]
fn a_satisfier_with_no_body_and_no_ensures_is_refused_for_the_missing_law() {
    let source = proofs(LAWLESS);
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "lawless");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    // The law is the one `RingOps2<Peano>::add_assoc` declares, with `add`
    // resolved to `plus`, the machine that satisfies it for `Peano`.
    assert_eq!(
        law.statement,
        "cannot prove the law `lawless` inherits from the requirement it satisfies: \
         core\\Identity::<Peano>(plus(plus(a, b), c), plus(a, plus(b, c)))"
    );
    let site = law.site.expect("the satisfier's declaration");
    assert!(source[site.span.start..].starts_with("lawless"), "{site:?}");
}

#[test]
fn without_the_recursive_call_the_successor_case_names_its_unproved_equality() {
    // The exact `ensures` of the conforming program; only the citation of
    // the law at `pred` is gone.
    let source = proofs(CONFORMING).replace("        plus_assoc(pred, b, c);\n", "");
    assert_ne!(source, proofs(CONFORMING));
    assert!(source.contains(
        "ensures\n    core\\Identity::<Peano>((plus(plus(a, b), c)), (plus(a, plus(b, c))))"
    ));
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "plus_assoc");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `plus_assoc` in the case `Peano::Succ`: \
         core\\Identity::<Peano>(plus(plus(Peano::Succ(pred), b), c), \
         plus(Peano::Succ(pred), plus(b, c)))"
    );
    // The position is the successor arm, not the law's `ensures` clause.
    let site = law.site.expect("the successor arm");
    let line_start = source[..site.span.start].rfind('\n').map_or(0, |at| at + 1);
    let line_end = source[site.span.start..]
        .find('\n')
        .map_or(source.len(), |at| site.span.start + at);
    assert_eq!(
        source[line_start..line_end].trim(),
        "Peano::Succ { pred } -> step(pred, b, c)"
    );
}

#[test]
fn an_exhausted_proof_budget_is_reported_apart_from_a_missing_proof() {
    let law = |cause| UnprovedLaw {
        machine: "plus_assoc".to_owned(),
        site: None,
        cause,
        statement: "cannot prove ensures contract for `plus_assoc`: core\\Identity::<Peano>(a, a)"
            .to_owned(),
        detail: "mathematical equality proof budget exhausted".to_owned(),
    };
    let exhausted = law(UnprovedCause::BudgetExhausted)
        .reject_diagnostic()
        .message;
    let missing = law(UnprovedCause::NoDerivation).reject_diagnostic().message;
    assert!(exhausted.starts_with("not supported yet: "), "{exhausted}");
    assert!(
        exhausted.contains("ran out of proof budget")
            && exhausted.contains("no claim that it is wrong"),
        "{exhausted}"
    );
    assert!(!missing.starts_with("not supported yet"), "{missing}");
}
