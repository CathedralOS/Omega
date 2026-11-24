use super::*;

#[test]
fn cited_kernel_terms_rewrite_beneath_an_application_and_chain_in_reverse() {
    use crate::checking::equality::{SearchError, prove};
    use proof_admission::{
        Budget, Context, Declaration, Level, Sort, Term, TermArena, check_signature, check_type,
    };
    let mut arena = TermArena::new();
    let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let [carrier, operation, x, y, z, xy, zy] = std::array::from_fn(|slot| {
        arena.insert(Term::Constant {
            declaration: slot as u32,
            levels: vec![],
        })
    });
    let unary = arena.insert(Term::Pi {
        domain: carrier,
        codomain: carrier,
    });
    let x_equals_y = arena.insert(Term::Id {
        ty: carrier,
        left: x,
        right: y,
    });
    let z_equals_y = arena.insert(Term::Id {
        ty: carrier,
        left: z,
        right: y,
    });
    let declarations = [
        universe, unary, carrier, carrier, carrier, x_equals_y, z_equals_y,
    ]
    .map(|ty| Declaration::assumption(0, ty));
    let mut budget = Budget::default();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    let context = Context::empty().with_signature(signature);
    let left = arena.apply_arguments(operation, &[x]);
    let right = arena.apply_arguments(operation, &[z]);
    let goal = arena.insert(Term::Id {
        ty: carrier,
        left,
        right,
    });
    let proof = prove(&mut arena, &context, goal, &[xy, zy], 256, &mut budget).unwrap();
    check_type(&mut arena, &context, proof, goal, &mut budget).unwrap();
    assert_eq!(
        prove(&mut arena, &context, goal, &[xy], 256, &mut budget),
        Err(SearchError::NoProof)
    );
    assert_eq!(
        prove(&mut arena, &context, goal, &[xy, zy], 1, &mut budget),
        Err(SearchError::Budget)
    );
}

const CITED: &str = include_str!("../../tests/fixtures/semiring_identity/pass/cited/main.omg");
const UNCITED: &str = include_str!("../../tests/fixtures/semiring_identity/fail/uncited/main.omg");

fn generic(source: &str) -> RouteVerdict {
    let typed = super::build_purpose::frontier_with_core(
        source,
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
    let mut parts = crate::checking::judge_compilation(compilation.program).into_iter();
    let closure = parts.next().expect("package judgment").unwrap();
    assert!(matches!(closure, RouteVerdict::Accepted(_)), "{closure:?}");
    let form = parts.next().expect("generic machine judgment").unwrap();
    for other in parts {
        assert!(matches!(other, Ok(RouteVerdict::Accepted(_))), "{other:?}");
    }
    form
}

#[test]
fn semiring_identity_uses_the_cited_law() {
    let verdict = generic(CITED);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("expected an accepted generic contract: {verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("expected one judged Identity, got {guarantees:?}")
    };
    assert!(matches!(
        judgment.decision,
        MathematicalCoreDecision::Judged(_)
    ));
    assert_eq!(judgment.citations.len(), 1);
    let mut checked =
        terminal_codec::decode_mathematical_certificate(&judgment.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    use proof_admission::Term;
    let Term::Id { ty, left, right } = checked.arena.get(checked.certificate.expected) else {
        panic!("the contract must denote kernel Id")
    };
    let binary = |term| {
        let Term::Apply {
            function,
            argument: second,
        } = checked.arena.get(term)
        else {
            panic!("second add operand")
        };
        let Term::Apply {
            function: add,
            argument: first,
        } = checked.arena.get(function)
        else {
            panic!("first add operand")
        };
        (add, first, second)
    };
    let (add, b, a) = binary(left);
    let (other_add, other_a, other_b) = binary(right);
    assert!(checked.arena.structurally_equal(add, other_add));
    assert!(checked.arena.structurally_equal(a, other_a));
    assert!(checked.arena.structurally_equal(b, other_b));
    assert!(!checked.arena.structurally_equal(a, b));
    assert!(matches!(checked.arena.get(ty), Term::Constant { .. }));
    assert!(matches!(
        checked.arena.get(checked.certificate.term),
        Term::IdElim { .. }
    ));
}

#[test]
fn a_law_over_an_integer_keeps_its_operation_and_inputs() {
    let source = CITED
        .replace("T [copy], ", "")
        .replace("<T>", "<u64>")
        .replace(": T", ": u64");
    let verdict = generic(&source);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    assert!(
        matches!(guarantees.as_slice(), [(_, Ok(Discharged::Identity(judgment)))]
        if matches!(judgment.decision, MathematicalCoreDecision::Judged(_))),
        "{guarantees:?}"
    );
}

#[test]
fn an_entry_equality_is_a_kernel_hypothesis() {
    let equality = UNCITED
        .lines()
        .find_map(|line| line.strip_prefix("ensures "))
        .unwrap();
    let source = UNCITED.replace("ensures ", &format!("requires {equality}\nensures "));
    let verdict = generic(&source);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("{guarantees:?}")
    };
    assert!(
        matches!(judgment.decision, MathematicalCoreDecision::Judged(receipt) if receipt.context_depth == 1)
    );
}

#[test]
fn semiring_identity_without_citation_names_the_unproved_equality() {
    let verdict = generic(UNCITED);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("expected an unproved return guarantee: {verdict:?}")
    };
    let [unproved] = unproved.as_slice() else {
        panic!("one unproved guarantee")
    };
    let equality = UNCITED
        .lines()
        .find_map(|line| line.strip_prefix("ensures "))
        .unwrap();
    assert_eq!(unproved.cause, UnprovedCause::NoDerivation);
    assert!(unproved.statement.contains(equality), "{unproved:?}");
    let terminal_check_tables::program::SourceOrigin::Authored(site) = unproved.site else {
        panic!("expected authored source position")
    };
    assert!(
        UNCITED[site.span.start..site.span.end].contains(equality),
        "{site:?}"
    );
}
