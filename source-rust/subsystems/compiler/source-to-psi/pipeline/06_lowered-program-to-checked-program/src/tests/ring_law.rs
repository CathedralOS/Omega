//! The Peano addition associativity law of the recovered
//! `ring_law_conformance` program, judged by the mathematical kernel.
//!
//! `Peano` contains itself, so it and the machines over it exist only in the
//! compilation's mathematical signature. `plus` is defined by cases on its
//! first parameter and `plus_assoc` proves associativity by induction on its
//! first: the `Succ` case cites the law at the case's own field.
//!
//! `peano_right_distributivity` adds `times` and the law that it distributes
//! over `plus` from the right. Its successor case needs three addends
//! regrouped, which the proof does under the associativity law the program's
//! conformance declares, without citing it.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const CONFORMING: &str =
    include_str!("../../tests/fixtures/ring_law/pass/ring_law_conformance/main.omg");
const LAWLESS: &str =
    include_str!("../../tests/fixtures/ring_law/fail/ring_law_unproven_rejected/main.omg");
const DISTRIBUTES: &str =
    include_str!("../../tests/fixtures/ring_law/pass/peano_right_distributivity/main.omg");
const DISTRIBUTES_UNCITED: &str =
    include_str!("../../tests/fixtures/ring_law/fail/peano_right_distributivity_uncited/main.omg");

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

/// The line of the source that holds this position.
fn line(source: &str, at: usize) -> &str {
    let start = source[..at].rfind('\n').map_or(0, |found| found + 1);
    let end = source[at..]
        .find('\n')
        .map_or(source.len(), |found| at + found);
    source[start..end].trim()
}

#[test]
fn right_distributivity_regroups_under_the_associativity_law_of_its_conformance() {
    use terminal_check_tables::mathematics::{OperandLaw, operand_law};
    let verdicts = judged(&proofs(DISTRIBUTES));
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "times_distributes_right")
        .unwrap_or_else(|| panic!("the distributivity law: {judgment:?}"));
    let MathematicalCoreDecision::Judged(receipt) = law.decision else {
        panic!("the kernel must judge the law: {law:?}")
    };
    // Closed, and resting on exactly one assumption.
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 1));
    eprintln!(
        "times_distributes_right: {} declarations, {} arena slots, {} conversion steps",
        receipt.declarations, receipt.arena_slots, law.conversion_steps
    );

    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    let arena = &checked.arena;
    let declarations = &checked.certificate.signature;
    // The one assumption is associativity of the operation the law's
    // statement adds with. The body never names it; the proof term does.
    let closure = proof_admission::certificate_assumption_closure(arena, &checked.certificate);
    let [assumed] = closure.iter().copied().collect::<Vec<_>>()[..] else {
        panic!("one assumption: {closure:?}")
    };
    let (plus, shape) = operand_law(arena, declarations[assumed as usize].ty)
        .expect("the assumption states a law of one operation");
    assert_eq!(
        shape,
        OperandLaw::Associates {
            left_nested_first: true
        }
    );
    // The statement is times(plus(a, b), c) = plus(times(a, c), times(b, c)),
    // and nothing the proof is checked against states it.
    let mut statement = checked.certificate.expected;
    while let proof_admission::Term::Pi { codomain, .. } = arena.get(statement) {
        statement = codomain;
    }
    let proof_admission::Term::Id { right, .. } = arena.get(statement) else {
        panic!("the law must denote kernel Id")
    };
    let proof_admission::Term::Apply { function, .. } = arena.get(right) else {
        panic!("the right side adds two products")
    };
    let proof_admission::Term::Apply { function: head, .. } = arena.get(function) else {
        panic!("the right side adds two products")
    };
    assert_eq!(
        arena.get(head),
        proof_admission::Term::Constant {
            declaration: plus,
            levels: Vec::new()
        }
    );
    for declaration in declarations {
        assert!(!arena.structurally_equal(declaration.ty, checked.certificate.expected));
    }
}

#[test]
fn an_associativity_law_outside_the_conformance_licenses_nothing() {
    // The same proved law, no longer declared as `Semiring2<Peano>::add_assoc`.
    // A law of the right shape in scope is not a license; the conformance is.
    // The associativity law is moved above the distributivity law first, so
    // the order of the two in the source is not what decides.
    let authored = proofs(DISTRIBUTES);
    let distributes = authored.find("\n// Declared above").expect("the law");
    let associates = authored.find("\nmachine plus_assoc").expect("its license");
    let conforming = format!(
        "{}{}{}",
        &authored[..distributes],
        &authored[associates..],
        &authored[distributes..associates]
    );
    assert!(
        signature(&judged(&conforming))
            .laws
            .iter()
            .any(|law| law.machine == "times_distributes_right")
    );
    let source = conforming.replace("satisfies Semiring2<Peano>::add_assoc\n", "");
    assert_ne!(source, conforming);
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "times_distributes_right");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

