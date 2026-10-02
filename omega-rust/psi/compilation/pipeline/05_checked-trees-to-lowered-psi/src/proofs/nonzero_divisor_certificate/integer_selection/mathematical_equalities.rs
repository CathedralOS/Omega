//! Compose scoped mathematical equalities with checked ring identities.
//!
//! SSA transport preserves mathematical constructors, so a retained equality
//! may differ from a goal by exact arithmetic on either endpoint. Cite that
//! equality unchanged and prove both connecting identities with the existing
//! primitive. No normalization result becomes an ambient axiom. This bounded
//! producer searches one retained equality at a time in the canonical forward
//! orientation supported by the existing equality-transitivity checker.

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule, decide_primitive};
use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext};

use super::super::integer_evidence::projected_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::IntegerMathEqual(..) = goal else {
        return None;
    };
    for fact in projected_facts(assumptions, semantic_axioms) {
        super::super::search_context::spend(1)?;
        let Proposition::IntegerMathEqual(..) = fact.proposition else {
            continue;
        };
        for citation in super::super::value_transport::citation_forms(
            context,
            fact.proof(),
            assumptions,
            semantic_axioms,
        ) {
            if let Some(proof) = compose_citation(context, goal, citation) {
                return Some(proof);
            }
        }
    }
    None
}

fn compose_citation(
    context: &PropositionContext,
    goal: &Proposition,
    citation: ProofNode,
) -> Option<ProofNode> {
    let (Proposition::IntegerMathEqual(left, right), Proposition::IntegerMathEqual(first, second)) =
        (goal, &citation.conclusion)
    else {
        return None;
    };
    let prefix = (left != first).then(|| identity(context, left, first));
    let suffix = (second != right).then(|| identity(context, second, right));
    let mut proof = citation;
    if let Some(prefix) = prefix {
        proof = compose(prefix?, proof)?;
    }
    if let Some(suffix) = suffix {
        proof = compose(proof, suffix?)?;
    }
    (proof.conclusion == *goal).then_some(proof)
}

fn identity(
    context: &PropositionContext,
    left: &IntegerMathTerm,
    right: &IntegerMathTerm,
) -> Option<ProofNode> {
    // Mathematical equality uses canonical operands. Do not manufacture a
    // reversed intermediate proposition to satisfy the directed rule.
    if left > right {
        return None;
    }
    super::super::search_context::spend(1)?;
    let conclusion = Proposition::IntegerMathEqual(left.clone(), right.clone());
    decide_primitive(
        context,
        &conclusion,
        PrimitiveJudgment::ClosedIntegerRelation,
    )
    .ok()?;
    Some(ProofNode {
        conclusion,
        rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
    })
}

