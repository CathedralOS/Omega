//! Precedence pins for root selection.
//!
//! Each fixture gives two root sources enough evidence to complete the goal
//! on their own, with different certificates. The entry must emit the earlier
//! source's certificate; reordering the precedence list changes the emitted
//! proof bytes and fails the pin.

use proof_admission::{
    IntegerAffineWitness, IntegerCastChainWitness, ProofNode, ProofRule, check_certificate,
};
use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext, ScalarTerm,
    ScalarType, ValueId,
};

use super::super::affine_custody::DefinitionIndex;
use super::{
    AFFINE_CUSTODY_SOURCES, CAST_CUSTODY_SOURCES, Completion, Request, RootSource,
    prove_affine_bound, prove_affine_bound_without_cast, prove_cast_bound, select,
};

fn byte() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 8).unwrap()
}

fn half_word() -> IntegerType {
    IntegerType::new(IntegerSign::Unsigned, 16).unwrap()
}

fn value(identity: u64, integer_type: IntegerType) -> ScalarTerm {
    ScalarTerm::value(
        ValueId::new(identity).unwrap(),
        ScalarType::Integer(integer_type),
    )
}

fn literal(integer_type: IntegerType, literal: u128) -> ScalarTerm {
    ScalarTerm::integer(integer_type, IntegerValue::Unsigned(literal)).unwrap()
}

fn at_most(left: &ScalarTerm, right: &ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(left.clone(), right.clone())
}

fn equal(left: &ScalarTerm, right: &ScalarTerm) -> Proposition {
    Proposition::Equal(left.clone(), right.clone())
}

fn assumption(assumptions: &[Proposition], index: usize) -> ProofNode {
    ProofNode {
        conclusion: assumptions[index].clone(),
        rule: ProofRule::Assumption { index },
    }
}

/// `x` and `y` are bytes; `c == widen(x)` is a half word; goal `c <= 5`.
struct CastFixture {
    context: PropositionContext,
    x: ScalarTerm,
    y: ScalarTerm,
    cast: ScalarTerm,
    axioms: Vec<Proposition>,
    goal: Proposition,
}

impl CastFixture {
    fn new() -> Self {
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Integer(byte())),
            (ValueId::new(2).unwrap(), ScalarType::Integer(half_word())),
            (ValueId::new(3).unwrap(), ScalarType::Integer(byte())),
        ])
        .unwrap();
        let x = value(1, byte());
        let cast = value(2, half_word());
        let axioms = vec![equal(
            &cast,
            &ScalarTerm::integer_widen(byte(), half_word(), x.clone()).unwrap(),
        )];
        let goal = at_most(&cast, &literal(half_word(), 5));
        Self {
            context,
            x,
            y: value(3, byte()),
            cast,
            axioms,
            goal,
        }
    }

    fn request<'a>(&'a self, assumptions: &'a [Proposition]) -> Request<'a> {
        Request {
            context: &self.context,
            goal: &self.goal,
            assumptions,
            semantic_axioms: &self.axioms,
        }
    }

    fn alone(&self, assumptions: &[Proposition], source: RootSource) -> ProofNode {
        select(
            self.request(assumptions),
            &[source],
            Completion::CastCustody,
        )
        .unwrap_or_else(|| panic!("{source:?} completes the goal on its own"))
    }

    fn emitted(&self, assumptions: &[Proposition]) -> ProofNode {
        let proof = prove_cast_bound(&self.context, &self.goal, assumptions, &self.axioms)
            .expect("cast custody proves the widened bound");
        check_certificate(&self.context, &self.goal, assumptions, &self.axioms, &proof).unwrap();
        proof
    }

    fn certificate(&self, root_bound: ProofNode) -> ProofNode {
        ProofNode {
            conclusion: self.goal.clone(),
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(root_bound),
                witness: IntegerCastChainWitness {
                    root: self.x.clone(),
                    target: self.cast.clone(),
                    definition_axioms: vec![0],
                },
            },
        }
    }
}

fn assert_precedes(sources: &[RootSource], earlier: RootSource, later: RootSource) {
    let position = |source| sources.iter().position(|&candidate| candidate == source);
    assert!(position(earlier).unwrap() < position(later).unwrap());
}

#[test]
fn cast_custody_emits_the_retained_bound_before_its_one_alias_transport() {
    let fixture = CastFixture::new();
    let five = literal(byte(), 5);
    let assumptions = vec![
        at_most(&fixture.x, &five),
        equal(&fixture.x, &fixture.y),
        at_most(&fixture.y, &five),
    ];
    let retained = fixture.alone(&assumptions, RootSource::RetainedBound);
    let one_alias = fixture.alone(&assumptions, RootSource::OneAlias);
    assert_ne!(retained, one_alias);

    let emitted = fixture.emitted(&assumptions);
    assert_eq!(emitted, retained);
    assert_eq!(emitted, fixture.certificate(assumption(&assumptions, 0)));
    assert_precedes(
        &CAST_CUSTODY_SOURCES,
        RootSource::RetainedBound,
        RootSource::OneAlias,
    );
}

