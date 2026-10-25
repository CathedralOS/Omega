use super::{BoundedDenotation, Denotation, Elaboration};
use crate::proof::assumption_proof;
use crate::{Budget, IntegerAffineWitness, ProofNode, ProofRule, verify_bounded_certificate};
use semantic_vocabulary::{
    IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
    PropositionError, ScalarTerm, ScalarType, ValueId,
};
use std::collections::BTreeSet;

/// The `PredicateDenotation` proof of a single assumption premise,
/// verified against the goal with no semantic axioms and a default
/// budget — the preamble every denotation test used to restate.
pub(crate) fn denote_via_assumption_premise(
    context: &PropositionContext,
    goal: &Proposition,
    premise: &Proposition,
    message: &str,
) -> BoundedDenotation {
    let proof = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::PredicateDenotation {
            premise: Box::new(assumption_proof(premise.clone(), 0)),
        },
    };
    verify_bounded_certificate(
        context,
        goal,
        std::slice::from_ref(premise),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .expect(message)
}

pub(crate) fn mathematical(term: &ScalarTerm) -> IntegerMathTerm {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: ScalarType::Integer(source_type),
        } => IntegerMathTerm::MathValue {
            source_type: *source_type,
            value: *id,
        },
        ScalarTerm::Integer { value, .. } => IntegerMathTerm::literal(*value),
        _ => unreachable!(),
    }
}

/// A skipped numeric resource refusal stays refused: the open
/// operation embedding a value verifies while the closed giant term
/// keeps failing resource admission.
pub(crate) fn open_operation_preserves_skipped_numeric_resource_refusals(
    operation: fn(Box<IntegerMathTerm>, Box<IntegerMathTerm>) -> IntegerMathTerm,
) {
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let value = ValueId::new(1).unwrap();
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer))]).unwrap();
    let large = IntegerMathTerm::ShiftLeft {
        value: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(1))),
        count: Box::new(IntegerMathTerm::literal(IntegerValue::Unsigned(65_536))),
    };
    let open = operation(
        Box::new(IntegerMathTerm::MathValue {
            source_type: integer,
            value,
        }),
        Box::new(large.clone()),
    );
    let goal = Proposition::IntegerMathEqual(open.clone(), open);
    let proof = assumption_proof(goal.clone(), 0);
    verify_bounded_certificate(
        &context,
        &goal,
        std::slice::from_ref(&goal),
        &[],
        &proof,
        &mut Budget::default(),
    )
    .unwrap();
    let closed_goal = Proposition::IntegerMathEqual(large.clone(), large);
    let closed_proof = assumption_proof(closed_goal.clone(), 0);
    assert_refused(
        &context,
        &closed_goal,
        std::slice::from_ref(&closed_goal),
        &[],
        &closed_proof,
    );
}

pub(super) fn direct_fixture(
    context: &PropositionContext,
    goal: &Proposition,
    premise: &Proposition,
    proof: &ProofNode,
) -> Denotation {
    verify_bounded_certificate(
        context,
        goal,
        std::slice::from_ref(premise),
        &[],
        proof,
        &mut Budget::default(),
    )
    .unwrap();
    let parameters = BTreeSet::new();
    let assumptions = [premise.clone()];
    let mut elaboration = Elaboration::new(context, goal, &assumptions, &[], &parameters).unwrap();
    elaboration.node(proof).unwrap();
    elaboration.denotation
}

pub(crate) fn direct_affine_proof(
    integer: IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    premise: &Proposition,
    goal: &Proposition,
    operation: fn(IntegerType, ScalarTerm, ScalarTerm) -> Result<ScalarTerm, PropositionError>,
) -> ProofNode {
    ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(assumption_proof(premise.clone(), 0)),
            witness: IntegerAffineWitness {
                root: left.clone(),
                target: operation(integer, left, right).unwrap(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            },
        },
    }
}

/// Kernel-verify `proof` against `goal` under `assumptions` with a
/// default budget, then elaborate it for the rule-and-axiom assertions
/// below — the prelude the bound tests ran by hand.
pub(super) fn verify_then_elaborate<'a>(
    context: &'a PropositionContext,
    goal: &Proposition,
    assumptions: &'a [Proposition],
    machine_parameter_values: &'a BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Elaboration<'a> {
    verify_bounded_certificate(
        context,
        goal,
        assumptions,
        &[],
        proof,
        &mut Budget::default(),
    )
    .unwrap();
    elaborate_checked(
        context,
        goal,
        assumptions,
        &[],
        machine_parameter_values,
        proof,
    )
}

/// Elaborate `proof` against `goal` under `assumptions`/`semantic_axioms`
/// — the prelude the bounded tests run before asserting on the recorded
/// rule and axiom rosters.
pub(super) fn elaborate_checked<'a>(
    context: &'a PropositionContext,
    goal: &Proposition,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    machine_parameter_values: &'a BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Elaboration<'a> {
    let mut elaboration = Elaboration::new(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    )
    .unwrap();
    elaboration.node(proof).unwrap();
    elaboration
}

/// Assert the checker refuses a certificate the ambient premises do not
/// license — the tampered-goal, tampered-premise and tampered-axiom
/// controls every bounded suite restated by hand.
pub(crate) fn assert_refused(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) {
    assert!(
        verify_bounded_certificate(
            context,
            goal,
            assumptions,
            semantic_axioms,
            proof,
            &mut Budget::default(),
        )
        .is_err()
    );
}

/// Verify `proof`'s certificate under the ambient premises, assert the
/// kernel rejects a forged term, and hand the denotation back for the
/// caller's remaining checks.
pub(crate) fn verify_and_assert_forged(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> BoundedDenotation {
    let mut denoted = verify_bounded_certificate(
        context,
        goal,
        assumptions,
        semantic_axioms,
        proof,
        &mut Budget::default(),
    )
    .unwrap();
    crate::forge_term_and_assert_rejected(&mut denoted);
    denoted
}