fn compose(first: ProofNode, second: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::IntegerMathEqual(left, first_middle),
        Proposition::IntegerMathEqual(second_middle, right),
    ) = (&first.conclusion, &second.conclusion)
    else {
        return None;
    };
    if first_middle != second_middle || left > right {
        return None;
    }
    Some(ProofNode {
        conclusion: Proposition::IntegerMathEqual(left.clone(), right.clone()),
        rule: ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(first),
            middle_equals_right: Box::new(second),
        },
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use proof_admission::check_certificate;
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionContext,
        ScalarTerm, ScalarType, ValueId,
    };

    use crate::proofs::nonzero_divisor_certificate::produce_checked_canonical_integer_proof;

    #[test]
    fn mathematical_equality_composition_retains_scoped_premise_and_exact_updates() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let scalar = ScalarType::Integer(integer);
        let context = PropositionContext::from_value_types(
            (1..=8).map(|raw| (ValueId::new(raw).unwrap(), scalar)),
        )
        .unwrap();
        let value = |raw| ScalarTerm::value(ValueId::new(raw).unwrap(), scalar);
        let mathematical = |raw| IntegerMathTerm::MathValue {
            source_type: integer,
            value: ValueId::new(raw).unwrap(),
        };
        let expression = |acc, limit, index| {
            IntegerMathTerm::Subtract(
                Box::new(IntegerMathTerm::Add(
                    Box::new(mathematical(acc)),
                    Box::new(mathematical(limit)),
                )),
                Box::new(mathematical(index)),
            )
        };
        let premise = Proposition::IntegerMathEqual(expression(1, 2, 3), expression(4, 5, 6));
        let goal = Proposition::IntegerMathEqual(expression(1, 2, 3), expression(7, 5, 8));
        let increment = |source, amount| {
            ScalarTerm::exact_integer_add(
                integer,
                value(source),
                ScalarTerm::integer(integer, IntegerValue::Unsigned(amount)).unwrap(),
            )
            .unwrap()
        };
        let equations = [
            Proposition::Equal(value(7), increment(4, 1)),
            Proposition::Equal(value(8), increment(6, 1)),
        ];
        let produce = |requirements: &[Proposition], equations: &[Proposition]| {
            produce_checked_canonical_integer_proof(
                &context,
                &goal,
                requirements,
                equations,
                &BTreeSet::new(),
                &mut crate::proofs::ProofSearchDeclines::default(),
            )
        };
        let requirements = [premise.clone()];
        let proof = produce(&requirements, &equations)
            .expect("retained conservation composes with equal exact increments");
        check_certificate(&context, &goal, &requirements, &equations, &proof).unwrap();
        // Invocation and header aliases are independently retained scalar
        // equations. Expanding the goal must also preserve a transported view
        // of the original conservation citation, with that same evidence.
        let mut aliased = equations.to_vec();
        aliased.push(Proposition::Equal(value(2), value(5)));
        let alias_proof = produce(&requirements, &aliased)
            .expect("entry/header alias does not hide retained conservation");
        check_certificate(&context, &goal, &requirements, &aliased, &alias_proof).unwrap();
        let normalized_goal =
            Proposition::IntegerMathEqual(expression(1, 5, 3), expression(7, 5, 8));
        let normalized = produce_checked_canonical_integer_proof(
            &context,
            &normalized_goal,
            &requirements,
            &aliased,
            &BTreeSet::new(),
            &mut crate::proofs::ProofSearchDeclines::default(),
        )
        .expect("original citation reaches the normalized namespace");
        check_certificate(
            &context,
            &normalized_goal,
            &requirements,
            &aliased,
            &normalized,
        )
        .unwrap();
        assert!(
            check_certificate(
                &context,
                &normalized_goal,
                &requirements,
                &equations,
                &normalized
            )
            .is_err()
        );
        let mut substituted_alias = aliased.clone();
        substituted_alias[2] = Proposition::Equal(value(2), value(6));
        assert!(
            check_certificate(
                &context,
                &normalized_goal,
                &requirements,
                &substituted_alias,
                &normalized,
            )
            .is_err()
        );
        assert!(produce(&[], &equations).is_none());
        assert!(check_certificate(&context, &goal, &[], &equations, &proof).is_err());
        let wrong_middle = [Proposition::IntegerMathEqual(
            expression(1, 2, 3),
            expression(4, 5, 3),
        )];
        assert!(produce(&wrong_middle, &equations).is_none());
        assert!(check_certificate(&context, &goal, &wrong_middle, &equations, &proof).is_err());
        let wrong_update = [
            equations[0].clone(),
            Proposition::Equal(value(8), increment(6, 2)),
        ];
        assert!(produce(&requirements, &wrong_update).is_none());
        assert!(check_certificate(&context, &goal, &requirements, &wrong_update, &proof).is_err());
        assert!(
            check_certificate(
                &PropositionContext::default(),
                &goal,
                &requirements,
                &equations,
                &proof,
            )
            .is_err()
        );
        let conditional = [Proposition::Implication {
            premise: Box::new(Proposition::Falsehood),
            conclusion: Box::new(premise),
        }];
        assert!(produce(&conditional, &equations).is_none());
    }
}
