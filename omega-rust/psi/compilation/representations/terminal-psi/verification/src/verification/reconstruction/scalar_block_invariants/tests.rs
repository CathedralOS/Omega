use super::{certified_members, member_certified};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};
use std::collections::BTreeMap;

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

fn propositions(facts: &[super::HeaderAxiomFact]) -> Vec<Proposition> {
    facts.iter().map(|fact| fact.proposition.clone()).collect()
}

/// Every member of a flat declared conjunction joins the roster as a fact
/// the certificate checker re-decided — the declared predicate is
/// assumption zero and each emission is eliminated along its exact index.
#[test]
fn flat_conjunction_members_certify_in_order() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let integer_scalar = ScalarType::Integer(integer_type);
    let members = vec![
        Proposition::Equal(value(1, ScalarType::Boolean), ScalarTerm::Boolean(true)),
        Proposition::LessOrEqual(value(2, integer_scalar), integer(integer_type, 100)),
    ];
    let predicate = Proposition::Conjunction(members.clone());
    let facts = certified_members(
        &context(&[(1, ScalarType::Boolean), (2, integer_scalar)]),
        &predicate,
    );
    assert_eq!(propositions(&facts), members);
    assert!(facts.iter().all(|fact| fact.certified));
}

/// Members buried inside nested conjunctions certify through a chained
/// elimination on their recorded conjunct-index path.
#[test]
fn nested_conjunction_members_certify() {
    let integer_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
    let integer_scalar = ScalarType::Integer(integer_type);
    let inner = vec![
        Proposition::LessOrEqual(value(2, integer_scalar), integer(integer_type, 100)),
        Proposition::Equal(value(3, ScalarType::Boolean), ScalarTerm::Boolean(false)),
    ];
    let outer_first = Proposition::Equal(value(1, ScalarType::Boolean), ScalarTerm::Boolean(true));
    let predicate = Proposition::Conjunction(vec![
        outer_first.clone(),
        Proposition::Conjunction(inner.clone()),
    ]);
    let facts = certified_members(
        &context(&[
            (1, ScalarType::Boolean),
            (2, integer_scalar),
            (3, ScalarType::Boolean),
        ]),
        &predicate,
    );
    let mut expected = vec![outer_first];
    expected.extend(inner);
    assert_eq!(propositions(&facts), expected);
    assert!(facts.iter().all(|fact| fact.certified));
}

/// A disjunction member is emitted whole: members under alternative or
/// implication connectives are never independently established, but the
/// member itself certifies as a direct conjunct of the declared predicate.
#[test]
fn alternatives_join_whole_and_certify() {
    let boolean = ScalarType::Boolean;
    let disjunction = Proposition::Disjunction(vec![
        Proposition::Equal(value(1, boolean), ScalarTerm::Boolean(true)),
        Proposition::Equal(value(2, boolean), ScalarTerm::Boolean(false)),
    ]);
    let equality = Proposition::Equal(value(3, boolean), ScalarTerm::Boolean(true));
    let predicate = Proposition::Conjunction(vec![disjunction.clone(), equality.clone()]);
    let facts = certified_members(
        &context(&[(1, boolean), (2, boolean), (3, boolean)]),
        &predicate,
    );
    assert_eq!(propositions(&facts), vec![disjunction, equality]);
    assert!(facts.iter().all(|fact| fact.certified));
}

/// A non-conjunction declared predicate emits itself, certified by the
/// bare assumption citation: the checker re-decides that the emission is
/// exactly the declared predicate and no other proposition.
#[test]
fn bare_predicate_certifies_as_own_member() {
    let predicate = Proposition::Equal(value(1, ScalarType::Boolean), ScalarTerm::Boolean(true));
    let facts = certified_members(&context(&[(1, ScalarType::Boolean)]), &predicate);
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].proposition, predicate);
    assert!(facts[0].certified);
}

/// The classification is honest: an index path that does not select the
/// claimed member — or walks off the conjunction — reports `false` and
/// keeps the emission under the licensed premise row rather than claiming
/// an elimination the checker never re-decided.
#[test]
fn mismatched_paths_report_licensed() {
    let boolean = ScalarType::Boolean;
    let left = Proposition::Equal(value(1, boolean), ScalarTerm::Boolean(true));
    let right = Proposition::Equal(value(2, boolean), ScalarTerm::Boolean(false));
    let predicate = Proposition::Conjunction(vec![left, right.clone()]);
    let proposition_context = context(&[(1, boolean), (2, boolean)]);
    // Index 0 selects `left`, not the claimed `right`.
    assert!(!member_certified(
        &proposition_context,
        &predicate,
        &[0],
        &right,
    ));
    // An out-of-range index selects nothing.
    assert!(!member_certified(
        &proposition_context,
        &predicate,
        &[9],
        &right,
    ));
    // The correct path certifies.
    assert!(member_certified(
        &proposition_context,
        &predicate,
        &[1],
        &right,
    ));
}

/// A proposition that is no conjunct of the declared predicate can never
/// certify — the checker would reject the elimination, and the leaf guard
/// already refuses to route a fabricated emission through the machinery.
#[test]
fn fabricated_members_report_licensed() {
    let boolean = ScalarType::Boolean;
    let declared = Proposition::Equal(value(1, boolean), ScalarTerm::Boolean(true));
    let predicate = Proposition::Conjunction(vec![declared]);
    let fabricated = Proposition::Equal(value(2, boolean), ScalarTerm::Boolean(false));
    assert!(!member_certified(
        &context(&[(1, boolean), (2, boolean)]),
        &predicate,
        &[0],
        &fabricated,
    ));
}