#[test]
fn operands_are_reordered_only_under_a_commutativity_law() {
    // No Peano program can prove commutativity yet: its base case is
    // `plus(b, Zero) = b`, which today's encoding of data cannot prove (the
    // kernel audit's step 3a notes say why). So the laws here are
    // assumptions of a bare signature: a type, one operation on it, four
    // values, and the two laws.
    use crate::checking::equality::{License, SearchError, prove_under};
    use proof_admission::{
        Budget, Context, Declaration, Level, Sort, Term, TermArena, check_signature, check_type,
    };
    let mut arena = TermArena::new();
    let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let [carrier, operation, a, b, c, d, commutes, associates] = std::array::from_fn(|slot| {
        arena.insert(Term::Constant {
            declaration: slot as u32,
            levels: vec![],
        })
    });
    let binary = {
        let unary = arena.insert(Term::Pi {
            domain: carrier,
            codomain: carrier,
        });
        arena.insert(Term::Pi {
            domain: carrier,
            codomain: unary,
        })
    };
    let bound = |arena: &mut TermArena, index| arena.insert(Term::Variable(index));
    let law = |arena: &mut TermArena, formals: usize, sides: &dyn Fn(&mut TermArena) -> (_, _)| {
        let (left, right) = sides(arena);
        let mut statement = arena.insert(Term::Id {
            ty: carrier,
            left,
            right,
        });
        for _ in 0..formals {
            statement = arena.insert(Term::Pi {
                domain: carrier,
                codomain: statement,
            });
        }
        statement
    };
    let commutativity = law(&mut arena, 2, &|arena| {
        let (x, y) = (bound(arena, 1), bound(arena, 0));
        (
            arena.apply_arguments(operation, &[x, y]),
            arena.apply_arguments(operation, &[y, x]),
        )
    });
    let associativity = law(&mut arena, 3, &|arena| {
        let (x, y, z) = (bound(arena, 2), bound(arena, 1), bound(arena, 0));
        let xy = arena.apply_arguments(operation, &[x, y]);
        let yz = arena.apply_arguments(operation, &[y, z]);
        (
            arena.apply_arguments(operation, &[xy, z]),
            arena.apply_arguments(operation, &[x, yz]),
        )
    });
    let declarations = [
        universe,
        binary,
        carrier,
        carrier,
        carrier,
        carrier,
        commutativity,
        associativity,
    ]
    .map(|ty| Declaration::assumption(0, ty));
    let mut budget = Budget::default();
    let checked = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    let context = Context::empty().with_signature(checked);
    let sum = |arena: &mut TermArena, left, right| arena.apply_arguments(operation, &[left, right]);
    let equal = |arena: &mut TermArena, left, right| {
        arena.insert(Term::Id {
            ty: carrier,
            left,
            right,
        })
    };
    // The four-addend shuffle `mul_distributes` leaves in `core/nat.omg`:
    // (a + b) + (c + d) = (a + c) + (b + d).
    let shuffle = {
        let (ab, cd) = (sum(&mut arena, a, b), sum(&mut arena, c, d));
        let (ac, bd) = (sum(&mut arena, a, c), sum(&mut arena, b, d));
        let (left, right) = (sum(&mut arena, ab, cd), sum(&mut arena, ac, bd));
        equal(&mut arena, left, right)
    };
    let swap = {
        let (ab, ba) = (sum(&mut arena, a, b), sum(&mut arena, b, a));
        equal(&mut arena, ab, ba)
    };
    let regroup = {
        let (ab, bc) = (sum(&mut arena, a, b), sum(&mut arena, b, c));
        let (left, right) = (sum(&mut arena, ab, c), sum(&mut arena, a, bc));
        equal(&mut arena, left, right)
    };
    let license = |associates, commutes| License {
        operation,
        carrier,
        associates,
        commutes,
    };
    let both = license(Some((associates, true)), Some(commutes));
    let regrouping = license(Some((associates, true)), None);
    let reordering = license(None, Some(commutes));
    let attempt = |arena: &mut TermArena, goal, licenses: &[License]| {
        let mut budget = Budget::default();
        let proof = prove_under(arena, &context, goal, &[], licenses, 4096, &mut budget)?;
        check_type(arena, &context, proof, goal, &mut budget).unwrap();
        Ok::<_, SearchError>(())
    };
    for goal in [shuffle, swap, regroup] {
        attempt(&mut arena, goal, &[both]).unwrap();
        assert_eq!(attempt(&mut arena, goal, &[]), Err(SearchError::NoProof));
    }
    // Each law licenses its own rearrangement and not the other's.
    attempt(&mut arena, regroup, &[regrouping]).unwrap();
    attempt(&mut arena, swap, &[reordering]).unwrap();
    for (goal, licenses) in [
        (swap, regrouping),
        (shuffle, regrouping),
        (regroup, reordering),
        (shuffle, reordering),
    ] {
        assert_eq!(
            attempt(&mut arena, goal, &[licenses]),
            Err(SearchError::NoProof)
        );
    }
}

#[test]
fn without_its_recursive_call_distributivity_would_need_itself() {
    let source = proofs(DISTRIBUTES_UNCITED);
    // Only the law's call on `pred` is gone.
    assert_eq!(
        source,
        proofs(DISTRIBUTES)
            .replace("        times_distributes_right(pred, b, c);\n", "")
            .replace(
                proofs(DISTRIBUTES).split("data Peano").next().unwrap(),
                source.split("data Peano").next().unwrap()
            )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "times_distributes_right");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `times_distributes_right` in the case `Peano::Succ`: \
         core\\Identity::<Peano>(times(plus(Peano::Succ(pred), b), c), \
         plus(times(Peano::Succ(pred), c), times(b, c)))"
    );
    let site = law.site.expect("the successor arm");
    assert_eq!(
        line(&source, site.span.start),
        "Peano::Succ { pred } -> step(pred, b, c)"
    );
    assert!(
        !source[..site.span.start].contains("machine plus_assoc"),
        "the arm of the distributivity law, not of the associativity law"
    );
}
