use super::{RewrittenSuccessorFact, successor_rewrite_certified};
use semantic_vocabulary::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    ScalarQualificationSetId, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeMap;
use terminal_psi::{Block, Terminator, ValueDeclaration};

fn value(index: u64, scalar_type: ScalarType) -> ScalarTerm {
    ScalarTerm::value(ValueId::new(index).unwrap(), scalar_type)
}

fn integer(scalar_type: IntegerType, literal: i128) -> ScalarTerm {
    ScalarTerm::integer(
        scalar_type,
        match scalar_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(literal),
            IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(literal).unwrap()),
        },
    )
    .unwrap()
}

fn context(value_types: &[(u64, ScalarType)]) -> PropositionContext {
    PropositionContext::from_value_types(
        value_types
            .iter()
            .map(|(index, scalar_type)| (ValueId::new(*index).unwrap(), *scalar_type))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}

fn successor_block(parameters: Vec<(u64, ScalarType)>) -> Block {
    Block {
        id: BlockId::new(7).unwrap(),
        parameters: parameters
            .into_iter()
            .map(|(index, scalar_type)| ValueDeclaration {
                id: ValueId::new(index).unwrap(),
                scalar_type,
                qualifications: ScalarQualificationSetId::ZERO,
            })
            .collect(),
        erased_scalar_formals: Vec::new(),
        erased_proof_formals: Vec::new(),
        structural_parameters: Vec::new(),
        operations: Vec::new(),
        terminator: Terminator::ReturnUnit {
            edge: EdgeId::new(3).unwrap(),
            trivial_affine_discards: Vec::new(),
        },
    }
}

/// A fact restated through the edge's own binding equalities is a transport,
/// never a fresh premise: the checker re-decides the certificate before the
/// emission may count as proved.
#[test]
fn successor_rewrite_certifies_through_binding_equalities() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
    let argument = ValueId::new(1).unwrap();
    let mut axioms = vec![Proposition::LessOrEqual(
        value(1, scalar_type),
        integer(IntegerType::new(IntegerSign::Signed, 8).unwrap(), 100),
    )];
    let block = successor_block(vec![(2, scalar_type)]);
    let proposition_context = context(&[(1, scalar_type), (2, scalar_type)]);
    let rewritten = super::super::bind_successor_axioms(
        &mut axioms,
        &block,
        &[argument],
        &|id| ScalarTerm::value(id, scalar_type),
        &proposition_context,
        true,
    );
    let expected = Proposition::LessOrEqual(
        value(2, scalar_type),
        integer(IntegerType::new(IntegerSign::Signed, 8).unwrap(), 100),
    );
    assert_eq!(
        axioms[1],
        Proposition::Equal(value(2, scalar_type), value(1, scalar_type))
    );
    assert_eq!(axioms[2], expected);
    assert_eq!(rewritten.len(), 1);
    assert_eq!(rewritten[0].proposition, expected);
    assert!(rewritten[0].certified);
}

/// An established fact that does not mention the edge's arguments is not a
/// successor rewrite and produces no transport obligation at all.
#[test]
fn unrelated_facts_are_not_rewritten() {
    let scalar_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 8).unwrap());
    let argument = ValueId::new(1).unwrap();
    let mut axioms = vec![Proposition::LessOrEqual(
        value(9, scalar_type),
        integer(IntegerType::new(IntegerSign::Signed, 8).unwrap(), 5),
    )];
    let block = successor_block(vec![(2, scalar_type)]);
    let proposition_context = context(&[(1, scalar_type), (2, scalar_type), (9, scalar_type)]);
    let rewritten = super::super::bind_successor_axioms(
        &mut axioms,
        &block,
        &[argument],
        &|id| ScalarTerm::value(id, scalar_type),
        &proposition_context,
        true,
    );
    assert!(rewritten.is_empty());
    assert_eq!(axioms.len(), 2);
}

/// A fact appended under an already-bound edge transports through that
/// edge's equalities when the substitution is a genuine restatement.
#[test]
fn appended_fact_certifies_through_pushed_equalities() {
    let scalar_type = ScalarType::Boolean;
    let argument = ValueId::new(1).unwrap();
    let mut axioms = vec![Proposition::Equal(
        value(2, scalar_type),
        value(1, scalar_type),
    )];
    let block = successor_block(vec![(2, scalar_type)]);
    let proposition = Proposition::Equal(value(1, scalar_type), ScalarTerm::Boolean(true));
    let proposition_context = context(&[(1, scalar_type), (2, scalar_type)]);
    let rewritten = super::super::append_successor_fact(
        &mut axioms,
        &proposition,
        &block,
        &[argument],
        &|id| ScalarTerm::value(id, scalar_type),
        &proposition_context,
    );
    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        rewritten[0].proposition,
        Proposition::Equal(value(2, scalar_type), ScalarTerm::Boolean(true))
    );
    assert!(rewritten[0].certified);
    assert_eq!(axioms.last().unwrap(), &rewritten[0].proposition);
}