#[test]
fn cast_custody_emits_the_landed_literal_before_its_one_alias_transport() {
    let fixture = CastFixture::new();
    let assumptions = vec![
        equal(&fixture.x, &literal(byte(), 3)),
        equal(&fixture.x, &fixture.y),
        at_most(&fixture.y, &literal(byte(), 5)),
    ];
    let landed = fixture.alone(&assumptions, RootSource::GoalLandedLiteral);
    let one_alias = fixture.alone(&assumptions, RootSource::OneAlias);
    assert_ne!(landed, one_alias);

    let emitted = fixture.emitted(&assumptions);
    assert_eq!(emitted, landed);
    let closed = ProofNode {
        conclusion: at_most(&literal(byte(), 3), &literal(byte(), 5)),
        rule: ProofRule::Primitive(proof_admission::PrimitiveJudgment::ClosedIntegerRelation),
    };
    let root_bound = ProofNode {
        conclusion: at_most(&fixture.x, &literal(byte(), 5)),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(closed),
            equality: Box::new(assumption(&assumptions, 0)),
            endpoint: 0,
        },
    };
    assert_eq!(emitted, fixture.certificate(root_bound));
    assert_precedes(
        &CAST_CUSTODY_SOURCES,
        RootSource::GoalLandedLiteral,
        RootSource::OneAlias,
    );
}

/// `x`, `t == x + 1`, `y`, `w` are bytes; goal `t <= 6`.
struct AffineFixture {
    context: PropositionContext,
    x: ScalarTerm,
    sum: ScalarTerm,
    y: ScalarTerm,
    w: ScalarTerm,
    axioms: Vec<Proposition>,
    goal: Proposition,
}

impl AffineFixture {
    fn new() -> Self {
        let context = PropositionContext::from_value_types(
            (1..=4).map(|identity| (ValueId::new(identity).unwrap(), ScalarType::Integer(byte()))),
        )
        .unwrap();
        let x = value(1, byte());
        let sum = value(2, byte());
        let axioms = vec![equal(
            &sum,
            &ScalarTerm::exact_integer_add(byte(), x.clone(), literal(byte(), 1)).unwrap(),
        )];
        let goal = at_most(&sum, &literal(byte(), 6));
        Self {
            context,
            x,
            sum,
            y: value(3, byte()),
            w: value(4, byte()),
            axioms,
            goal,
        }
    }

    fn alone(&self, assumptions: &[Proposition], source: RootSource) -> ProofNode {
        let mut definitions = DefinitionIndex::new(&self.context, assumptions, &self.axioms);
        select(
            Request {
                context: &self.context,
                goal: &self.goal,
                assumptions,
                semantic_axioms: &self.axioms,
            },
            &[source],
            Completion::AffineCustody(&mut definitions),
        )
        .unwrap_or_else(|| panic!("{source:?} completes the goal on its own"))
    }

    fn certificate(&self, root_bound: ProofNode) -> ProofNode {
        ProofNode {
            conclusion: self.goal.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(root_bound),
                witness: IntegerAffineWitness {
                    root: self.x.clone(),
                    target: self.sum.clone(),
                    literal_axioms: vec![None],
                    definition_axioms: vec![0],
                },
            },
        }
    }
}

#[test]
fn affine_custody_emits_the_retained_bound_before_the_closed_landed_literal() {
    let fixture = AffineFixture::new();
    let assumptions = vec![
        at_most(&fixture.x, &literal(byte(), 5)),
        equal(&fixture.x, &literal(byte(), 5)),
    ];
    let retained = fixture.alone(&assumptions, RootSource::RetainedBound);
    let landed = fixture.alone(&assumptions, RootSource::ClosedLandedLiteral);
    assert_ne!(retained, landed);

    let mut definitions = DefinitionIndex::new(&fixture.context, &assumptions, &fixture.axioms);
    let emitted = prove_affine_bound(
        &fixture.context,
        &fixture.goal,
        &assumptions,
        &fixture.axioms,
        &mut definitions,
    )
    .expect("affine custody proves the offset bound");
    check_certificate(
        &fixture.context,
        &fixture.goal,
        &assumptions,
        &fixture.axioms,
        &emitted,
    )
    .unwrap();
    assert_eq!(emitted, retained);
    assert_eq!(emitted, fixture.certificate(assumption(&assumptions, 0)));
    assert_precedes(
        &AFFINE_CUSTODY_SOURCES,
        RootSource::RetainedBound,
        RootSource::ClosedLandedLiteral,
    );
}

#[test]
fn affine_custody_emits_the_one_alias_transport_before_the_two_citation_chain() {
    let fixture = AffineFixture::new();
    let five = literal(byte(), 5);
    let assumptions = vec![
        equal(&fixture.x, &fixture.y),
        at_most(&fixture.y, &five),
        at_most(&fixture.x, &fixture.w),
        at_most(&fixture.w, &five),
    ];
    let one_alias = fixture.alone(&assumptions, RootSource::OneAlias);
    let chain = fixture.alone(&assumptions, RootSource::TwoCitationChain);
    assert_ne!(one_alias, chain);

    let mut definitions = DefinitionIndex::new(&fixture.context, &assumptions, &fixture.axioms);
    let emitted = prove_affine_bound_without_cast(
        &fixture.context,
        &fixture.goal,
        &assumptions,
        &fixture.axioms,
        &mut definitions,
    )
    .expect("affine custody proves the aliased offset bound");
    check_certificate(
        &fixture.context,
        &fixture.goal,
        &assumptions,
        &fixture.axioms,
        &emitted,
    )
    .unwrap();
    assert_eq!(emitted, one_alias);
    let root_bound = ProofNode {
        conclusion: at_most(&fixture.x, &five),
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(assumption(&assumptions, 1)),
            equality: Box::new(assumption(&assumptions, 0)),
            endpoint: 0,
        },
    };
    assert_eq!(emitted, fixture.certificate(root_bound));
    assert_precedes(
        &AFFINE_CUSTODY_SOURCES,
        RootSource::OneAlias,
        RootSource::TwoCitationChain,
    );
}