/// The classification is honest: a conclusion the certificate checker cannot
/// re-decide through the edge's equalities reports `false` and keeps the
/// emission under the licensed premise row rather than claiming transport.
#[test]
fn unmatched_restatements_report_licensed() {
    let scalar_type = ScalarType::Boolean;
    let axioms = vec![
        Proposition::Equal(value(1, scalar_type), ScalarTerm::Boolean(true)),
        Proposition::Equal(value(2, scalar_type), value(1, scalar_type)),
    ];
    let proposition_context = context(&[(1, scalar_type), (2, scalar_type), (3, scalar_type)]);
    // The claimed conclusion names a value unrelated by the edge equalities.
    assert!(!successor_rewrite_certified(
        &proposition_context,
        &axioms,
        0,
        &[1],
        &Proposition::Equal(value(3, scalar_type), ScalarTerm::Boolean(true)),
    ));
    // A stale source index cannot certify anything.
    assert!(!successor_rewrite_certified(
        &proposition_context,
        &axioms,
        5,
        &[1],
        &Proposition::Equal(value(2, scalar_type), ScalarTerm::Boolean(true)),
    ));
    // Without the edge's binding equalities there is no transport premise.
    assert!(!successor_rewrite_certified(
        &proposition_context,
        &axioms,
        0,
        &[],
        &Proposition::Equal(value(2, scalar_type), ScalarTerm::Boolean(true)),
    ));
}

/// The certificate's conclusion is the actual restated proposition the
/// generator emits, so a correct rewrite of a licensed emission still
/// certifies even when its source joined the roster late.
#[test]
fn late_source_facts_still_certify() {
    let scalar_type = ScalarType::Boolean;
    let argument = ValueId::new(1).unwrap();
    let mut axioms = vec![
        Proposition::Equal(value(4, scalar_type), ScalarTerm::Boolean(false)),
        Proposition::Equal(value(2, scalar_type), value(1, scalar_type)),
    ];
    let block = successor_block(vec![(2, scalar_type)]);
    let proposition = Proposition::Equal(value(4, scalar_type), value(1, scalar_type));
    let proposition_context = context(&[(1, scalar_type), (2, scalar_type), (4, scalar_type)]);
    let rewritten = super::super::append_successor_fact(
        &mut axioms,
        &proposition,
        &block,
        &[argument],
        &|id| ScalarTerm::value(id, scalar_type),
        &proposition_context,
    );
    assert_eq!(rewritten.len(), 1);
    assert_eq!(
        rewritten[0].proposition,
        Proposition::Equal(value(4, scalar_type), value(2, scalar_type))
    );
    assert!(rewritten[0].certified);
}

/// `RewrittenSuccessorFact` keeps the emission inspectable by tests.
#[test]
fn classification_carries_proposition_and_certification() {
    let proposition = Proposition::Truth;
    let fact = RewrittenSuccessorFact {
        proposition: proposition.clone(),
        certified: false,
    };
    assert_eq!(fact.proposition, proposition);
    assert!(!fact.certified);
}

/// Handing the checker only the cited source fact and binding equalities
/// decides each rewrite exactly as the complete roster did: unrelated rows
/// before, between and after the cited ones change neither acceptance nor
/// rejection, and the source may follow or coincide with the equalities.
#[test]
fn cited_rows_decide_like_the_complete_roster() {
    let scalar_type = ScalarType::Boolean;
    let equal = |left: u64, right: u64| {
        Proposition::Equal(value(left, scalar_type), value(right, scalar_type))
    };
    let truth = |id: u64| Proposition::Equal(value(id, scalar_type), ScalarTerm::Boolean(true));
    let axioms = vec![
        equal(8, 9),
        truth(1),
        truth(7),
        equal(2, 1),
        equal(6, 5),
        equal(3, 1),
        Proposition::Equal(value(4, scalar_type), value(1, scalar_type)),
        equal(9, 8),
    ];
    let proposition_context = context(&(1..=9).map(|id| (id, scalar_type)).collect::<Vec<_>>());
    let full_roster = |source: usize, equalities: &[usize], rewritten: &Proposition| {
        let citation = |index: usize| proof_admission::ProofNode {
            conclusion: axioms[index].clone(),
            rule: proof_admission::ProofRule::SemanticAxiom { index },
        };
        let certificate = proof_admission::ProofNode {
            conclusion: rewritten.clone(),
            rule: proof_admission::ProofRule::ValueEqualityTransport {
                premise: Box::new(citation(source)),
                equalities: equalities.iter().map(|&index| citation(index)).collect(),
            },
        };
        proof_admission::check_certificate(
            &proposition_context,
            rewritten,
            &[],
            &axioms,
            &certificate,
        )
        .is_ok()
    };
    let cases: [(usize, &[usize], Proposition); 8] = [
        (1, &[3], truth(2)),
        (1, &[3, 5], truth(3)),
        (1, &[5, 3], truth(2)),
        (1, &[3], truth(3)),
        (2, &[3], truth(2)),
        (6, &[3], equal(4, 2)),
        (3, &[3], equal(2, 2)),
        (7, &[0], equal(8, 8)),
    ];
    let mut accepted = 0;
    for (source, equalities, rewritten) in &cases {
        let expected = full_roster(*source, equalities, rewritten);
        assert_eq!(
            successor_rewrite_certified(
                &proposition_context,
                &axioms,
                *source,
                equalities,
                rewritten
            ),
            expected,
            "source {source}, equalities {equalities:?}, rewritten {rewritten:?}"
        );
        accepted += usize::from(expected);
    }
    assert!(
        accepted > 0 && accepted < cases.len(),
        "{accepted} accepted"
    );
}
