/*
    Explicit `ProofNode` certificate traversal, local rule verification,
    trust-closure recording (`CertificateAcceptance`), and mathematical-core
    re-decision.

    We walk bounded certificates in scoped postorder on an explicit work stack,
    verify propositional, equality, integer-order, and integer-bound rules
    against their cited premises, and require agreement from the common
    mathematical core on every covered certificate.
*/

use std::collections::BTreeSet;
pub use terminal_psi::{ProofNode, ProofRule};

use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext, ScalarTerm, ValueId};

use crate::mathematical_core::{
    BoundedDenotationError, Budget, verify_bounded_certificate_with_machine_parameters,
};
use crate::{
    IntegerAffineBoundConversionError, IntegerAffineWitnessError, IntegerCastBoundConversionError,
    IntegerCastChainWitnessError, IntegerCorrelatedForbiddenRootConversionError,
    IntegerCorrelatedForbiddenRootWitnessError, KernelError,
};
use equality_rules::{check_equality_symmetry, check_equality_transitivity};
use integer_order_rules::{
    check_integer_add_order, check_integer_order_weakening, check_integer_subtract_order,
};
use propositional_rules::{check_assumption, check_conjunction_introduction, check_semantic_axiom};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AcceptedProofRule {
    Primitive,
    SemanticAxiom,
    Assumption,
    ConjunctionIntroduction,
    ConjunctionElimination,
    DisjunctionIntroduction,
    DisjunctionElimination,
    ImplicationIntroduction,
    ImplicationElimination,
    EqualityTransitivity,
    EqualitySymmetry,
    PredicateDenotation,
    ValueEqualityTransport,
    IntegerOrderWeakening,
    IntegerOrderDiscreteness,
    IntegerSubtractOrder,
    IntegerAddOrder,
    IntegerSubtractAntitone,
    IntegerLessOrEqualTransitivity,
    IntegerStrictOrderTransitivity,
    IntegerOrderSubstitution,
    IntegerMathOrderTranslation,
    IntegerOrderAntisymmetry,
    IntegerAffineBound,
    /// The `IntegerAffineBound` form that bounds a remainder by its dividend.
    /// It is a row of its own because it is provisional
    /// (`AcceptedProofRule::is_provisional`).
    IntegerRemainderWithinDividend,
    /// The `IntegerAffineBound` form that relates a quotient to its
    /// dividend; provisional, like the row above.
    IntegerQuotientAtMostDividend,
    IntegerExactAddDefinitionBound,
    IntegerExactSubtractDefinitionBound,
    IntegerCastBound,
    IntegerCorrelatedForbiddenRoots,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedPremise {
    pub index: usize,
    pub proposition: Proposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateAcceptance {
    pub rules: Vec<AcceptedProofRule>,
    pub assumptions: Vec<AcceptedPremise>,
    pub semantic_axioms: Vec<AcceptedPremise>,
    pub mathematical_core: MathematicalCoreDecision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathematicalCoreDecision {
    Judged(MathematicalJudgmentReceipt),
    Refused(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalJudgmentReceipt {
    pub declarations: u32,
    pub assumption_closure: u32,
    pub context_depth: u32,
    pub arena_slots: u32,
}

#[derive(Default)]
struct AcceptanceBuilder {
    ambient_assumption_count: usize,
    rules: std::collections::BTreeSet<AcceptedProofRule>,
    assumptions: PremiseRoster,
    semantic_axioms: PremiseRoster,
}

impl AcceptanceBuilder {
    fn record_assumption(&mut self, index: usize, proposition: &Proposition) {
        if index < self.ambient_assumption_count {
            self.assumptions.record(index, proposition);
        }
    }

    fn record_semantic_axiom(&mut self, index: usize, proposition: &Proposition) {
        self.semantic_axioms.record(index, proposition);
    }

    fn finish(self, mathematical_core: MathematicalCoreDecision) -> CertificateAcceptance {
        CertificateAcceptance {
            rules: self.rules.into_iter().collect(),
            assumptions: self.assumptions.premises,
            semantic_axioms: self.semantic_axioms.premises,
            mathematical_core,
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct PremiseRoster {
    pub(crate) premises: Vec<AcceptedPremise>,
    positions: std::collections::BTreeMap<usize, Vec<usize>>,
}

impl PremiseRoster {
    pub(crate) fn record(&mut self, index: usize, proposition: &Proposition) {
        if let Some(recorded) = self.positions.get(&index)
            && recorded
                .iter()
                .any(|position| self.premises[*position].proposition == *proposition)
        {
            return;
        }
        let position = self.premises.len();
        self.premises.push(AcceptedPremise {
            index,
            proposition: proposition.clone(),
        });
        self.positions.entry(index).or_default().push(position);
    }
}

pub fn check_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<(), ProofError> {
    accept_certificate(context, goal, assumptions, semantic_axioms, proof).map(|_| ())
}

pub fn accept_certificate(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    proof: &ProofNode,
) -> Result<CertificateAcceptance, ProofError> {
    accept_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &BTreeSet::new(),
        proof,
    )
}

pub fn accept_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<CertificateAcceptance, ProofError> {
    #[cfg(test)]
    crate::mathematical_core::bounded_observations::record_admission();
    context
        .validate(goal)
        .map_err(ProofError::MalformedProposition)?;
    for assumption in assumptions {
        context
            .validate(assumption)
            .map_err(ProofError::MalformedProposition)?;
    }
    for axiom in semantic_axioms {
        context
            .validate(axiom)
            .map_err(ProofError::MalformedProposition)?;
    }
    let mut acceptance = AcceptanceBuilder {
        ambient_assumption_count: assumptions.len(),
        ..AcceptanceBuilder::default()
    };
    check_node(
        context,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
        &mut acceptance,
    )?;
    if &proof.conclusion != goal {
        return Err(ProofError::CertificateConclusionMismatch);
    }
    let mathematical_core = re_decide_in_mathematical_core(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
    )?;
    Ok(acceptance.finish(mathematical_core))
}

fn re_decide_in_mathematical_core(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<MathematicalCoreDecision, ProofError> {
    match verify_bounded_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
        &mut Budget::default(),
    ) {
        Ok(denoted) => Ok(MathematicalCoreDecision::Judged(denoted.receipt())),
        Err(BoundedDenotationError::Unsupported(family)) => {
            Ok(MathematicalCoreDecision::Refused(family))
        }
        Err(error) => Err(ProofError::MathematicalCore(Box::new(error))),
    }
}

pub(crate) mod add_order {
    use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm, ScalarType};

    use super::ProofError;

    pub(crate) fn check(
        sum: &Proposition,
        positive: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let Proposition::Equal(
            result,
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            },
        ) = sum
        else {
            return Err(ProofError::RulePremiseMismatch("integer addition order"));
        };
        let Proposition::LessThan(zero, increment) = positive else {
            return Err(ProofError::RulePremiseMismatch(
                "integer addition positivity",
            ));
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed
            || scalar_type.is_address()
            || result.scalar_type() != ScalarType::Integer(*scalar_type)
            || zero.scalar_type() != ScalarType::Integer(*scalar_type)
            || increment != right.as_ref()
            || !matches!(
                zero.integer_value(),
                Some((_, IntegerValue::Signed(0) | IntegerValue::Unsigned(0)))
            )
        {
            return Err(ProofError::RulePremiseMismatch("integer addition order"));
        }
        (conclusion == &Proposition::LessThan(left.as_ref().clone(), result.clone()))
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
    }
}
pub(crate) mod equality_rules {
    use super::dispatched_rule;
    use super::integer_math_normalization::propositions_match_under_integer_math_normalization;
    use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
    use semantic_vocabulary::{Proposition, PropositionContext};
    use terminal_psi::{ProofNode, ProofRule};

    pub(super) fn check_equality_symmetry(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::EqualitySymmetry { equality } => EqualitySymmetry);
        let Proposition::Equal(left, right) = &equality.conclusion else {
            return Err(ProofError::RulePremiseMismatch("equality symmetry"));
        };
        (proof.conclusion == Proposition::Equal(right.clone(), left.clone()))
            .then_some(())
            .ok_or(ProofError::EqualityConclusionMismatch)
    }

    pub(crate) fn predicate_denotation_relation(
        context: &PropositionContext,
        premise: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let original = crate::check_predicate_denotations(context, premise, &[], &[])
            .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
        let converted = crate::check_predicate_denotations(context, conclusion, &[], &[])
            .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
        if original.goal() != converted.goal() {
            return Err(ProofError::RuleConclusionMismatch("predicate denotation"));
        }
        Ok(())
    }

    pub(super) fn check_predicate_denotation(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::PredicateDenotation { premise } => PredicateDenotation);
        let RuleScope { context, .. } = *scope;
        predicate_denotation_relation(context, &premise.conclusion, &proof.conclusion)?;
        Ok(())
    }

    pub(crate) fn value_equality_transport_relation<'a>(
        context: &PropositionContext,
        premise: &Proposition,
        equalities: impl Iterator<Item = &'a Proposition> + Clone,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let original = crate::check_value_equality_denotation(context, premise, equalities.clone())
            .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
        let transported = crate::check_value_equality_denotation(context, conclusion, equalities)
            .map_err(|error| ProofError::PredicateDenotation(Box::new(error)))?;
        if original != transported {
            return Err(ProofError::RuleConclusionMismatch(
                "value equality transport",
            ));
        }
        Ok(())
    }

    pub(super) fn check_value_equality_transport(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::ValueEqualityTransport { premise, equalities } => ValueEqualityTransport);
        let RuleScope { context, .. } = *scope;
        let equations = || equalities.iter().map(|equality| &equality.conclusion);
        value_equality_transport_relation(
            context,
            &premise.conclusion,
            equations(),
            &proof.conclusion,
        )?;
        Ok(())
    }

    pub(crate) fn equality_transitivity_relation(
        left_equals_middle: &Proposition,
        middle_equals_right: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        match (left_equals_middle, middle_equals_right) {
            (Proposition::Equal(left, first_middle), Proposition::Equal(second_middle, right)) => {
                if first_middle != second_middle {
                    return Err(ProofError::EqualityMiddleMismatch);
                }
                let composed = Proposition::Equal(left.clone(), right.clone());
                if !propositions_match_under_integer_math_normalization(&composed, conclusion) {
                    return Err(ProofError::EqualityConclusionMismatch);
                }
                Ok(())
            }
            (
                Proposition::IntegerMathEqual(left, first_middle),
                Proposition::IntegerMathEqual(second_middle, right),
            ) => {
                if first_middle != second_middle {
                    return Err(ProofError::EqualityMiddleMismatch);
                }
                let mut left = left.clone();
                let mut right = right.clone();
                if left > right {
                    std::mem::swap(&mut left, &mut right);
                }
                (conclusion == &Proposition::IntegerMathEqual(left, right))
                    .then_some(())
                    .ok_or(ProofError::EqualityConclusionMismatch)
            }
            (
                Proposition::ContentConservation(left_equation),
                Proposition::ContentConservation(right_equation),
            ) => {
                let Proposition::ContentConservation(expected) = conclusion else {
                    return Err(ProofError::RuleConclusionMismatch("equality transitivity"));
                };
                if left_equation.algebra() != right_equation.algebra()
                    || left_equation.algebra() != expected.algebra()
                {
                    return Err(ProofError::EqualityAlgebraMismatch);
                }
                let left_terms = [left_equation.left(), left_equation.right()];
                let right_terms = [right_equation.left(), right_equation.right()];
                let mut shared_middle = false;
                for (left_index, left_term) in left_terms.iter().enumerate() {
                    for (right_index, right_term) in right_terms.iter().enumerate() {
                        if left_term != right_term {
                            continue;
                        }
                        shared_middle = true;
                        let composed = semantic_vocabulary::ContentConservation::new(
                            left_equation.algebra().clone(),
                            left_terms[1 - left_index].clone(),
                            right_terms[1 - right_index].clone(),
                        );
                        if &composed == expected {
                            return Ok(());
                        }
                    }
                }
                Err(if shared_middle {
                    ProofError::EqualityConclusionMismatch
                } else {
                    ProofError::EqualityMiddleMismatch
                })
            }
            _ => Err(ProofError::RulePremiseMismatch("equality transitivity")),
        }
    }

    pub(super) fn check_equality_transitivity(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::EqualityTransitivity { left_equals_middle, middle_equals_right } => EqualityTransitivity);
        equality_transitivity_relation(
            &left_equals_middle.conclusion,
            &middle_equals_right.conclusion,
            &proof.conclusion,
        )
    }
}
pub(crate) mod integer_bound_rules {
    use std::collections::BTreeSet;

    use super::dispatched_rule;
    use super::integer_math_normalization::lower_integer_math_relation;
    use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
    use crate::{
        CheckedIntegerCastChain, CheckedIntegerCorrelatedForbiddenRoots,
        IntegerAffineBoundConversionError, IntegerAffineWitness, IntegerCastBoundConversionError,
        IntegerCorrelatedForbiddenRootWitness, check_integer_affine_bound_conversion,
        check_integer_affine_witness, check_integer_cast_bound_conversion,
        check_integer_cast_chain_witness, check_integer_correlated_forbidden_root_conversion,
        check_integer_correlated_forbidden_root_witness, integer_affine_truth_bounds,
        integer_cast_truth_bounds, map_integer_affine_bound,
    };
    use semantic_vocabulary::IntegerType;
    use semantic_vocabulary::{
        IntegerMathTerm, Proposition, PropositionContext, ScalarTerm, ValueId,
    };
    use terminal_psi::{ProofNode, ProofRule};

    pub(crate) fn affine_bound_relation(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        root_bound: &Proposition,
        witness: &IntegerAffineWitness,
        conclusion: &Proposition,
    ) -> Result<(AcceptedProofRule, Vec<usize>), ProofError> {
        let form = check_integer_affine_witness(context, semantic_axioms, witness)
            .map_err(ProofError::IntegerAffineWitness)?;
        let rule = if form.is_remainder_within_dividend() {
            AcceptedProofRule::IntegerRemainderWithinDividend
        } else if form.is_quotient_at_most_dividend() {
            AcceptedProofRule::IntegerQuotientAtMostDividend
        } else {
            AcceptedProofRule::IntegerAffineBound
        };
        let normalized_conclusion =
            lower_integer_math_relation(conclusion).unwrap_or_else(|| conclusion.clone());
        if *root_bound == Proposition::Truth {
            let bounds = integer_affine_truth_bounds(&form)
                .map_err(ProofError::IntegerAffineBoundConversion)?;
            if !bounds.contains(&normalized_conclusion) {
                return Err(ProofError::IntegerAffineBoundConversion(
                    IntegerAffineBoundConversionError::ConclusionMismatch,
                ));
            }
        } else {
            check_integer_affine_bound_conversion(&form, root_bound, &normalized_conclusion)
                .map_err(ProofError::IntegerAffineBoundConversion)?;
        }
        let mut cited = Vec::new();
        for (&definition_index, &literal_index) in witness
            .definition_axioms
            .iter()
            .zip(&witness.literal_axioms)
        {
            if let Some(index) = literal_index {
                if index >= semantic_axioms.len() {
                    return Err(ProofError::UnknownSemanticAxiom(index));
                }
                cited.push(index);
            }
            if definition_index >= semantic_axioms.len() {
                return Err(ProofError::UnknownSemanticAxiom(definition_index));
            }
            cited.push(definition_index);
        }
        Ok((rule, cited))
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum ExactDefinitionOperation {
        Add,
        Subtract,
    }

    impl ExactDefinitionOperation {
        fn definition_label(self) -> &'static str {
            match self {
                Self::Add => "integer exact-add definition",
                Self::Subtract => "integer exact-subtract definition",
            }
        }

        fn type_label(self) -> &'static str {
            match self {
                Self::Add => "integer exact-add definition type",
                Self::Subtract => "integer exact-subtract definition type",
            }
        }

        fn mapped_label(self) -> &'static str {
            match self {
                Self::Add => "integer exact-add mapped bound",
                Self::Subtract => "integer exact-subtract mapped bound",
            }
        }

        fn literal_label(self) -> &'static str {
            match self {
                Self::Add => "integer exact-add mapped literal",
                Self::Subtract => "integer exact-subtract mapped literal",
            }
        }

        fn operands(self, expression: &ScalarTerm) -> Option<(IntegerType, &ScalarTerm)> {
            match (self, expression) {
                (
                    Self::Add,
                    ScalarTerm::ExactIntegerAdd {
                        scalar_type, left, ..
                    },
                )
                | (
                    Self::Subtract,
                    ScalarTerm::ExactIntegerSubtract {
                        scalar_type, left, ..
                    },
                ) => Some((*scalar_type, left.as_ref())),
                _ => None,
            }
        }

        fn is_mapped_term(self, term: &IntegerMathTerm) -> bool {
            matches!(
                (self, term),
                (Self::Add, IntegerMathTerm::Add(_, _))
                    | (Self::Subtract, IntegerMathTerm::Subtract(_, _))
            )
        }
    }

    pub(crate) fn exact_definition_bound_relation(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        left_bound: &Proposition,
        right_bound: &Proposition,
        definition_axiom: usize,
        conclusion: &Proposition,
        operation: ExactDefinitionOperation,
    ) -> Result<(), ProofError> {
        let definition = semantic_axioms
            .get(definition_axiom)
            .ok_or(ProofError::UnknownSemanticAxiom(definition_axiom))?;
        context
            .validate(definition)
            .map_err(ProofError::MalformedProposition)?;
        let Proposition::Equal(first, second) = definition else {
            return Err(ProofError::RulePremiseMismatch(
                operation.definition_label(),
            ));
        };
        let (output, expression) = match (first, second) {
            (ScalarTerm::Value { .. }, expression) if operation.operands(expression).is_some() => {
                (first, second)
            }
            (expression, ScalarTerm::Value { .. }) if operation.operands(expression).is_some() => {
                (second, first)
            }
            _ => {
                return Err(ProofError::RulePremiseMismatch(
                    operation.definition_label(),
                ));
            }
        };
        let (scalar_type, left) = operation
            .operands(expression)
            .expect("matched the exact-operation definition");
        if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
            || output.scalar_type() != semantic_vocabulary::ScalarType::Integer(scalar_type)
        {
            return Err(ProofError::RulePremiseMismatch(operation.type_label()));
        }
        let witness = IntegerAffineWitness {
            root: left.clone(),
            target: expression.clone(),
            definition_axioms: Vec::new(),
            literal_axioms: Vec::new(),
        };
        let form = check_integer_affine_witness(context, semantic_axioms, &witness)
            .map_err(ProofError::IntegerAffineWitness)?;
        let evidence = Proposition::Conjunction(vec![left_bound.clone(), right_bound.clone()]);
        let mapped = map_integer_affine_bound(&form, &evidence)
            .map_err(ProofError::IntegerAffineBoundConversion)?;
        let Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right) = mapped else {
            return Err(ProofError::RulePremiseMismatch(operation.mapped_label()));
        };
        let (literal, lower) = match (&mapped_left, &mapped_right) {
            (IntegerMathTerm::IntegerLiteral(literal), term) if operation.is_mapped_term(term) => {
                (literal, true)
            }
            (term, IntegerMathTerm::IntegerLiteral(literal)) if operation.is_mapped_term(term) => {
                (literal, false)
            }
            _ => {
                return Err(ProofError::RulePremiseMismatch(operation.mapped_label()));
            }
        };
        let value = literal
            .as_integer_value(scalar_type)
            .ok_or(ProofError::RulePremiseMismatch(operation.literal_label()))?;
        let literal = ScalarTerm::integer(scalar_type, value)
            .map_err(|_| ProofError::RulePremiseMismatch(operation.literal_label()))?;
        let expected = if lower {
            Proposition::LessOrEqual(literal, output.clone())
        } else {
            Proposition::LessOrEqual(output.clone(), literal)
        };
        if *conclusion != expected {
            return Err(ProofError::IntegerAffineBoundConversion(
                IntegerAffineBoundConversionError::ConclusionMismatch,
            ));
        }
        Ok(())
    }

    pub(super) fn check_integer_exact_definition_bound(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        let (left_bound, right_bound, definition_axiom, operation, rule) = match &proof.rule {
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => (
                left_bound,
                right_bound,
                definition_axiom,
                ExactDefinitionOperation::Add,
                AcceptedProofRule::IntegerExactAddDefinitionBound,
            ),
            ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                definition_axiom,
            } => (
                left_bound,
                right_bound,
                definition_axiom,
                ExactDefinitionOperation::Subtract,
                AcceptedProofRule::IntegerExactSubtractDefinitionBound,
            ),
            _ => unreachable!("dispatched check_integer_exact_definition_bound"),
        };
        let RuleScope {
            context,
            semantic_axioms,
            ..
        } = *scope;
        acceptance.rules.insert(rule);
        exact_definition_bound_relation(
            context,
            semantic_axioms,
            &left_bound.conclusion,
            &right_bound.conclusion,
            *definition_axiom,
            &proof.conclusion,
            operation,
        )?;
        record_cited_axioms(acceptance, semantic_axioms, [*definition_axiom]);
        Ok(())
    }

    pub(crate) fn cast_bound_relation(
        context: &PropositionContext,
        semantic_axioms: &[Proposition],
        root_bound: &Proposition,
        witness: &crate::IntegerCastChainWitness,
        conclusion: &Proposition,
    ) -> Result<(Vec<usize>, CheckedIntegerCastChain), ProofError> {
        let chain = check_integer_cast_chain_witness(context, semantic_axioms, witness)
            .map_err(ProofError::IntegerCastChainWitness)?;
        let normalized_conclusion =
            lower_integer_math_relation(conclusion).unwrap_or_else(|| conclusion.clone());
        if *root_bound == Proposition::Truth {
            let bounds = integer_cast_truth_bounds(&chain)
                .map_err(ProofError::IntegerCastBoundConversion)?;
            if !bounds.contains(&normalized_conclusion) {
                return Err(ProofError::IntegerCastBoundConversion(
                    IntegerCastBoundConversionError::ConclusionLiteralMismatch,
                ));
            }
        } else {
            check_integer_cast_bound_conversion(&chain, root_bound, &normalized_conclusion)
                .map_err(ProofError::IntegerCastBoundConversion)?;
        }
        let mut cited = Vec::with_capacity(witness.definition_axioms.len());
        for &index in &witness.definition_axioms {
            if index >= semantic_axioms.len() {
                return Err(ProofError::UnknownSemanticAxiom(index));
            }
            cited.push(index);
        }
        Ok((cited, chain))
    }

    pub(super) fn check_integer_cited_bound(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        let RuleScope {
            context,
            semantic_axioms,
            ..
        } = *scope;
        let conclusion = &proof.conclusion;
        let cited = match &proof.rule {
            ProofRule::IntegerAffineBound {
                root_bound,
                witness,
            } => {
                let (rule, cited) = affine_bound_relation(
                    context,
                    semantic_axioms,
                    &root_bound.conclusion,
                    witness,
                    conclusion,
                )?;
                acceptance.rules.insert(rule);
                cited
            }
            ProofRule::IntegerCastBound {
                root_bound,
                witness,
            } => {
                acceptance.rules.insert(AcceptedProofRule::IntegerCastBound);
                let (cited, _chain) = cast_bound_relation(
                    context,
                    semantic_axioms,
                    &root_bound.conclusion,
                    witness,
                    conclusion,
                )?;
                cited
            }
            _ => unreachable!("dispatched check_integer_cited_bound"),
        };
        record_cited_axioms(acceptance, semantic_axioms, cited);
        Ok(())
    }

    fn record_cited_axioms(
        acceptance: &mut AcceptanceBuilder,
        semantic_axioms: &[Proposition],
        cited: impl IntoIterator<Item = usize>,
    ) {
        for index in cited {
            let proposition = semantic_axioms
                .get(index)
                .expect("the relation bounds-checked every cited index");
            acceptance.record_semantic_axiom(index, proposition);
        }
    }

    #[derive(Debug, Default)]
    pub(crate) struct CorrelatedCitations {
        pub semantic_axioms: Vec<usize>,
        pub assumptions: Vec<usize>,
    }

    pub(crate) fn correlated_forbidden_roots_relation(
        context: &PropositionContext,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
        machine_parameter_values: &BTreeSet<ValueId>,
        witness: &IntegerCorrelatedForbiddenRootWitness,
        conclusion: &Proposition,
    ) -> Result<(CorrelatedCitations, CheckedIntegerCorrelatedForbiddenRoots), ProofError> {
        if witness.definition_axiom_count != semantic_axioms.len() {
            return Err(ProofError::IntegerCorrelatedForbiddenRootDefinitionBoundary);
        }
        let mut ledger = Vec::with_capacity(semantic_axioms.len() + assumptions.len());
        ledger.extend_from_slice(semantic_axioms);
        ledger.extend_from_slice(assumptions);
        let checked = check_integer_correlated_forbidden_root_witness(
            context,
            &ledger,
            machine_parameter_values,
            witness,
        )
        .map_err(ProofError::IntegerCorrelatedForbiddenRootWitness)?;
        check_integer_correlated_forbidden_root_conversion(&checked, conclusion)
            .map_err(ProofError::IntegerCorrelatedForbiddenRootConversion)?;

        let mut citations = CorrelatedCitations::default();
        for branch in [&witness.dividend, &witness.divisor] {
            for step in &branch.steps {
                if let Some(index) = step.literal_axiom {
                    if index >= semantic_axioms.len() {
                        return Err(ProofError::UnknownSemanticAxiom(index));
                    }
                    citations.semantic_axioms.push(index);
                }
                let index = step.definition_axiom;
                if index >= semantic_axioms.len() {
                    return Err(ProofError::UnknownSemanticAxiom(index));
                }
                citations.semantic_axioms.push(index);
            }
        }
        let (lower_bound, upper_bound) = checked.bound_axioms();
        for ledger_index in [lower_bound, upper_bound] {
            let index = ledger_index
                .checked_sub(semantic_axioms.len())
                .ok_or(ProofError::IntegerCorrelatedForbiddenRootRequirementBoundary)?;
            if index >= assumptions.len() {
                return Err(ProofError::UnknownAssumption(index));
            }
            citations.assumptions.push(index);
        }
        Ok((citations, checked))
    }

    pub(super) fn check_integer_correlated_forbidden_roots(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerCorrelatedForbiddenRoots { witness } => IntegerCorrelatedForbiddenRoots);
        let RuleScope {
            context,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
            ..
        } = *scope;
        let (citations, _) = correlated_forbidden_roots_relation(
            context,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
            witness,
            &proof.conclusion,
        )?;
        record_cited_axioms(acceptance, semantic_axioms, citations.semantic_axioms);
        for index in citations.assumptions {
            let proposition = assumptions
                .get(index)
                .expect("the relation bounds-checked every cited index");
            acceptance.record_assumption(index, proposition);
        }
        Ok(())
    }
}
pub(crate) mod integer_math_normalization {
    use semantic_vocabulary::{
        IntegerCarrier, IntegerMathTerm, IntegerType, Proposition, ScalarTerm, ScalarType,
    };

    pub fn lift_fixed_integer_relation(proposition: &Proposition) -> Option<Proposition> {
        let (kind, left, right) = match proposition {
            Proposition::Equal(left, right) => (0, left, right),
            Proposition::LessThan(left, right) => (1, left, right),
            Proposition::LessOrEqual(left, right) => (2, left, right),
            _ => return None,
        };
        crate::check_predicate_evaluation_size([proposition]).ok()?;
        let mut left = lift_fixed_integer_term(left)?;
        let mut right = lift_fixed_integer_term(right)?;
        Some(match kind {
            0 => {
                if left > right {
                    std::mem::swap(&mut left, &mut right);
                }
                Proposition::IntegerMathEqual(left, right)
            }
            1 => Proposition::IntegerMathLessThan(left, right),
            _ => Proposition::IntegerMathLessOrEqual(left, right),
        })
    }

    pub fn lower_integer_math_relation(proposition: &Proposition) -> Option<Proposition> {
        let (kind, left, right) = match proposition {
            Proposition::IntegerMathEqual(left, right) => (0, left, right),
            Proposition::IntegerMathLessThan(left, right) => (1, left, right),
            Proposition::IntegerMathLessOrEqual(left, right) => (2, left, right),
            _ => return None,
        };
        crate::check_predicate_evaluation_size([proposition]).ok()?;
        let source_type = [left, right].into_iter().find_map(|term| match term {
            IntegerMathTerm::MathValue { source_type, .. } => Some(*source_type),
            IntegerMathTerm::Embed(term) => match term.scalar_type() {
                ScalarType::Integer(integer) if exact_ring_carrier(&integer) => Some(integer),
                _ => None,
            },
            _ => None,
        })?;
        let lower = |term: &IntegerMathTerm| match term {
            IntegerMathTerm::MathValue {
                source_type: actual,
                value,
            } if *actual == source_type => {
                Some(ScalarTerm::value(*value, ScalarType::Integer(source_type)))
            }
            IntegerMathTerm::IntegerLiteral(literal) => {
                ScalarTerm::integer(source_type, literal.as_integer_value(source_type)?).ok()
            }
            IntegerMathTerm::Embed(term)
                if exact_ring_carrier(&source_type)
                    && term.scalar_type() == ScalarType::Integer(source_type) =>
            {
                Some(term.as_ref().clone())
            }
            _ => None,
        };
        let left = lower(left)?;
        let right = lower(right)?;
        Some(match kind {
            0 => Proposition::Equal(left, right),
            1 => Proposition::LessThan(left, right),
            _ => Proposition::LessOrEqual(left, right),
        })
    }

    pub(crate) fn propositions_match_under_integer_math_normalization(
        retained: &Proposition,
        requested: &Proposition,
    ) -> bool {
        retained == requested
            || lift_fixed_integer_relation(retained).as_ref() == Some(requested)
            || lower_integer_math_relation(retained).as_ref() == Some(requested)
    }

    fn lift_fixed_integer_term(term: &ScalarTerm) -> Option<IntegerMathTerm> {
        exact_ring_carrier(&term.scalar_type().as_integer()?)
            .then(|| IntegerMathTerm::embed(term.clone()))
            .flatten()
    }

    fn exact_ring_carrier(scalar_type: &IntegerType) -> bool {
        scalar_type.carrier() == IntegerCarrier::Fixed && !scalar_type.is_address()
    }

    pub(crate) fn lift_exact_integer_relation(
        proposition: &Proposition,
    ) -> Option<(IntegerMathTerm, IntegerMathTerm)> {
        crate::check_predicate_evaluation_size([proposition]).ok()?;
        let (left, right) = match proposition {
            semantic_vocabulary::ordered_comparison_operands!(left, right) => (left, right),
            _ => return None,
        };
        Some((
            lift_exact_integer_term(left)?,
            lift_exact_integer_term(right)?,
        ))
    }

    pub(crate) fn lift_exact_integer_term(term: &ScalarTerm) -> Option<IntegerMathTerm> {
        match term {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if exact_ring_carrier(scalar_type) => Some(IntegerMathTerm::Add(
                Box::new(lift_exact_integer_term(left)?),
                Box::new(lift_exact_integer_term(right)?),
            )),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if exact_ring_carrier(scalar_type) => Some(IntegerMathTerm::Subtract(
                Box::new(lift_exact_integer_term(left)?),
                Box::new(lift_exact_integer_term(right)?),
            )),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if exact_ring_carrier(scalar_type) => Some(IntegerMathTerm::Multiply(
                Box::new(lift_exact_integer_term(left)?),
                Box::new(lift_exact_integer_term(right)?),
            )),
            // An exact left shift completes only when `value * 2^count` is
            // representable and the count lies below the width (Terminal
            // semantics owes exactly that, over this same mathematical
            // term), so the value it produces is that term, as an exact
            // product's is its mathematical product.
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } if exact_ring_carrier(value_type) && exact_ring_carrier(count_type) => {
                Some(IntegerMathTerm::ShiftLeft {
                    value: Box::new(lift_exact_integer_term(value)?),
                    count: Box::new(lift_exact_integer_term(count)?),
                })
            }
            ScalarTerm::Value { .. } | ScalarTerm::Integer { .. } => lift_fixed_integer_term(term),
            _ => None,
        }
    }

    pub fn lift_exact_integer_order(proposition: &Proposition) -> Option<Proposition> {
        let (left, right) = lift_exact_integer_relation(proposition)?;
        proposition.validate().ok()?;
        match proposition {
            Proposition::LessThan(..) => Some(Proposition::IntegerMathLessThan(left, right)),
            Proposition::LessOrEqual(..) => Some(Proposition::IntegerMathLessOrEqual(left, right)),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::check_certificate;
        use semantic_vocabulary::{
            IntegerSign, IntegerValue, PlaceId, PropositionContext, ScalarType, StructuralFieldId,
            StructuralPlaceKind, ValueId,
        };
        use std::slice::from_ref;
        use terminal_psi::{ProofNode, ProofRule};

        #[test]
        fn an_exact_left_shift_means_its_mathematical_shift_and_a_wrapping_one_does_not() {
            let value_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
            let count_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
            let value =
                ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Integer(value_type));
            let count =
                ScalarTerm::value(ValueId::new(2).unwrap(), ScalarType::Integer(count_type));
            let exact = ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: Box::new(value.clone()),
                count: Box::new(count.clone()),
            };
            assert_eq!(
                lift_exact_integer_term(&exact),
                Some(IntegerMathTerm::ShiftLeft {
                    value: Box::new(IntegerMathTerm::embed(value.clone()).unwrap()),
                    count: Box::new(IntegerMathTerm::embed(count.clone()).unwrap()),
                })
            );
            let wrapping = ScalarTerm::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value: Box::new(value),
                count: Box::new(count),
            };
            assert_eq!(lift_exact_integer_term(&wrapping), None);
        }

        #[test]
        fn embedded_endpoints_preserve_projection_and_operation_meaning() {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let root = PlaceId::new(1).unwrap();
            let value = ValueId::new(1).unwrap();
            let context = PropositionContext::from_value_types_and_places(
                [(value, ScalarType::Integer(integer))],
                [(root, StructuralPlaceKind::Parameter { position: 0 })],
            )
            .unwrap();
            let field = ScalarTerm::integer_field_path(
                root,
                vec![semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    StructuralFieldId::new(1).unwrap(),
                )],
                integer,
            );
            let literal = ScalarTerm::integer(integer, IntegerValue::Unsigned(10)).unwrap();
            let scalar = ScalarTerm::value(value, ScalarType::Integer(integer));
            let wrapping = ScalarTerm::WrappingIntegerAdd {
                scalar_type: integer,
                left: Box::new(scalar.clone()),
                right: Box::new(literal.clone()),
            };
            for endpoint in [field, wrapping] {
                let finite = Proposition::LessThan(endpoint.clone(), literal.clone());
                let mathematical = lift_fixed_integer_relation(&finite).unwrap();
                assert_eq!(
                    lower_integer_math_relation(&mathematical),
                    Some(finite.clone())
                );
                for (requirement, goal) in [
                    (finite.clone(), mathematical.clone()),
                    (mathematical.clone(), finite),
                ] {
                    let proof = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::Assumption { index: 0 },
                    };
                    check_certificate(&context, &goal, from_ref(&requirement), &[], &proof)
                        .unwrap();
                    crate::verify_bounded_certificate(
                        &context,
                        &goal,
                        &[requirement],
                        &[],
                        &proof,
                        &mut crate::Budget::default(),
                    )
                    .unwrap();
                }
                let wrong = Proposition::IntegerMathLessThan(
                    IntegerMathTerm::embed(scalar.clone()).unwrap(),
                    IntegerMathTerm::literal(IntegerValue::Unsigned(10)),
                );
                let proof = ProofNode {
                    conclusion: wrong.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                };
                assert!(check_certificate(&context, &wrong, &[mathematical], &[], &proof).is_err());
            }
            let other = IntegerType::new(IntegerSign::Unsigned, 16).unwrap();
            let mixed = Proposition::IntegerMathLessThan(
                IntegerMathTerm::embed(scalar).unwrap(),
                IntegerMathTerm::MathValue {
                    source_type: other,
                    value,
                },
            );
            assert!(lower_integer_math_relation(&mixed).is_none());
            let outside = Proposition::IntegerMathLessThan(
                IntegerMathTerm::MathValue {
                    source_type: integer,
                    value,
                },
                IntegerMathTerm::literal(IntegerValue::Unsigned(256)),
            );
            assert!(lower_integer_math_relation(&outside).is_none());

            let mut deep = literal.clone();
            for _ in 0..65 {
                deep = ScalarTerm::WrappingIntegerAdd {
                    scalar_type: integer,
                    left: Box::new(deep),
                    right: Box::new(literal.clone()),
                };
            }
            let oversized = Proposition::LessThan(deep, literal);
            assert!(lift_fixed_integer_relation(&oversized).is_none());
            assert!(lift_exact_integer_relation(&oversized).is_none());
            assert!(
                crate::decide_primitive(
                    &context,
                    &oversized,
                    terminal_psi::PrimitiveJudgment::ClosedIntegerRelation
                )
                .is_err()
            );
        }
    }
}
pub(crate) mod integer_order_rules {
    use super::dispatched_rule;
    use super::integer_math_normalization::{
        lift_exact_integer_order, lower_integer_math_relation,
        propositions_match_under_integer_math_normalization,
    };
    use super::{
        AcceptanceBuilder, AcceptedProofRule, ProofError, mathematical_order, scalar_order,
    };
    use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarType};
    use terminal_psi::{ProofNode, ProofRule};

    pub(super) fn check_integer_subtract_order(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerSubtractOrder { difference, positive } => IntegerSubtractOrder);
        super::subtract_order::check(
            &difference.conclusion,
            &positive.conclusion,
            &proof.conclusion,
        )
    }

    pub(super) fn check_integer_add_order(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerAddOrder { sum, positive } => IntegerAddOrder);
        super::add_order::check(&sum.conclusion, &positive.conclusion, &proof.conclusion)
    }

    pub(super) fn check_integer_subtract_antitone(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerSubtractAntitone { smaller, larger, order } => IntegerSubtractAntitone);
        super::subtract_antitone::check(
            &smaller.conclusion,
            &larger.conclusion,
            &order.conclusion,
            &proof.conclusion,
        )
    }

    pub(super) fn check_integer_order_discreteness(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderDiscreteness { relation } => IntegerOrderDiscreteness);
        super::order_discreteness::check(&relation.conclusion, &proof.conclusion)
    }

    pub(crate) fn order_weakening(
        relation: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let (Proposition::Equal(left, right) | Proposition::LessThan(left, right)) = relation
        else {
            return Err(ProofError::RulePremiseMismatch("integer order weakening"));
        };
        if !matches!(left.scalar_type(), ScalarType::Integer(_))
            || left.scalar_type() != right.scalar_type()
        {
            return Err(ProofError::RulePremiseMismatch("integer order weakening"));
        }
        propositions_match_under_integer_math_normalization(
            &Proposition::LessOrEqual(left.clone(), right.clone()),
            conclusion,
        )
        .then_some(())
        .ok_or(ProofError::IntegerOrderConclusionMismatch)
    }

    pub(super) fn check_integer_order_weakening(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderWeakening { relation } => IntegerOrderWeakening);
        order_weakening(&relation.conclusion, &proof.conclusion)
    }

    pub(crate) fn less_or_equal_transitivity(
        left_less_or_equal_middle: &Proposition,
        middle_less_or_equal_right: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        match (left_less_or_equal_middle, middle_less_or_equal_right) {
            (
                Proposition::LessOrEqual(left, first_middle),
                Proposition::LessOrEqual(second_middle, right),
            ) => {
                if first_middle != second_middle {
                    return Err(ProofError::IntegerOrderMiddleMismatch);
                }
                let composed = Proposition::LessOrEqual(left.clone(), right.clone());
                if !propositions_match_under_integer_math_normalization(&composed, conclusion) {
                    return Err(ProofError::IntegerOrderConclusionMismatch);
                }
                Ok(())
            }
            (
                Proposition::IntegerMathLessOrEqual(left, first_middle),
                Proposition::IntegerMathLessOrEqual(second_middle, right),
            ) => {
                if first_middle != second_middle {
                    return Err(ProofError::IntegerOrderMiddleMismatch);
                }
                (conclusion == &Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()))
                    .then_some(())
                    .ok_or(ProofError::IntegerOrderConclusionMismatch)
            }
            _ => Err(ProofError::RulePremiseMismatch("integer <= transitivity")),
        }
    }

    pub(super) fn check_integer_less_or_equal_transitivity(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerLessOrEqualTransitivity { left_less_or_equal_middle, middle_less_or_equal_right } => IntegerLessOrEqualTransitivity);
        less_or_equal_transitivity(
            &left_less_or_equal_middle.conclusion,
            &middle_less_or_equal_right.conclusion,
            &proof.conclusion,
        )
    }

    pub(super) fn check_integer_strict_order_transitivity(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerStrictOrderTransitivity { left_to_middle, middle_to_right } => IntegerStrictOrderTransitivity);
        super::strict_order_transitivity::check(
            &left_to_middle.conclusion,
            &middle_to_right.conclusion,
            &proof.conclusion,
        )
    }

    pub(super) fn check_integer_order_substitution(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderSubstitution { relation, equality, endpoint } => IntegerOrderSubstitution);
        endpoint_substitution(
            &relation.conclusion,
            &equality.conclusion,
            *endpoint,
            &proof.conclusion,
        )
    }

    pub(crate) fn endpoint_substitution(
        relation: &Proposition,
        equality: &Proposition,
        endpoint: usize,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        fn check_endpoints<T: PartialEq>(
            relation: (bool, &T, &T),
            equality: (&T, &T),
            endpoint: usize,
            conclusion: (bool, &T, &T),
        ) -> Result<(), ProofError> {
            let (relation_strict, relation_left, relation_right) = relation;
            let (conclusion_strict, conclusion_left, conclusion_right) = conclusion;
            if relation_strict != conclusion_strict {
                return Err(ProofError::IntegerOrderConclusionMismatch);
            }
            let (old_endpoint, new_endpoint) = match endpoint {
                0 => {
                    if relation_right != conclusion_right {
                        return Err(ProofError::IntegerOrderUnchangedEndpointMismatch);
                    }
                    (relation_left, conclusion_left)
                }
                1 => {
                    if relation_left != conclusion_left {
                        return Err(ProofError::IntegerOrderUnchangedEndpointMismatch);
                    }
                    (relation_right, conclusion_right)
                }
                endpoint => return Err(ProofError::UnknownIntegerOrderEndpoint(endpoint)),
            };
            ((equality.0 == old_endpoint && equality.1 == new_endpoint)
                || (equality.1 == old_endpoint && equality.0 == new_endpoint))
                .then_some(())
                .ok_or(ProofError::IntegerOrderSubstitutionMismatch)
        }

        if let Some(relation) = mathematical_order(relation) {
            let Proposition::IntegerMathEqual(left, right) = equality else {
                return Err(ProofError::RulePremiseMismatch(
                    "integer order substitution equality",
                ));
            };
            let lifted = lift_exact_integer_order(conclusion);
            let conclusion = mathematical_order(lifted.as_ref().unwrap_or(conclusion)).ok_or(
                ProofError::RuleConclusionMismatch("integer order substitution"),
            )?;
            return check_endpoints(relation, (left, right), endpoint, conclusion);
        }
        let relation = scalar_order(relation).ok_or(ProofError::RulePremiseMismatch(
            "integer order substitution relation",
        ))?;
        let Proposition::Equal(left, right) = equality else {
            return Err(ProofError::RulePremiseMismatch(
                "integer order substitution equality",
            ));
        };
        let normalized_conclusion =
            lower_integer_math_relation(conclusion).unwrap_or_else(|| conclusion.clone());
        let conclusion = scalar_order(&normalized_conclusion).ok_or(
            ProofError::RuleConclusionMismatch("integer order substitution"),
        )?;
        check_endpoints(relation, (left, right), endpoint, conclusion)
    }

    pub(crate) fn math_order_translation(
        relation: &Proposition,
        addend: &IntegerMathTerm,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let Proposition::IntegerMathLessOrEqual(left, right) = relation else {
            return Err(ProofError::RulePremiseMismatch(
                "mathematical integer order translation",
            ));
        };
        let Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::Add(conclusion_left, left_addend),
            IntegerMathTerm::Add(conclusion_right, right_addend),
        ) = conclusion
        else {
            return Err(ProofError::IntegerOrderConclusionMismatch);
        };
        (conclusion_left.as_ref() == left
            && conclusion_right.as_ref() == right
            && left_addend.as_ref() == addend
            && right_addend.as_ref() == addend)
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
    }

    pub(super) fn check_integer_math_order_translation(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerMathOrderTranslation { relation, addend } => IntegerMathOrderTranslation);
        math_order_translation(&relation.conclusion, addend, &proof.conclusion)?;
        Ok(())
    }

    pub(crate) fn order_antisymmetry(
        forward: &Proposition,
        backward: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let Proposition::LessOrEqual(left, right) = forward else {
            return Err(ProofError::RulePremiseMismatch(
                "integer order antisymmetry",
            ));
        };
        let Some(carrier) = left.scalar_type().as_integer() else {
            return Err(ProofError::RulePremiseMismatch(
                "integer order antisymmetry carrier",
            ));
        };
        if carrier.is_address() || right.scalar_type() != ScalarType::Integer(carrier) {
            return Err(ProofError::RulePremiseMismatch(
                "integer order antisymmetry carrier",
            ));
        }
        let Proposition::LessOrEqual(backward_left, backward_right) = backward else {
            return Err(ProofError::RulePremiseMismatch(
                "integer order antisymmetry",
            ));
        };
        if backward_left != right || backward_right != left {
            return Err(ProofError::IntegerOrderMiddleMismatch);
        }
        match conclusion {
            Proposition::Equal(actual_left, actual_right)
                if actual_left == left && actual_right == right =>
            {
                Ok(())
            }
            _ => Err(ProofError::IntegerOrderConclusionMismatch),
        }
    }

    pub(super) fn check_integer_order_antisymmetry(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::IntegerOrderAntisymmetry { forward, backward } => IntegerOrderAntisymmetry);
        order_antisymmetry(&forward.conclusion, &backward.conclusion, &proof.conclusion)?;
        Ok(())
    }
}
pub(crate) mod order_discreteness {
    use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm};

    use super::ProofError;

    pub(crate) fn check(
        relation: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        match (relation, conclusion) {
            (
                Proposition::LessOrEqual(left, right),
                Proposition::LessThan(strict_left, strict_right),
            ) => {
                same_carrier(left, right)?;
                ((right == strict_right && adjacent(left, false).as_ref() == Some(strict_left))
                    || (left == strict_left
                        && adjacent(right, true).as_ref() == Some(strict_right)))
                .then_some(())
                .ok_or(ProofError::IntegerOrderConclusionMismatch)
            }
            (
                Proposition::LessThan(left, right),
                Proposition::LessOrEqual(bound_left, bound_right),
            ) => {
                same_carrier(left, right)?;
                ((right == bound_right && adjacent(left, true).as_ref() == Some(bound_left))
                    || (left == bound_left && adjacent(right, false).as_ref() == Some(bound_right)))
                .then_some(())
                .ok_or(ProofError::IntegerOrderConclusionMismatch)
            }
            (Proposition::LessOrEqual(_, _) | Proposition::LessThan(_, _), _) => {
                Err(ProofError::IntegerOrderConclusionMismatch)
            }
            _ => Err(ProofError::RulePremiseMismatch(
                "integer order discreteness",
            )),
        }
    }

    fn same_carrier(left: &ScalarTerm, right: &ScalarTerm) -> Result<(), ProofError> {
        (left.scalar_type() == right.scalar_type())
            .then_some(())
            .ok_or(ProofError::RulePremiseMismatch(
                "integer order discreteness",
            ))
    }

    fn adjacent(literal: &ScalarTerm, increasing: bool) -> Option<ScalarTerm> {
        let (integer_type, value) = literal.integer_value()?;
        if integer_type.carrier() != IntegerCarrier::Fixed || integer_type.is_address() {
            return None;
        }
        let adjacent = match (value, increasing) {
            (IntegerValue::Signed(value), true) => IntegerValue::Signed(value.checked_add(1)?),
            (IntegerValue::Signed(value), false) => IntegerValue::Signed(value.checked_sub(1)?),
            (IntegerValue::Unsigned(value), true) => IntegerValue::Unsigned(value.checked_add(1)?),
            (IntegerValue::Unsigned(value), false) => IntegerValue::Unsigned(value.checked_sub(1)?),
        };
        ScalarTerm::integer(integer_type, adjacent).ok()
    }

    #[cfg(test)]
    mod tests {
        use super::{IntegerValue, Proposition, ScalarTerm, adjacent};
        use crate::proof::semantic_axiom_proof;
        use crate::{ProofNode, ProofRule, check_certificate};
        use semantic_vocabulary::{
            IntegerSign, IntegerType, PropositionContext, ScalarType, ValueId,
        };
        use std::slice::from_ref;

        #[test]
        fn a_strict_bound_moves_one_literal_endpoint_inward() {
            for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                let integer_type = IntegerType::new(sign, 8).unwrap();
                let scalar_type = ScalarType::Integer(integer_type);
                let identity = ValueId::new(1).unwrap();
                let value = ScalarTerm::value(identity, scalar_type);
                let context =
                    PropositionContext::from_value_types([(identity, scalar_type)]).unwrap();
                let literal = |number| {
                    ScalarTerm::integer(integer_type, IntegerValue::for_sign(sign, number).unwrap())
                        .unwrap()
                };
                for lower in [false, true] {
                    let premise = if lower {
                        Proposition::LessThan(literal(0), value.clone())
                    } else {
                        Proposition::LessThan(value.clone(), literal(2))
                    };
                    let goal = if lower {
                        Proposition::LessOrEqual(literal(1), value.clone())
                    } else {
                        Proposition::LessOrEqual(value.clone(), literal(1))
                    };
                    let proof = ProofNode {
                        conclusion: goal.clone(),
                        rule: ProofRule::IntegerOrderDiscreteness {
                            relation: Box::new(semantic_axiom_proof(premise.clone(), 0)),
                        },
                    };
                    check_certificate(&context, &goal, &[], from_ref(&premise), &proof).unwrap();
                    assert!(check_certificate(&context, &goal, &[], &[], &proof).is_err());
                    for wrong in [
                        Proposition::LessOrEqual(literal(2), value.clone()),
                        Proposition::LessOrEqual(value.clone(), literal(0)),
                        Proposition::LessOrEqual(literal(0), value.clone()),
                        Proposition::LessOrEqual(value.clone(), literal(2)),
                        premise.clone(),
                    ] {
                        let mut altered = proof.clone();
                        altered.conclusion = wrong.clone();
                        assert!(
                            check_certificate(&context, &wrong, &[], from_ref(&premise), &altered)
                                .is_err(),
                            "{wrong:?}"
                        );
                    }
                }
            }
        }

        #[test]
        fn adjacency_never_wraps_at_carrier_or_host_limits() {
            for bits in [8, 128] {
                for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
                    let integer_type = IntegerType::new(sign, bits).unwrap();
                    let minimum =
                        ScalarTerm::integer(integer_type, integer_type.minimum_value()).unwrap();
                    let maximum =
                        ScalarTerm::integer(integer_type, integer_type.maximum_value()).unwrap();
                    assert!(adjacent(&minimum, false).is_none());
                    assert!(adjacent(&maximum, true).is_none());
                    assert!(adjacent(&minimum, true).is_some());
                    assert!(adjacent(&maximum, false).is_some());
                }
            }
            let address =
                ScalarTerm::integer(IntegerType::address(64).unwrap(), IntegerValue::Unsigned(1))
                    .unwrap();
            assert!(adjacent(&address, false).is_none());
            assert!(adjacent(&address, true).is_none());
        }
    }
}
mod propositional_rules {
    use super::dispatched_rule;
    use super::integer_math_normalization::propositions_match_under_integer_math_normalization;
    use super::{AcceptanceBuilder, AcceptedProofRule, ProofError, RuleScope};
    use crate::decide_primitive;
    use semantic_vocabulary::Proposition;
    use terminal_psi::{ProofNode, ProofRule};

    pub(super) fn check_primitive(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::Primitive(judgment) => Primitive);
        let RuleScope { context, .. } = *scope;
        decide_primitive(context, &proof.conclusion, *judgment)
            .map_err(ProofError::PrimitiveJudgment)
    }

    pub(super) fn check_semantic_axiom(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::SemanticAxiom { index } => SemanticAxiom);
        let RuleScope {
            semantic_axioms, ..
        } = *scope;
        let axiom = semantic_axioms
            .get(*index)
            .ok_or(ProofError::UnknownSemanticAxiom(*index))?;
        if !propositions_match_under_integer_math_normalization(axiom, &proof.conclusion) {
            return Err(ProofError::SemanticAxiomConclusionMismatch(*index));
        }
        acceptance.record_semantic_axiom(*index, axiom);
        Ok(())
    }

    pub(super) fn check_assumption(
        scope: &RuleScope<'_>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::Assumption { index } => Assumption);
        let RuleScope { assumptions, .. } = *scope;
        let assumption = assumptions
            .get(*index)
            .ok_or(ProofError::UnknownAssumption(*index))?;
        if !propositions_match_under_integer_math_normalization(assumption, &proof.conclusion) {
            return Err(ProofError::AssumptionConclusionMismatch(*index));
        }
        acceptance.record_assumption(*index, assumption);
        Ok(())
    }

    pub(super) fn check_conjunction_introduction(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::ConjunctionIntroduction(conjuncts) => ConjunctionIntroduction);
        let Proposition::Conjunction(expected) = &proof.conclusion else {
            return Err(ProofError::RuleConclusionMismatch(
                "conjunction introduction",
            ));
        };
        if expected.len() != conjuncts.len() {
            return Err(ProofError::ConjunctionArityMismatch);
        }
        for (expected, conjunct) in expected.iter().zip(conjuncts) {
            if &conjunct.conclusion != expected {
                return Err(ProofError::ConjunctConclusionMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn check_conjunction_elimination(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::ConjunctionElimination { conjunction, conjunct } => ConjunctionElimination);
        let Proposition::Conjunction(conjuncts) = &conjunction.conclusion else {
            return Err(ProofError::RulePremiseMismatch("conjunction elimination"));
        };
        let selected = conjuncts
            .get(*conjunct)
            .ok_or(ProofError::UnknownConjunct(*conjunct))?;
        (selected == &proof.conclusion)
            .then_some(())
            .ok_or(ProofError::ConjunctConclusionMismatch)
    }

    pub(super) fn check_disjunction_introduction(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::DisjunctionIntroduction { disjunct, index } => DisjunctionIntroduction);
        let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
            return Err(ProofError::RuleConclusionMismatch(
                "disjunction introduction",
            ));
        };
        let selected = disjuncts
            .get(*index)
            .ok_or(ProofError::UnknownDisjunct(*index))?;
        (selected == &disjunct.conclusion)
            .then_some(())
            .ok_or(ProofError::DisjunctConclusionMismatch)
    }

    pub(super) fn check_disjunction_elimination(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::DisjunctionElimination { disjunction, branches } => DisjunctionElimination);
        let Proposition::Disjunction(disjuncts) = &disjunction.conclusion else {
            return Err(ProofError::RulePremiseMismatch("disjunction elimination"));
        };
        if branches.len() != disjuncts.len() {
            return Err(ProofError::DisjunctionArityMismatch);
        }
        for branch in branches {
            if branch.conclusion != proof.conclusion {
                return Err(ProofError::DisjunctionBranchConclusionMismatch);
            }
        }
        Ok(())
    }

    pub(super) fn check_implication_introduction(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::ImplicationIntroduction { body } => ImplicationIntroduction);
        let Proposition::Implication { conclusion, .. } = &proof.conclusion else {
            return Err(ProofError::RuleConclusionMismatch(
                "implication introduction",
            ));
        };
        (&body.conclusion == conclusion.as_ref())
            .then_some(())
            .ok_or(ProofError::ImplicationConclusionMismatch)
    }

    pub(super) fn check_implication_elimination(
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        dispatched_rule!(proof, acceptance, ProofRule::ImplicationElimination { implication, premise } => ImplicationElimination);
        let Proposition::Implication {
            premise: required,
            conclusion,
        } = &implication.conclusion
        else {
            return Err(ProofError::RulePremiseMismatch("implication elimination"));
        };
        if premise.conclusion != **required {
            return Err(ProofError::ImplicationPremiseMismatch);
        }
        (&proof.conclusion == conclusion.as_ref())
            .then_some(())
            .ok_or(ProofError::ImplicationConclusionMismatch)
    }
}
pub(crate) mod strict_order_transitivity {
    use semantic_vocabulary::{Proposition, ScalarType};

    use super::{ProofError, mathematical_order, scalar_order};

    pub(crate) fn check(
        left_to_middle: &Proposition,
        middle_to_right: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        if let (Some(first), Some(second), Proposition::LessThan(left, right)) = (
            scalar_order(left_to_middle),
            scalar_order(middle_to_right),
            conclusion,
        ) {
            let scalar_type = first.1.scalar_type();
            if !matches!(scalar_type, ScalarType::Integer(_))
                || [first.2, second.1, second.2, left, right]
                    .iter()
                    .any(|term| term.scalar_type() != scalar_type)
            {
                return Err(ProofError::RulePremiseMismatch(
                    "strict integer order carrier",
                ));
            }
            return endpoints(first, second, left, right);
        }
        if let (Some(first), Some(second), Proposition::IntegerMathLessThan(left, right)) = (
            mathematical_order(left_to_middle),
            mathematical_order(middle_to_right),
            conclusion,
        ) {
            return endpoints(first, second, left, right);
        }
        Err(ProofError::RulePremiseMismatch(
            "strict integer order transitivity",
        ))
    }

    fn endpoints<Term: Eq>(
        first: (bool, &Term, &Term),
        second: (bool, &Term, &Term),
        left: &Term,
        right: &Term,
    ) -> Result<(), ProofError> {
        if !first.0 && !second.0 {
            return Err(ProofError::RulePremiseMismatch("strict integer order edge"));
        }
        if first.2 != second.1 {
            return Err(ProofError::IntegerOrderMiddleMismatch);
        }
        (first.1 == left && second.2 == right)
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
    }

    #[cfg(test)]
    mod tests {
        use crate::proof::{assumption_proof, semantic_axiom_proof};
        use crate::{
            ProofNode, ProofRule, check_certificate, lift_fixed_integer_relation, value_context,
        };
        use semantic_vocabulary::{
            IntegerSign, IntegerType, Proposition, PropositionContext, ScalarTerm, ScalarType,
            ValueId,
        };

        fn fixture(
            strict: [bool; 2],
            mathematical: bool,
        ) -> (PropositionContext, [Proposition; 2], ProofNode) {
            let scalar_type =
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
            let context = value_context(scalar_type, 4);
            let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
            let order = |left, right, strict| {
                let proposition = if strict {
                    Proposition::LessThan(value(left), value(right))
                } else {
                    Proposition::LessOrEqual(value(left), value(right))
                };
                if mathematical {
                    lift_fixed_integer_relation(&proposition).unwrap()
                } else {
                    proposition
                }
            };
            let premises = [order(1, 2, strict[0]), order(2, 3, strict[1])];
            let proof = ProofNode {
                conclusion: order(1, 3, true),
                rule: ProofRule::IntegerStrictOrderTransitivity {
                    left_to_middle: Box::new(assumption_proof(premises[0].clone(), 0)),
                    middle_to_right: Box::new(semantic_axiom_proof(premises[1].clone(), 0)),
                },
            };
            (context, premises, proof)
        }

        #[test]
        fn nonstrict_chains_and_endpoint_or_domain_drift_reject() {
            for mathematical in [false, true] {
                let (context, premises, proof) = fixture([false, false], mathematical);
                assert!(
                    check_certificate(
                        &context,
                        &proof.conclusion,
                        &premises[..1],
                        &premises[1..],
                        &proof
                    )
                    .is_err()
                );
                for mutation in [
                    "middle",
                    "left",
                    "right",
                    "conclusion kind",
                    "mixed domain",
                    "false child",
                ] {
                    let (context, mut premises, mut proof) = fixture([true, false], mathematical);
                    let (_, _, other) = fixture([true, false], !mathematical);
                    let ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle,
                        middle_to_right,
                    } = &mut proof.rule
                    else {
                        unreachable!()
                    };
                    match mutation {
                        "middle" => {
                            premises[1] = premises[0].clone();
                            middle_to_right.conclusion = premises[1].clone();
                        }
                        "left" => proof.conclusion = premises[1].clone(),
                        "right" => proof.conclusion = premises[0].clone(),
                        "conclusion kind" => {
                            proof.conclusion = match proof.conclusion {
                                Proposition::LessThan(left, right) => {
                                    Proposition::LessOrEqual(left, right)
                                }
                                Proposition::IntegerMathLessThan(left, right) => {
                                    Proposition::IntegerMathLessOrEqual(left, right)
                                }
                                _ => unreachable!(),
                            };
                        }
                        "mixed domain" => proof.conclusion = other.conclusion,
                        "false child" => {
                            left_to_middle.rule = ProofRule::SemanticAxiom { index: 0 }
                        }
                        _ => unreachable!(),
                    }
                    assert!(
                        check_certificate(
                            &context,
                            &proof.conclusion,
                            &premises[..1],
                            &premises[1..],
                            &proof
                        )
                        .is_err(),
                        "{mutation}"
                    );
                }
            }
        }

        #[test]
        fn boolean_relations_and_mismatched_integer_carriers_reject() {
            let (context, premises, proof) = fixture([true, false], false);
            let wrong = PropositionContext::from_value_types((1..=4).map(|identity| {
                (
                    ValueId::new(identity).unwrap(),
                    if identity == 2 {
                        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
                    } else {
                        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
                    },
                )
            }))
            .unwrap();
            assert!(
                check_certificate(
                    &wrong,
                    &proof.conclusion,
                    &premises[..1],
                    &premises[1..],
                    &proof
                )
                .is_err()
            );
            let first =
                Proposition::LessThan(ScalarTerm::Boolean(false), ScalarTerm::Boolean(true));
            let second =
                Proposition::LessOrEqual(ScalarTerm::Boolean(true), ScalarTerm::Boolean(true));
            assert!(super::check(&first, &second, &first).is_err());
            check_certificate(
                &context,
                &proof.conclusion,
                &premises[..1],
                &premises[1..],
                &proof,
            )
            .unwrap();
        }

        #[test]
        fn order_composition_preserves_scalar_carriers_without_arithmetic_conversion() {
            for integer in [
                IntegerType::new(IntegerSign::Signed, 8).unwrap(),
                IntegerType::new(IntegerSign::Unsigned, 128).unwrap(),
                IntegerType::address(64).unwrap(),
            ] {
                let scalar_type = ScalarType::Integer(integer);
                let context = value_context(scalar_type, 3);
                let value =
                    |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
                let premises = [
                    Proposition::LessThan(value(1), value(2)),
                    Proposition::LessOrEqual(value(2), value(3)),
                ];
                let proof = ProofNode {
                    conclusion: Proposition::LessThan(value(1), value(3)),
                    rule: ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle: Box::new(assumption_proof(premises[0].clone(), 0)),
                        middle_to_right: Box::new(assumption_proof(premises[1].clone(), 1)),
                    },
                };
                check_certificate(&context, &proof.conclusion, &premises, &[], &proof).unwrap();
            }
        }
    }
}
pub(crate) mod subtract_antitone {
    use semantic_vocabulary::{IntegerCarrier, Proposition, ScalarTerm, ScalarType};

    use super::ProofError;

    pub(crate) fn check(
        smaller: &Proposition,
        larger: &Proposition,
        order: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let difference = |proposition: &Proposition| match proposition {
            Proposition::Equal(
                result,
                ScalarTerm::ExactIntegerSubtract {
                    scalar_type,
                    left,
                    right,
                },
            ) => Some((
                result.clone(),
                *scalar_type,
                left.as_ref().clone(),
                right.as_ref().clone(),
            )),
            _ => None,
        };
        let (
            Some((smaller_result, scalar_type, minuend, large)),
            Some((larger_result, larger_type, larger_minuend, small)),
        ) = (difference(smaller), difference(larger))
        else {
            return Err(ProofError::RulePremiseMismatch(
                "integer subtraction antitonicity",
            ));
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed
            || scalar_type.is_address()
            || larger_type != scalar_type
            || larger_minuend != minuend
            || smaller_result.scalar_type() != ScalarType::Integer(scalar_type)
            || larger_result.scalar_type() != ScalarType::Integer(scalar_type)
            || order != &Proposition::LessThan(small, large)
        {
            return Err(ProofError::RulePremiseMismatch(
                "integer subtraction antitonicity",
            ));
        }
        (conclusion == &Proposition::LessThan(smaller_result, larger_result))
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
    }
}
pub(crate) mod subtract_order {
    use semantic_vocabulary::{IntegerCarrier, IntegerValue, Proposition, ScalarTerm, ScalarType};

    use super::ProofError;

    pub(crate) fn check(
        difference: &Proposition,
        positive: &Proposition,
        conclusion: &Proposition,
    ) -> Result<(), ProofError> {
        let Proposition::Equal(
            result,
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            },
        ) = difference
        else {
            return Err(ProofError::RulePremiseMismatch("integer subtraction order"));
        };
        let Proposition::LessThan(zero, decrement) = positive else {
            return Err(ProofError::RulePremiseMismatch(
                "integer subtraction positivity",
            ));
        };
        if scalar_type.carrier() != IntegerCarrier::Fixed
            || scalar_type.is_address()
            || result.scalar_type() != ScalarType::Integer(*scalar_type)
            || zero.scalar_type() != ScalarType::Integer(*scalar_type)
            || decrement != right.as_ref()
            || !matches!(
                zero.integer_value(),
                Some((_, IntegerValue::Signed(0) | IntegerValue::Unsigned(0)))
            )
        {
            return Err(ProofError::RulePremiseMismatch("integer subtraction order"));
        }
        (conclusion == &Proposition::LessThan(result.clone(), left.as_ref().clone()))
            .then_some(())
            .ok_or(ProofError::IntegerOrderConclusionMismatch)
    }
}
mod traversal {
    use super::{
        AcceptanceBuilder, BTreeSet, ProofError, ProofNode, ProofRule, Proposition,
        PropositionContext, ValueId, check_node_locally,
    };
    enum Action<'proof> {
        Enter(&'proof ProofNode),
        Post(&'proof ProofNode),
        Children(&'proof [ProofNode]),
        Cases(&'proof ProofNode, &'proof [ProofNode]),
        Branches(&'proof [Proposition], &'proof [ProofNode]),
        PushAssumption(&'proof Proposition),
        PopAssumption,
    }

    pub(super) fn check_node(
        context: &PropositionContext,
        assumptions: &[Proposition],
        semantic_axioms: &[Proposition],
        machine_parameter_values: &BTreeSet<ValueId>,
        proof: &ProofNode,
        acceptance: &mut AcceptanceBuilder,
    ) -> Result<(), ProofError> {
        let ambient_count = assumptions.len();
        let mut assumptions = assumptions.to_vec();
        let mut pending = vec![Action::Enter(proof)];
        while let Some(action) = pending.pop() {
            match action {
                Action::Enter(proof) => {
                    context
                        .validate(&proof.conclusion)
                        .map_err(ProofError::MalformedProposition)?;
                    pending.push(Action::Post(proof));
                    schedule_children(proof, &mut pending)?;
                }
                Action::Post(proof) => {
                    check_node_locally(
                        context,
                        &assumptions,
                        semantic_axioms,
                        machine_parameter_values,
                        proof,
                        acceptance,
                    )?;
                }
                Action::Children(children) => {
                    if let Some((first, remaining)) = children.split_first() {
                        pending.push(Action::Children(remaining));
                        pending.push(Action::Enter(first));
                    }
                }
                Action::Cases(disjunction, branches) => {
                    let Proposition::Disjunction(disjuncts) = &disjunction.conclusion else {
                        return Err(ProofError::RulePremiseMismatch("disjunction elimination"));
                    };
                    if branches.len() != disjuncts.len() {
                        return Err(ProofError::DisjunctionArityMismatch);
                    }
                    pending.push(Action::Branches(disjuncts, branches));
                }
                Action::Branches(disjuncts, branches) => {
                    if let Some((first, remaining)) = branches.split_first() {
                        let (disjunct, remaining_disjuncts) =
                            disjuncts.split_first().expect("checked case arity");
                        pending.push(Action::Branches(remaining_disjuncts, remaining));
                        pending.push(Action::PopAssumption);
                        pending.push(Action::Enter(first));
                        pending.push(Action::PushAssumption(disjunct));
                    }
                }
                Action::PushAssumption(proposition) => assumptions.push(proposition.clone()),
                Action::PopAssumption => {
                    debug_assert!(assumptions.len() > ambient_count);
                    assumptions.pop().expect("scheduled local assumption");
                }
            }
        }
        debug_assert_eq!(assumptions.len(), ambient_count);
        Ok(())
    }

    fn schedule_children<'proof>(
        proof: &'proof ProofNode,
        pending: &mut Vec<Action<'proof>>,
    ) -> Result<(), ProofError> {
        match &proof.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. }
            | ProofRule::IntegerCorrelatedForbiddenRoots { .. } => {}
            ProofRule::ConjunctionIntroduction(children) => {
                let Proposition::Conjunction(expected) = &proof.conclusion else {
                    return Err(ProofError::RuleConclusionMismatch(
                        "conjunction introduction",
                    ));
                };
                if children.len() != expected.len() {
                    return Err(ProofError::ConjunctionArityMismatch);
                }
                pending.push(Action::Children(children));
            }
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                pending.push(Action::Enter(conjunction));
            }
            ProofRule::DisjunctionIntroduction { disjunct, index } => {
                let Proposition::Disjunction(disjuncts) = &proof.conclusion else {
                    return Err(ProofError::RuleConclusionMismatch(
                        "disjunction introduction",
                    ));
                };
                disjuncts
                    .get(*index)
                    .ok_or(ProofError::UnknownDisjunct(*index))?;
                pending.push(Action::Enter(disjunct));
            }
            ProofRule::DisjunctionElimination {
                disjunction,
                branches,
            } => {
                pending.push(Action::Cases(disjunction, branches));
                pending.push(Action::Enter(disjunction));
            }
            ProofRule::ImplicationIntroduction { body } => {
                let Proposition::Implication { premise, .. } = &proof.conclusion else {
                    return Err(ProofError::RuleConclusionMismatch(
                        "implication introduction",
                    ));
                };
                pending.push(Action::PopAssumption);
                pending.push(Action::Enter(body));
                pending.push(Action::PushAssumption(premise));
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => {
                pending.push(Action::Enter(premise));
                pending.push(Action::Enter(implication));
            }
            ProofRule::EqualitySymmetry { equality } => {
                pending.push(Action::Enter(equality));
            }
            ProofRule::PredicateDenotation { premise } => {
                pending.push(Action::Enter(premise));
            }
            ProofRule::ValueEqualityTransport {
                premise,
                equalities,
            } => {
                pending.push(Action::Children(equalities));
                pending.push(Action::Enter(premise));
            }
            ProofRule::IntegerOrderAntisymmetry { forward, backward } => {
                pending.push(Action::Enter(backward));
                pending.push(Action::Enter(forward));
            }
            ProofRule::IntegerSubtractOrder {
                difference,
                positive,
            } => {
                pending.push(Action::Enter(positive));
                pending.push(Action::Enter(difference));
            }
            ProofRule::IntegerAddOrder { sum, positive } => {
                pending.push(Action::Enter(positive));
                pending.push(Action::Enter(sum));
            }
            ProofRule::IntegerSubtractAntitone {
                smaller,
                larger,
                order,
            } => {
                pending.push(Action::Enter(order));
                pending.push(Action::Enter(larger));
                pending.push(Action::Enter(smaller));
            }
            ProofRule::IntegerMathOrderTranslation { relation, .. }
            | ProofRule::IntegerOrderWeakening { relation }
            | ProofRule::IntegerOrderDiscreteness { relation } => {
                pending.push(Action::Enter(relation));
            }
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                pending.push(Action::Enter(middle_equals_right));
                pending.push(Action::Enter(left_equals_middle));
            }
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle,
                middle_less_or_equal_right,
            } => {
                pending.push(Action::Enter(middle_less_or_equal_right));
                pending.push(Action::Enter(left_less_or_equal_middle));
            }
            ProofRule::IntegerStrictOrderTransitivity {
                left_to_middle,
                middle_to_right,
            } => {
                pending.push(Action::Enter(middle_to_right));
                pending.push(Action::Enter(left_to_middle));
            }
            ProofRule::IntegerOrderSubstitution {
                relation, equality, ..
            } => {
                pending.push(Action::Enter(equality));
                pending.push(Action::Enter(relation));
            }
            ProofRule::IntegerAffineBound { root_bound, .. }
            | ProofRule::IntegerCastBound { root_bound, .. } => {
                pending.push(Action::Enter(root_bound));
            }
            ProofRule::IntegerExactAddDefinitionBound {
                left_bound,
                right_bound,
                ..
            }
            | ProofRule::IntegerExactSubtractDefinitionBound {
                left_bound,
                right_bound,
                ..
            } => {
                pending.push(Action::Enter(right_bound));
                pending.push(Action::Enter(left_bound));
            }
        }
        Ok(())
    }
}

pub use integer_math_normalization::{
    lift_exact_integer_order, lift_fixed_integer_relation, lower_integer_math_relation,
};
use traversal::check_node;

macro_rules! dispatched_rule {
    ($proof:expr, $acceptance:expr, $pattern:pat => $accepted:ident) => {
        let $pattern = &$proof.rule else {
            unreachable!(concat!("dispatched ", stringify!($accepted)))
        };
        $acceptance.rules.insert(AcceptedProofRule::$accepted);
    };
}
pub(crate) use dispatched_rule;

#[derive(Clone, Copy)]
struct RuleScope<'a> {
    context: &'a PropositionContext,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    machine_parameter_values: &'a BTreeSet<ValueId>,
}

fn check_node_locally(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    let scope = RuleScope {
        context,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    };
    match &proof.rule {
        ProofRule::Primitive(_) => propositional_rules::check_primitive(&scope, proof, acceptance),
        ProofRule::SemanticAxiom { .. } => check_semantic_axiom(&scope, proof, acceptance),
        ProofRule::Assumption { .. } => check_assumption(&scope, proof, acceptance),
        ProofRule::ConjunctionIntroduction(_) => check_conjunction_introduction(proof, acceptance),
        ProofRule::ConjunctionElimination { .. } => {
            propositional_rules::check_conjunction_elimination(proof, acceptance)
        }
        ProofRule::DisjunctionIntroduction { .. } => {
            propositional_rules::check_disjunction_introduction(proof, acceptance)
        }
        ProofRule::DisjunctionElimination { .. } => {
            propositional_rules::check_disjunction_elimination(proof, acceptance)
        }
        ProofRule::ImplicationIntroduction { .. } => {
            propositional_rules::check_implication_introduction(proof, acceptance)
        }
        ProofRule::ImplicationElimination { .. } => {
            propositional_rules::check_implication_elimination(proof, acceptance)
        }
        ProofRule::EqualitySymmetry { .. } => check_equality_symmetry(proof, acceptance),
        ProofRule::PredicateDenotation { .. } => {
            equality_rules::check_predicate_denotation(&scope, proof, acceptance)
        }
        ProofRule::ValueEqualityTransport { .. } => {
            equality_rules::check_value_equality_transport(&scope, proof, acceptance)
        }
        ProofRule::EqualityTransitivity { .. } => check_equality_transitivity(proof, acceptance),
        ProofRule::IntegerSubtractOrder { .. } => check_integer_subtract_order(proof, acceptance),
        ProofRule::IntegerAddOrder { .. } => check_integer_add_order(proof, acceptance),
        ProofRule::IntegerSubtractAntitone { .. } => {
            integer_order_rules::check_integer_subtract_antitone(proof, acceptance)
        }
        ProofRule::IntegerOrderDiscreteness { .. } => {
            integer_order_rules::check_integer_order_discreteness(proof, acceptance)
        }
        ProofRule::IntegerOrderWeakening { .. } => check_integer_order_weakening(proof, acceptance),
        ProofRule::IntegerLessOrEqualTransitivity { .. } => {
            integer_order_rules::check_integer_less_or_equal_transitivity(proof, acceptance)
        }
        ProofRule::IntegerStrictOrderTransitivity { .. } => {
            integer_order_rules::check_integer_strict_order_transitivity(proof, acceptance)
        }
        ProofRule::IntegerOrderSubstitution { .. } => {
            integer_order_rules::check_integer_order_substitution(proof, acceptance)
        }
        ProofRule::IntegerOrderAntisymmetry { .. } => {
            integer_order_rules::check_integer_order_antisymmetry(proof, acceptance)
        }
        ProofRule::IntegerMathOrderTranslation { .. } => {
            integer_order_rules::check_integer_math_order_translation(proof, acceptance)
        }
        ProofRule::IntegerAffineBound { .. } | ProofRule::IntegerCastBound { .. } => {
            integer_bound_rules::check_integer_cited_bound(&scope, proof, acceptance)
        }
        ProofRule::IntegerExactAddDefinitionBound { .. }
        | ProofRule::IntegerExactSubtractDefinitionBound { .. } => {
            integer_bound_rules::check_integer_exact_definition_bound(&scope, proof, acceptance)
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { .. } => {
            integer_bound_rules::check_integer_correlated_forbidden_roots(&scope, proof, acceptance)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofError {
    MalformedProposition(semantic_vocabulary::PropositionError),
    PrimitiveJudgment(KernelError),
    UnknownSemanticAxiom(usize),
    SemanticAxiomConclusionMismatch(usize),
    UnknownAssumption(usize),
    AssumptionConclusionMismatch(usize),
    UnknownConjunct(usize),
    UnknownDisjunct(usize),
    ConjunctionArityMismatch,
    ConjunctConclusionMismatch,
    DisjunctConclusionMismatch,
    DisjunctionArityMismatch,
    DisjunctionBranchConclusionMismatch,
    ImplicationPremiseMismatch,
    ImplicationConclusionMismatch,
    EqualityMiddleMismatch,
    EqualityAlgebraMismatch,
    EqualityConclusionMismatch,
    PredicateDenotation(Box<crate::PredicateDenotationError>),
    IntegerOrderMiddleMismatch,
    IntegerOrderConclusionMismatch,
    UnknownIntegerOrderEndpoint(usize),
    IntegerOrderUnchangedEndpointMismatch,
    IntegerOrderSubstitutionMismatch,
    IntegerAffineWitness(IntegerAffineWitnessError),
    IntegerAffineBoundConversion(IntegerAffineBoundConversionError),
    IntegerCastChainWitness(IntegerCastChainWitnessError),
    IntegerCastBoundConversion(IntegerCastBoundConversionError),
    IntegerCorrelatedForbiddenRootDefinitionBoundary,
    IntegerCorrelatedForbiddenRootRequirementBoundary,
    IntegerCorrelatedForbiddenRootWitness(IntegerCorrelatedForbiddenRootWitnessError),
    IntegerCorrelatedForbiddenRootConversion(IntegerCorrelatedForbiddenRootConversionError),
    CertificateConclusionMismatch,
    RuleConclusionMismatch(&'static str),
    RulePremiseMismatch(&'static str),
    MathematicalCore(Box<BoundedDenotationError>),
}

crate::debug_display!(ProofError);

impl std::error::Error for ProofError {}

#[cfg(test)]
pub(crate) fn assumption_proof(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

#[cfg(test)]
pub(crate) fn semantic_axiom_proof(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::SemanticAxiom { index },
    }
}

pub(crate) fn scalar_order(proposition: &Proposition) -> Option<(bool, &ScalarTerm, &ScalarTerm)> {
    match proposition {
        Proposition::LessThan(left, right) => Some((true, left, right)),
        Proposition::LessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

pub(crate) fn mathematical_order(
    proposition: &Proposition,
) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

#[cfg(test)]
mod disjunction_elimination {
    use super::{
        CertificateAcceptance, ProofError, ProofNode, ProofRule, Proposition, PropositionContext,
        accept_certificate,
    };
    use crate::proof::assumption_proof;
    use semantic_vocabulary::PropositionId;

    fn atom(index: u64) -> Proposition {
        Proposition::Atom(PropositionId::new(index).expect("nonzero atom"))
    }

    fn fixture() -> (Vec<Proposition>, ProofNode) {
        let left = atom(1);
        let right = atom(2);
        let goal = atom(3);
        let disjunction = Proposition::Disjunction(vec![left.clone(), right.clone()]);
        let implications: Vec<_> = [left.clone(), right.clone()]
            .into_iter()
            .map(|premise| Proposition::Implication {
                premise: Box::new(premise),
                conclusion: Box::new(goal.clone()),
            })
            .collect();
        let branches = [left, right]
            .into_iter()
            .enumerate()
            .map(|(index, premise)| ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::ImplicationElimination {
                    implication: Box::new(assumption_proof(implications[index].clone(), index + 1)),
                    premise: Box::new(assumption_proof(premise, 3)),
                },
            })
            .collect();
        let proof = ProofNode {
            conclusion: goal,
            rule: ProofRule::DisjunctionElimination {
                disjunction: Box::new(assumption_proof(disjunction.clone(), 0)),
                branches,
            },
        };
        (
            vec![
                disjunction,
                implications[0].clone(),
                implications[1].clone(),
            ],
            proof,
        )
    }

    fn check(
        assumptions: &[Proposition],
        proof: &ProofNode,
    ) -> Result<CertificateAcceptance, ProofError> {
        accept_certificate(
            &PropositionContext::default(),
            &proof.conclusion,
            assumptions,
            &[],
            proof,
        )
    }

    #[test]
    fn elimination_requires_every_branch_and_the_common_conclusion() {
        let (assumptions, proof) = fixture();
        for count in [0, 1, 3] {
            let mut malformed = proof.clone();
            let ProofRule::DisjunctionElimination { branches, .. } = &mut malformed.rule else {
                unreachable!()
            };
            branches.resize(count, branches[0].clone());
            assert_eq!(
                check(&assumptions, &malformed),
                Err(ProofError::DisjunctionArityMismatch)
            );
        }
        let mut wrong_conclusion = proof.clone();
        let ProofRule::DisjunctionElimination { branches, .. } = &mut wrong_conclusion.rule else {
            unreachable!()
        };
        branches[0] = assumption_proof(atom(1), 3);
        assert_eq!(
            check(&assumptions, &wrong_conclusion),
            Err(ProofError::DisjunctionBranchConclusionMismatch)
        );

        let mut wrong_premise = proof;
        let ProofRule::DisjunctionElimination { disjunction, .. } = &mut wrong_premise.rule else {
            unreachable!()
        };
        **disjunction = assumption_proof(assumptions[1].clone(), 1);
        assert_eq!(
            check(&assumptions, &wrong_premise),
            Err(ProofError::RulePremiseMismatch("disjunction elimination"))
        );
    }

    #[test]
    fn branches_cannot_reuse_another_case_or_an_unproved_disjunction() {
        let (assumptions, proof) = fixture();
        let mut swapped = proof.clone();
        let ProofRule::DisjunctionElimination { branches, .. } = &mut swapped.rule else {
            unreachable!()
        };
        branches.swap(0, 1);
        assert_eq!(
            check(&assumptions, &swapped),
            Err(ProofError::AssumptionConclusionMismatch(3))
        );

        let mut leaked = proof.clone();
        let ProofRule::DisjunctionElimination { branches, .. } = &mut leaked.rule else {
            unreachable!()
        };
        let ProofRule::ImplicationElimination { premise, .. } = &mut branches[1].rule else {
            unreachable!()
        };
        premise.rule = ProofRule::Assumption { index: 4 };
        assert_eq!(
            check(&assumptions, &leaked),
            Err(ProofError::UnknownAssumption(4))
        );

        let mut unproved = proof;
        let ProofRule::DisjunctionElimination { disjunction, .. } = &mut unproved.rule else {
            unreachable!()
        };
        disjunction.rule = ProofRule::Assumption { index: 3 };
        assert_eq!(
            check(&assumptions, &unproved),
            Err(ProofError::UnknownAssumption(3))
        );
    }
}

#[cfg(test)]
mod order_substitution_tests {
    use super::{
        ProofError, ProofNode, ProofRule, Proposition, PropositionContext, ValueId,
        check_certificate, lift_fixed_integer_relation,
    };
    use crate::PrimitiveJudgment;
    use crate::proof::semantic_axiom_proof;
    use crate::proof_node;
    use crate::value_context;
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType};

    fn fixture(
        strict: bool,
        mathematical: bool,
        reverse: bool,
    ) -> (PropositionContext, Vec<Proposition>, ProofNode) {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).unwrap();
        let scalar_type = ScalarType::Integer(integer_type);
        let context = value_context(scalar_type, 3);
        let value = |identity| ScalarTerm::value(ValueId::new(identity).unwrap(), scalar_type);
        let literal =
            |integer| ScalarTerm::integer(integer_type, IntegerValue::Signed(integer)).unwrap();
        let project = |proposition: Proposition| {
            if mathematical {
                lift_fixed_integer_relation(&proposition).unwrap()
            } else {
                proposition
            }
        };
        let order = |left, right| {
            project(if strict {
                Proposition::LessThan(left, right)
            } else {
                Proposition::LessOrEqual(left, right)
            })
        };
        let mut proof = proof_node!(order(literal(1), literal(2)) => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation));
        let mut axioms = Vec::new();
        for endpoint in 0..2 {
            let scalar = value(endpoint as u64 + 1);
            let constant = literal(endpoint as i128 + 1);
            let equality = project(if reverse {
                Proposition::Equal(constant, scalar.clone())
            } else {
                Proposition::Equal(scalar.clone(), constant)
            });
            axioms.push(equality.clone());
            let conclusion = if endpoint == 0 {
                order(scalar, literal(2))
            } else {
                order(value(1), scalar)
            };
            proof = ProofNode {
                conclusion,
                rule: ProofRule::IntegerOrderSubstitution {
                    relation: Box::new(proof),
                    equality: Box::new(semantic_axiom_proof(equality, endpoint)),
                    endpoint,
                },
            };
        }
        (context, axioms, proof)
    }

    #[test]
    fn substitution_cannot_change_order_strictness_in_either_direction() {
        for strict in [false, true] {
            for mathematical in [false, true] {
                let (context, axioms, mut proof) = fixture(strict, mathematical, false);
                proof.conclusion = match proof.conclusion {
                    Proposition::LessThan(left, right) => Proposition::LessOrEqual(left, right),
                    Proposition::LessOrEqual(left, right) => Proposition::LessThan(left, right),
                    Proposition::IntegerMathLessThan(left, right) => {
                        Proposition::IntegerMathLessOrEqual(left, right)
                    }
                    Proposition::IntegerMathLessOrEqual(left, right) => {
                        Proposition::IntegerMathLessThan(left, right)
                    }
                    _ => unreachable!(),
                };
                assert_eq!(
                    check_certificate(&context, &proof.conclusion, &[], &axioms, &proof),
                    Err(ProofError::IntegerOrderConclusionMismatch)
                );
            }
        }
    }

    #[test]
    fn substitution_rejects_endpoint_equality_and_carrier_drift() {
        let (context, axioms, original) = fixture(true, false, false);
        for mutation in 0..4 {
            let mut proof = original.clone();
            let ProofRule::IntegerOrderSubstitution {
                equality, endpoint, ..
            } = &mut proof.rule
            else {
                unreachable!()
            };
            match mutation {
                0 => *endpoint = 2,
                1 => *endpoint = 0,
                2 => equality.rule = ProofRule::SemanticAxiom { index: 0 },
                _ => {
                    let Proposition::LessThan(left, right) = &mut proof.conclusion else {
                        unreachable!()
                    };
                    *left = right.clone();
                }
            }
            assert!(
                check_certificate(&context, &proof.conclusion, &[], &axioms, &proof).is_err(),
                "mutation {mutation}"
            );
        }
        let wrong_context = PropositionContext::from_value_types([
            (ValueId::new(1).unwrap(), ScalarType::Boolean),
            (
                ValueId::new(2).unwrap(),
                ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 16).unwrap()),
            ),
        ])
        .unwrap();
        assert!(
            check_certificate(
                &wrong_context,
                &original.conclusion,
                &[],
                &axioms,
                &original
            )
            .is_err()
        );
    }

    #[test]
    fn strict_substitution_preserves_existing_fixed_to_math_conclusion_projection() {
        let (context, axioms, mut proof) = fixture(true, false, false);
        proof.conclusion = lift_fixed_integer_relation(&proof.conclusion).unwrap();
        check_certificate(&context, &proof.conclusion, &[], &axioms, &proof).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::mathematical_core::bounded_verification_entries;
    use crate::proof::{assumption_proof, semantic_axiom_proof};
    use semantic_vocabulary::{
        IntegerMathTerm, IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType,
    };
    use std::slice::from_ref;

    use super::{
        AcceptedPremise, AcceptedProofRule, BTreeSet, IntegerAffineBoundConversionError,
        IntegerAffineWitnessError, IntegerCastBoundConversionError, IntegerCastChainWitnessError,
        IntegerCorrelatedForbiddenRootConversionError, IntegerCorrelatedForbiddenRootWitnessError,
        MathematicalCoreDecision, ProofError, ProofNode, ProofRule, Proposition,
        PropositionContext, ValueId, accept_certificate,
        accept_certificate_with_machine_parameters, check_certificate, lift_fixed_integer_relation,
    };
    use crate::IntegerAffineWitness;
    use crate::{
        CorrelatedAffineBranchWitness, IntegerCastChainWitness,
        IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment, correlated_step, proof_node,
    };
    use semantic_vocabulary::PropositionId;

    fn transport_certificate() -> (PropositionContext, Vec<Proposition>, ProofNode) {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let scalar_type = ScalarType::Integer(integer);
        let source = ValueId::new(1).expect("source");
        let destination = ValueId::new(2).expect("destination");
        let context = PropositionContext::from_value_types([
            (source, scalar_type),
            (destination, scalar_type),
        ])
        .expect("context");
        let source = ScalarTerm::value(source, scalar_type);
        let destination = ScalarTerm::value(destination, scalar_type);
        let ceiling = ScalarTerm::integer(integer, IntegerValue::Signed(100)).expect("ceiling");
        let axioms = vec![
            Proposition::LessOrEqual(source.clone(), ceiling.clone()),
            Proposition::Equal(destination.clone(), source),
        ];
        let proof = ProofNode {
            conclusion: Proposition::LessOrEqual(destination, ceiling),
            rule: ProofRule::ValueEqualityTransport {
                premise: Box::new(semantic_axiom_proof(axioms[0].clone(), 0)),
                equalities: vec![semantic_axiom_proof(axioms[1].clone(), 1)],
            },
        };
        (context, axioms, proof)
    }

    #[test]
    fn public_bounded_entrypoints_keep_callers_default_stack() {
        std::thread::spawn(|| {
            let (context, axioms, proof) = transport_certificate();
            let before = bounded_verification_entries();
            let accepted = accept_certificate(&context, &proof.conclusion, &[], &axioms, &proof)
                .expect("transport certificate");
            assert_eq!(bounded_verification_entries(), (before.0 + 1, before.1 + 1));
            let mut denoted = crate::denote_bounded_certificate(
                &context,
                &proof.conclusion,
                &[],
                &axioms,
                &proof,
            )
            .expect("independent public denotation");
            assert_eq!(bounded_verification_entries(), (before.0 + 1, before.1 + 2));
            crate::verify_mathematical_certificate(
                &mut denoted.arena,
                &denoted.certificate,
                &mut crate::Budget::default(),
            )
            .expect("independent raw judgment of the public denotation");
            assert_eq!(bounded_verification_entries(), (before.0 + 1, before.1 + 2));
            let verified = crate::verify_bounded_certificate(
                &context,
                &proof.conclusion,
                &[],
                &axioms,
                &proof,
                &mut crate::Budget::default(),
            )
            .expect("independent public kernel judgment");
            assert_eq!(bounded_verification_entries(), (before.0 + 1, before.1 + 3));
            assert_eq!(accepted.semantic_axioms.len(), 2);
            assert_eq!(
                accepted.mathematical_core,
                MathematicalCoreDecision::Judged(verified.receipt())
            );
            assert_eq!(denoted.receipt(), verified.receipt());
            let wrong = Proposition::Falsehood;
            assert!(accept_certificate(&context, &wrong, &[], &axioms, &proof).is_err());
            assert_eq!(bounded_verification_entries(), (before.0 + 2, before.1 + 3));
        })
        .join()
        .expect("all public bounded operations run on the caller's default stack");
    }

    #[test]
    fn standalone_certificate_receipt_uses_default_stack_and_matches_admission() {
        let (context, axioms, proof) = transport_certificate();
        let accepted = accept_certificate(&context, &proof.conclusion, &[], &axioms, &proof)
            .expect("transport certificate");
        let denoted = crate::verify_bounded_certificate(
            &context,
            &proof.conclusion,
            &[],
            &axioms,
            &proof,
            &mut crate::Budget::default(),
        )
        .expect("independent denotation and kernel check");
        let before = bounded_verification_entries();
        let receipt = denoted.receipt();
        assert_eq!(bounded_verification_entries(), before);
        assert_eq!(
            accepted.mathematical_core,
            MathematicalCoreDecision::Judged(receipt)
        );
    }

    #[test]
    #[ignore = "manual certificate admission timing; cargo test -- --ignored --nocapture"]
    fn measure_repeated_transport_certificate_admission() {
        let (context, axioms, proof) = transport_certificate();
        let started = std::time::Instant::now();
        for _ in 0..1000 {
            let accepted = accept_certificate(&context, &proof.conclusion, &[], &axioms, &proof)
                .expect("transport certificate");
            std::hint::black_box(accepted);
        }
        eprintln!("1000 transport admissions: {:?}", started.elapsed());
    }

    #[test]
    fn existing_assumption_rule_checks_fixed_carrier_normalization_to_math() {
        let integer_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let value_id = ValueId::new(88).expect("value");
        let value = ScalarTerm::value(value_id, ScalarType::Integer(integer_type));
        let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("zero");
        let assumption = Proposition::LessOrEqual(zero, value);
        let goal = lift_fixed_integer_relation(&assumption).expect("mathematical carrier relation");
        let context = integer_context(integer_type, &[88]);
        let proof = assumption_proof(goal.clone(), 0);
        assert!(check_certificate(&context, &goal, &[assumption], &[], &proof).is_ok());

        let wrong = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(1)),
            IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: value_id,
            },
        );
        let tampered = assumption_proof(wrong.clone(), 0);
        assert!(
            check_certificate(
                &context,
                &wrong,
                &[Proposition::LessOrEqual(
                    ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("zero"),
                    ScalarTerm::value(value_id, ScalarType::Integer(integer_type)),
                )],
                &[],
                &tampered
            )
            .is_err()
        );
    }

    #[test]
    fn disjunction_introduction_checks_one_exact_selected_child() {
        let left = Proposition::Atom(PropositionId::new(1).expect("left atom"));
        let right = Proposition::Atom(PropositionId::new(2).expect("right atom"));
        let goal = Proposition::Disjunction(vec![left.clone(), right.clone()]);
        let branch = |proposition: Proposition, assumption: usize, index: usize| ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(assumption_proof(proposition, assumption)),
                index,
            },
        };
        let assumptions = vec![left.clone(), right.clone()];

        for (proof, expected) in [
            (branch(left.clone(), 0, 0), left.clone()),
            (branch(right.clone(), 1, 1), right.clone()),
        ] {
            let accepted = accept_certificate(
                &PropositionContext::default(),
                &goal,
                &assumptions,
                &[],
                &proof,
            )
            .expect("selected disjunct is independently established");
            assert_eq!(
                accepted.rules,
                vec![
                    AcceptedProofRule::Assumption,
                    AcceptedProofRule::DisjunctionIntroduction,
                ]
            );
            assert_eq!(
                accepted.assumptions,
                vec![recorded_premise(usize::from(expected == right), &expected)]
            );
        }

        assert_eq!(
            check_certificate(
                &PropositionContext::default(),
                &goal,
                &assumptions,
                &[],
                &branch(left.clone(), 0, 2),
            ),
            Err(ProofError::UnknownDisjunct(2))
        );
        assert_eq!(
            check_certificate(
                &PropositionContext::default(),
                &goal,
                &assumptions,
                &[],
                &branch(left.clone(), 0, 1),
            ),
            Err(ProofError::DisjunctConclusionMismatch)
        );
        let non_disjunction = Proposition::Implication {
            premise: Box::new(left.clone()),
            conclusion: Box::new(left.clone()),
        };
        assert_eq!(
            check_certificate(
                &PropositionContext::default(),
                &non_disjunction,
                &assumptions,
                &[],
                &ProofNode {
                    conclusion: non_disjunction.clone(),
                    rule: ProofRule::DisjunctionIntroduction {
                        disjunct: Box::new(assumption_proof(left, 0)),
                        index: 0,
                    },
                },
            ),
            Err(ProofError::RuleConclusionMismatch(
                "disjunction introduction"
            ))
        );
    }

    #[test]
    fn integer_order_transitivity_weakens_a_negative_bound_for_nonzero() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = integer_value(integer, 1);
        let literal = |value| signed_literal(integer, value);
        let negative_two_bound = Proposition::LessOrEqual(divisor.clone(), literal(-2));
        let negative_one_bound = Proposition::LessOrEqual(divisor.clone(), literal(-1));
        let positive_bound = Proposition::LessOrEqual(literal(1), divisor.clone());
        let goal = Proposition::Disjunction(vec![negative_one_bound.clone(), positive_bound]);
        let weakened = ProofNode {
            conclusion: negative_one_bound,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(semantic_axiom_proof(
                    negative_two_bound.clone(),
                    0,
                )),
                middle_less_or_equal_right: Box::new(
                    proof_node!(Proposition::LessOrEqual(literal(-2), literal(-1)) => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)),
                ),
            },
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(weakened),
                index: 0,
            },
        };
        let context = integer_context(integer, &[1]);
        let acceptance =
            accept_certificate(&context, &goal, &[], from_ref(&negative_two_bound), &proof)
                .expect("the tighter negative bound proves the canonical negative arm");
        assert_eq!(
            acceptance.rules,
            vec![
                AcceptedProofRule::Primitive,
                AcceptedProofRule::SemanticAxiom,
                AcceptedProofRule::DisjunctionIntroduction,
                AcceptedProofRule::IntegerLessOrEqualTransitivity,
            ]
        );
        assert_eq!(
            acceptance.semantic_axioms,
            vec![recorded_premise(0, &negative_two_bound)]
        );
    }

    #[test]
    fn integer_order_transitivity_requires_exact_middle_endpoints_and_relations() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(ValueId::new(1).expect("left"), ScalarType::Integer(integer));
        let literal = |value| signed_literal(integer, value);
        let first = Proposition::LessOrEqual(left.clone(), literal(-2));
        let context = integer_context(integer, &[1]);
        let proof = |second: Proposition, conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(assumption_proof(first.clone(), 0)),
                middle_less_or_equal_right: Box::new(
                    proof_node!(second => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)),
                ),
            },
        };

        let expected = Proposition::LessOrEqual(left.clone(), literal(-1));
        assert_eq!(
            check_certificate(
                &context,
                &expected,
                from_ref(&first),
                &[],
                &proof(
                    Proposition::LessOrEqual(literal(-3), literal(-1)),
                    expected.clone(),
                ),
            ),
            Err(ProofError::IntegerOrderMiddleMismatch),
        );

        let wider = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let wider_literal =
            |value| ScalarTerm::integer(wider, IntegerValue::Signed(value)).expect("i16 literal");
        assert_eq!(
            check_certificate(
                &context,
                &expected,
                from_ref(&first),
                &[],
                &proof(
                    Proposition::LessOrEqual(wider_literal(-2), wider_literal(-1)),
                    expected.clone(),
                ),
            ),
            Err(ProofError::IntegerOrderMiddleMismatch),
        );

        let wrong_conclusion = Proposition::LessOrEqual(left, literal(0));
        assert_eq!(
            check_certificate(
                &context,
                &wrong_conclusion,
                from_ref(&first),
                &[],
                &proof(
                    Proposition::LessOrEqual(literal(-2), literal(-1)),
                    wrong_conclusion.clone(),
                ),
            ),
            Err(ProofError::IntegerOrderConclusionMismatch),
        );

        assert_eq!(
            check_certificate(
                &context,
                &expected,
                from_ref(&first),
                &[],
                &proof(
                    Proposition::Equal(literal(-2), literal(-2)),
                    expected.clone(),
                ),
            ),
            Err(ProofError::RulePremiseMismatch("integer <= transitivity")),
        );
    }

    #[test]
    fn integer_order_substitution_transports_either_endpoint_in_either_equality_orientation() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = integer_value(integer, 1);
        let literal = |value| signed_literal(integer, value);
        let context = integer_context(integer, &[1]);

        let positive_literal = Proposition::LessOrEqual(literal(1), literal(5));
        let positive_equality = Proposition::Equal(divisor.clone(), literal(5));
        let positive = Proposition::LessOrEqual(literal(1), divisor.clone());
        let positive_proof = ProofNode {
            conclusion: positive.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(
                    proof_node!(positive_literal => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)),
                ),
                equality: Box::new(semantic_axiom_proof(positive_equality.clone(), 0)),
                endpoint: 1,
            },
        };
        let accepted = accept_certificate(
            &context,
            &positive,
            &[],
            from_ref(&positive_equality),
            &positive_proof,
        )
        .expect("a literal equality transports the right endpoint");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Primitive,
                AcceptedProofRule::SemanticAxiom,
                AcceptedProofRule::IntegerOrderSubstitution,
            ]
        );
        assert_eq!(
            accepted.semantic_axioms,
            vec![recorded_premise(0, &positive_equality)]
        );

        let negative_literal = Proposition::LessOrEqual(literal(-2), literal(-1));
        let reverse_negative_equality = Proposition::Equal(literal(-2), divisor.clone());
        let negative = Proposition::LessOrEqual(divisor, literal(-1));
        let negative_proof = ProofNode {
            conclusion: negative.clone(),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(
                    proof_node!(negative_literal => ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation)),
                ),
                equality: Box::new(assumption_proof(reverse_negative_equality.clone(), 0)),
                endpoint: 0,
            },
        };
        let accepted = accept_certificate(
            &context,
            &negative,
            from_ref(&reverse_negative_equality),
            &[],
            &negative_proof,
        )
        .expect("a reverse equality transports the left endpoint");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Primitive,
                AcceptedProofRule::Assumption,
                AcceptedProofRule::IntegerOrderSubstitution,
            ]
        );
    }

    #[test]
    fn integer_order_substitution_rejects_every_shape_and_endpoint_mismatch() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = integer_value(integer, 1);
        let literal = |value| signed_literal(integer, value);
        let context = integer_context(integer, &[1]);
        let relation = Proposition::LessOrEqual(literal(1), literal(5));
        let equality = Proposition::Equal(literal(5), divisor.clone());
        let conclusion = Proposition::LessOrEqual(literal(1), divisor.clone());
        let child = |conclusion: Proposition, rule: ProofRule| ProofNode { conclusion, rule };
        let proof = |relation: ProofNode,
                     equality: ProofNode,
                     endpoint: usize,
                     conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(relation),
                equality: Box::new(equality),
                endpoint,
            },
        };
        let relation_child = || {
            child(
                relation.clone(),
                ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            )
        };
        let equality_child = || child(equality.clone(), ProofRule::SemanticAxiom { index: 0 });
        let check = |proof: &ProofNode, goal: &Proposition, axioms: &[Proposition]| {
            check_certificate(&context, goal, &[], axioms, proof)
        };

        assert_eq!(
            check(
                &proof(relation_child(), equality_child(), 2, conclusion.clone()),
                &conclusion,
                from_ref(&equality),
            ),
            Err(ProofError::UnknownIntegerOrderEndpoint(2)),
        );

        let changed_other_endpoint = Proposition::LessOrEqual(literal(0), divisor.clone());
        assert_eq!(
            check(
                &proof(
                    relation_child(),
                    equality_child(),
                    1,
                    changed_other_endpoint.clone(),
                ),
                &changed_other_endpoint,
                from_ref(&equality),
            ),
            Err(ProofError::IntegerOrderUnchangedEndpointMismatch),
        );

        let unrelated = Proposition::Equal(literal(4), divisor);
        assert_eq!(
            check(
                &proof(
                    relation_child(),
                    child(unrelated.clone(), ProofRule::SemanticAxiom { index: 0 },),
                    1,
                    conclusion.clone(),
                ),
                &conclusion,
                from_ref(&unrelated),
            ),
            Err(ProofError::IntegerOrderSubstitutionMismatch),
        );

        let truth = child(
            Proposition::Truth,
            ProofRule::Primitive(PrimitiveJudgment::Truth),
        );
        assert_eq!(
            check(
                &proof(truth.clone(), equality_child(), 1, conclusion.clone()),
                &conclusion,
                from_ref(&equality),
            ),
            Err(ProofError::RulePremiseMismatch(
                "integer order substitution relation"
            )),
        );
        assert_eq!(
            check(
                &proof(relation_child(), truth.clone(), 1, conclusion.clone()),
                &conclusion,
                &[],
            ),
            Err(ProofError::RulePremiseMismatch(
                "integer order substitution equality"
            )),
        );
        assert_eq!(
            check(
                &proof(relation_child(), equality_child(), 1, Proposition::Truth),
                &Proposition::Truth,
                from_ref(&equality),
            ),
            Err(ProofError::RuleConclusionMismatch(
                "integer order substitution"
            )),
        );
        assert_eq!(
            check(
                &proof(
                    child(relation, ProofRule::SemanticAxiom { index: 1 }),
                    equality_child(),
                    1,
                    conclusion.clone(),
                ),
                &conclusion,
                from_ref(&equality),
            ),
            Err(ProofError::UnknownSemanticAxiom(1)),
        );
    }

    #[test]
    fn integer_affine_bound_checks_root_proof_normalization_and_exact_custody() {
        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root = integer_value(integer, 1);
        let target = integer_value(integer, 2);
        let sibling = integer_value(integer, 3);
        let literal = |value| signed_literal(integer, value);
        let context = integer_context(integer, &[1, 2, 3]);
        let root_bound = Proposition::LessOrEqual(literal(1), root.clone());
        let landing = Proposition::Equal(sibling.clone(), literal(2));
        let definition = Proposition::Equal(
            target.clone(),
            ScalarTerm::exact_integer_add(integer, root.clone(), sibling).expect("exact add"),
        );
        let semantic_axioms = [landing.clone(), definition.clone()];
        let conclusion = Proposition::LessOrEqual(literal(3), target.clone());
        let proof =
            |definition_axioms: Vec<usize>, literal_axioms, conclusion: Proposition| ProofNode {
                conclusion,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
                    witness: IntegerAffineWitness {
                        root: root.clone(),
                        target: target.clone(),
                        literal_axioms,
                        definition_axioms,
                    },
                },
            };

        let accepted = accept_certificate(
            &context,
            &conclusion,
            from_ref(&root_bound),
            &semantic_axioms,
            &proof(vec![1], vec![Some(0)], conclusion.clone()),
        )
        .expect("root proof and exact affine definition prove the mapped bound");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::IntegerAffineBound,
            ]
        );
        assert_eq!(
            accepted.semantic_axioms,
            vec![
                recorded_premise(0, &landing),
                recorded_premise(1, &definition),
            ]
        );
        assert_eq!(
            check_certificate(
                &context,
                &conclusion,
                from_ref(&root_bound),
                &semantic_axioms,
                &proof(vec![2], vec![Some(0)], conclusion.clone()),
            ),
            Err(ProofError::IntegerAffineWitness(
                IntegerAffineWitnessError::UnknownSemanticAxiom(2),
            )),
        );
        let wrong_conclusion = Proposition::LessOrEqual(literal(2), target.clone());
        assert_eq!(
            check_certificate(
                &context,
                &wrong_conclusion,
                from_ref(&root_bound),
                &semantic_axioms,
                &proof(vec![1], vec![Some(0)], wrong_conclusion.clone()),
            ),
            Err(ProofError::IntegerAffineBoundConversion(
                IntegerAffineBoundConversionError::ConclusionMismatch,
            )),
        );
        let non_order_proof = ProofNode {
            conclusion: conclusion.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(
                    proof_node!(Proposition::Truth => ProofRule::Primitive(PrimitiveJudgment::Truth)),
                ),
                witness: IntegerAffineWitness {
                    root,
                    target,
                    definition_axioms: vec![1],
                    literal_axioms: vec![Some(0)],
                },
            },
        };
        assert_eq!(
            check_certificate(
                &context,
                &conclusion,
                from_ref(&root_bound),
                &semantic_axioms,
                &non_order_proof,
            ),
            Err(ProofError::IntegerAffineBoundConversion(
                IntegerAffineBoundConversionError::TruthRootWithoutTotalImage,
            )),
        );
    }

    #[test]
    fn integer_cast_bound_checks_complete_definition_word_and_exact_custody() {
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root = integer_value(i16_type, 1);
        let target = integer_value(i8_type, 2);
        let middle = integer_value(u16_type, 3);
        let context = values_context([
            (1, ScalarType::Integer(i16_type)),
            (2, ScalarType::Integer(i8_type)),
            (3, ScalarType::Integer(u16_type)),
        ]);
        let root_bound = Proposition::LessOrEqual(
            ScalarTerm::integer(i16_type, IntegerValue::Signed(1)).expect("i16 one"),
            root.clone(),
        );
        let definition = Proposition::Equal(
            target.clone(),
            ScalarTerm::integer_exact_cast(i16_type, i8_type, root.clone()).expect("partial cast"),
        );
        let conclusion = Proposition::LessOrEqual(
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("i8 one"),
            target.clone(),
        );
        let proof = |definition_axioms, conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::IntegerCastBound {
                root_bound: Box::new(assumption_proof(root_bound.clone(), 0)),
                witness: IntegerCastChainWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms,
                },
            },
        };

        let accepted = accept_certificate(
            &context,
            &conclusion,
            from_ref(&root_bound),
            from_ref(&definition),
            &proof(vec![0], conclusion.clone()),
        )
        .expect("one cast maps the independently proved root bound");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::IntegerCastBound,
            ]
        );
        assert_eq!(
            accepted.semantic_axioms,
            vec![recorded_premise(0, &definition)]
        );
        let first_definition = Proposition::Equal(
            middle.clone(),
            ScalarTerm::integer_exact_cast(i16_type, u16_type, root.clone())
                .expect("first partial cast"),
        );
        let second_definition = Proposition::Equal(
            target.clone(),
            ScalarTerm::integer_exact_cast(u16_type, i8_type, middle).expect("second partial cast"),
        );
        let multi_axioms = [first_definition.clone(), second_definition.clone()];
        let accepted = accept_certificate(
            &context,
            &conclusion,
            from_ref(&root_bound),
            &multi_axioms,
            &proof(vec![0, 1], conclusion.clone()),
        )
        .expect("the complete contiguous cast word maps the root bound");
        assert_eq!(
            accepted.semantic_axioms,
            vec![
                recorded_premise(0, &first_definition),
                recorded_premise(1, &second_definition),
            ],
        );
        assert_eq!(
            check_certificate(
                &context,
                &conclusion,
                from_ref(&root_bound),
                from_ref(&definition),
                &proof(vec![1], conclusion.clone()),
            ),
            Err(ProofError::IntegerCastChainWitness(
                IntegerCastChainWitnessError::UnknownSemanticAxiom(1),
            )),
        );
        let wrong_conclusion = Proposition::LessOrEqual(
            target.clone(),
            ScalarTerm::integer(i8_type, IntegerValue::Signed(1)).expect("i8 one"),
        );
        assert_eq!(
            check_certificate(
                &context,
                &wrong_conclusion,
                from_ref(&root_bound),
                from_ref(&definition),
                &proof(vec![0], wrong_conclusion.clone()),
            ),
            Err(ProofError::IntegerCastBoundConversion(
                IntegerCastBoundConversionError::ConclusionTargetMismatch,
            )),
        );
    }

    #[test]
    fn implication_certificate_is_checked_structurally() {
        let proposition = Proposition::Atom(PropositionId::new(1).expect("atom identity"));
        let goal = Proposition::Implication {
            premise: Box::new(proposition.clone()),
            conclusion: Box::new(proposition.clone()),
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(assumption_proof(proposition, 0)),
            },
        };
        let accepted = accept_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
            .expect("P implies P");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Assumption,
                AcceptedProofRule::ImplicationIntroduction,
            ]
        );
        assert!(accepted.assumptions.is_empty());
    }

    #[test]
    fn excluded_middle_is_expressible_and_a_cited_premise_stays_recorded() {
        let atom = Proposition::Atom(PropositionId::new(1).expect("undecided atom"));
        let goal = Proposition::Disjunction(vec![
            atom.clone(),
            Proposition::Implication {
                premise: Box::new(atom),
                conclusion: Box::new(Proposition::Falsehood),
            },
        ]);
        let context = PropositionContext::default();
        let proof = assumption_proof(goal.clone(), 0);

        assert_eq!(
            accept_certificate(&context, &goal, &[], &[], &proof),
            Err(ProofError::UnknownAssumption(0))
        );

        let accepted = accept_certificate(&context, &goal, from_ref(&goal), &[], &proof)
            .expect("excluded middle supplied as an exact scoped premise");
        assert_eq!(accepted.assumptions, vec![recorded_premise(0, &goal)]);
        assert_eq!(accepted.rules, vec![AcceptedProofRule::Assumption]);
        assert!(accepted.semantic_axioms.is_empty());
        assert!(matches!(
            accepted.mathematical_core,
            MathematicalCoreDecision::Judged(_)
        ));
    }

    #[test]
    fn accepted_trust_closure_binds_the_exact_cited_premise() {
        let cited = Proposition::Atom(PropositionId::new(1).expect("cited atom"));
        let replacement = Proposition::Atom(PropositionId::new(2).expect("replacement atom"));
        let proof = assumption_proof(cited.clone(), 0);
        let accepted = accept_certificate(
            &PropositionContext::default(),
            &cited,
            from_ref(&cited),
            &[],
            &proof,
        )
        .expect("exact cited premise");
        assert_eq!(accepted.assumptions, vec![recorded_premise(0, &cited)]);
        assert_eq!(
            accept_certificate(
                &PropositionContext::default(),
                &cited,
                &[replacement],
                &[],
                &proof,
            ),
            Err(ProofError::AssumptionConclusionMismatch(0))
        );
    }

    #[test]
    fn nested_scopes_do_not_export_discharged_same_index_premises() {
        let first = Proposition::Atom(PropositionId::new(1).expect("first atom"));
        let second = Proposition::Atom(PropositionId::new(2).expect("second atom"));
        let implication = |proposition: &Proposition| Proposition::Implication {
            premise: Box::new(proposition.clone()),
            conclusion: Box::new(proposition.clone()),
        };
        let branch = |proposition: &Proposition| ProofNode {
            conclusion: implication(proposition),
            rule: ProofRule::ImplicationIntroduction {
                body: Box::new(assumption_proof(proposition.clone(), 0)),
            },
        };
        let goal = Proposition::Conjunction(vec![implication(&first), implication(&second)]);
        let proof = proof_node!(goal.clone() => ProofRule::ConjunctionIntroduction(vec![branch(&first), branch(&second)]));
        let accepted = accept_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
            .expect("both nested implication premises");
        assert!(accepted.assumptions.is_empty());
    }

    #[test]
    fn semantic_equalities_compose_only_through_the_same_middle_term() {
        let integer = IntegerType::new(IntegerSign::Signed, 32).expect("i32");
        let a = ScalarTerm::value(ValueId::new(1).expect("a"), ScalarType::Integer(integer));
        let b = ScalarTerm::value(ValueId::new(2).expect("b"), ScalarType::Integer(integer));
        let seven = ScalarTerm::integer(integer, IntegerValue::Signed(7)).expect("seven");
        let axioms = vec![
            Proposition::Equal(a.clone(), b.clone()),
            Proposition::Equal(b.clone(), seven.clone()),
        ];
        let goal = Proposition::Equal(a.clone(), seven.clone());
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(semantic_axiom_proof(axioms[0].clone(), 0)),
                middle_equals_right: Box::new(semantic_axiom_proof(axioms[1].clone(), 1)),
            },
        };
        let context = integer_context(integer, &[1, 2]);
        let accepted =
            accept_certificate(&context, &goal, &[], &axioms, &proof).expect("transitive equality");
        assert_eq!(
            accepted.semantic_axioms,
            vec![
                recorded_premise(0, &axioms[0]),
                recorded_premise(1, &axioms[1]),
            ]
        );
    }

    #[test]
    fn canonical_content_equalities_compose_through_either_orientation() {
        use semantic_vocabulary::{
            ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
            ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
            ContentStructuralPlace, ContentTerm, PlaceId, StructuralPlaceKind,
        };

        let root = PlaceId::new(1).expect("place");
        let projection = ContentProjectionIdentity {
            domain: ContentDomainId::new(2).expect("domain"),
            projection_report_fingerprint: 3,
        };
        let term = |field: &str| ContentTerm::Projection {
            projection,
            subject: ContentStructuralPlace {
                version: ContentPlaceVersion::Current,
                root,
                segments: vec![ContentPlaceSegment::Field(field.to_owned())],
            },
        };
        let algebra = ContentAlgebra {
            kind: ContentAlgebraKind::CountedQuantity,
            parameter: "Byte".to_owned(),
        };
        let a = term("a");
        let b = term("b");
        let c = term("c");
        let axioms = vec![
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                a.clone(),
                c.clone(),
            )),
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                b.clone(),
                c,
            )),
        ];
        let goal = Proposition::ContentConservation(ContentConservation::new(algebra, a, b));
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::EqualityTransitivity {
                left_equals_middle: Box::new(semantic_axiom_proof(axioms[0].clone(), 0)),
                middle_equals_right: Box::new(semantic_axiom_proof(axioms[1].clone(), 1)),
            },
        };
        let context = PropositionContext::from_value_types_and_places(
            [],
            [(root, StructuralPlaceKind::Parameter { position: 0 })],
        )
        .expect("context");

        check_certificate(&context, &goal, &[], &axioms, &proof)
            .expect("canonical equality orientation must not erase transitivity");
    }

    fn correlated_division_fixture() -> (
        PropositionContext,
        Vec<Proposition>,
        Vec<Proposition>,
        BTreeSet<ValueId>,
        ProofNode,
    ) {
        let integer_type = IntegerType::new(IntegerSign::Signed, 8).expect("signed carrier");
        let scalar_type = ScalarType::Integer(integer_type);
        let value = |raw| ScalarTerm::value(ValueId::new(raw).expect("value"), scalar_type);
        let integer =
            |raw| ScalarTerm::integer(integer_type, IntegerValue::Signed(raw)).expect("literal");
        let root = value(100);
        let dividend = value(101);
        let product = value(102);
        let divisor = value(103);
        let semantic_axioms = vec![
            Proposition::Equal(
                dividend.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, root.clone(), integer(-2))
                    .expect("multiply"),
            ),
            Proposition::Equal(
                product.clone(),
                ScalarTerm::exact_integer_multiply(integer_type, root.clone(), integer(2))
                    .expect("multiply"),
            ),
            Proposition::Equal(
                divisor.clone(),
                ScalarTerm::exact_integer_add(integer_type, product, integer(1)).expect("add"),
            ),
        ];
        let requirements = vec![
            Proposition::LessOrEqual(integer(-1), root.clone()),
            Proposition::LessOrEqual(root.clone(), integer(0)),
        ];
        let goal = Proposition::Disjunction(vec![
            Proposition::LessOrEqual(divisor.clone(), integer(-2)),
            Proposition::LessOrEqual(integer(1), divisor.clone()),
            Proposition::Conjunction(vec![
                Proposition::LessOrEqual(divisor.clone(), integer(-1)),
                Proposition::LessOrEqual(integer(-127), dividend.clone()),
            ]),
        ]);
        let proof = ProofNode {
            conclusion: goal,
            rule: ProofRule::IntegerCorrelatedForbiddenRoots {
                witness: IntegerCorrelatedForbiddenRootWitness {
                    dividend: CorrelatedAffineBranchWitness {
                        root: root.clone(),
                        target: dividend,
                        steps: vec![correlated_step(0, None)],
                    },
                    divisor: CorrelatedAffineBranchWitness {
                        root: root.clone(),
                        target: divisor,
                        steps: vec![correlated_step(1, None), correlated_step(2, None)],
                    },
                    definition_axiom_count: 3,
                    lower_bound_axiom: 3,
                    upper_bound_axiom: 4,
                    conclusion: Proposition::Conjunction(requirements.clone()),
                },
            },
        };
        let context = integer_context(integer_type, &[100, 101, 102, 103]);
        (
            context,
            semantic_axioms,
            requirements,
            BTreeSet::from([ValueId::new(100).expect("root")]),
            proof,
        )
    }

    #[test]
    fn correlated_forbidden_root_rule_binds_parameter_and_exact_premise_closure() {
        let (context, axioms, requirements, parameters, proof) = correlated_division_fixture();
        let accepted = accept_certificate_with_machine_parameters(
            &context,
            &proof.conclusion,
            &requirements,
            &axioms,
            &parameters,
            &proof,
        )
        .expect("safe same-parameter correlated division");

        assert_eq!(
            accepted.rules,
            vec![AcceptedProofRule::IntegerCorrelatedForbiddenRoots]
        );
        assert_eq!(
            accepted
                .semantic_axioms
                .iter()
                .map(|premise| premise.index)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            accepted
                .assumptions
                .iter()
                .map(|premise| premise.index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn correlated_forbidden_root_rule_rejects_nonparameter_root_and_forged_conclusion() {
        let (context, axioms, requirements, _, proof) = correlated_division_fixture();
        assert!(matches!(
            accept_certificate_with_machine_parameters(
                &context,
                &proof.conclusion,
                &requirements,
                &axioms,
                &BTreeSet::from([ValueId::new(999).expect("unrelated parameter")]),
                &proof,
            ),
            Err(ProofError::IntegerCorrelatedForbiddenRootWitness(
                IntegerCorrelatedForbiddenRootWitnessError::RootNotSignedNative(_)
            ))
        ));

        let forged = proof_node!(Proposition::Truth => proof.rule);
        assert_eq!(
            accept_certificate_with_machine_parameters(
                &context,
                &forged.conclusion,
                &requirements,
                &axioms,
                &BTreeSet::from([ValueId::new(100).expect("root")]),
                &forged,
            ),
            Err(ProofError::IntegerCorrelatedForbiddenRootConversion(
                IntegerCorrelatedForbiddenRootConversionError::ConclusionMismatch,
            ))
        );
    }

    fn integer_context(integer: IntegerType, values: &[u64]) -> PropositionContext {
        values_context(
            values
                .iter()
                .map(|value| (*value, ScalarType::Integer(integer))),
        )
    }

    fn values_context(values: impl IntoIterator<Item = (u64, ScalarType)>) -> PropositionContext {
        PropositionContext::from_value_types(
            values
                .into_iter()
                .map(|(value, scalar_type)| (ValueId::new(value).expect("value"), scalar_type)),
        )
        .expect("context")
    }

    fn integer_value(integer: IntegerType, value: u64) -> ScalarTerm {
        ScalarTerm::value(
            ValueId::new(value).expect("value"),
            ScalarType::Integer(integer),
        )
    }

    fn signed_literal(integer: IntegerType, value: i128) -> ScalarTerm {
        ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("signed literal")
    }

    fn recorded_premise(index: usize, proposition: &Proposition) -> AcceptedPremise {
        AcceptedPremise {
            index,
            proposition: proposition.clone(),
        }
    }

    #[test]
    fn deep_logical_assumption_admission_uses_default_stack_and_exact_premise() {
        std::thread::spawn(|| {
        fn nested(last: Proposition) -> Proposition {
            let mut proposition = last;
            for depth in 0..8192 {
                proposition = match depth % 3 {
                    0 => Proposition::Implication { premise: Box::new(Proposition::Truth), conclusion: Box::new(proposition) },
                    1 => Proposition::Conjunction(vec![Proposition::Truth, proposition]),
                    _ => Proposition::Disjunction(vec![Proposition::Falsehood, proposition]),
                };
            }
            proposition
        }
        for valid in [true, false] {
            let goal = nested(Proposition::Truth);
            let assumptions = [nested(Proposition::Truth)];
            let proof = ProofNode { conclusion: nested(if valid { Proposition::Truth } else { Proposition::Falsehood }), rule: ProofRule::Assumption { index: 0 } };
            let context = PropositionContext::default();
            context.validate(&goal).expect("the whole source goal is admitted");
            eprintln!("8192-node assumption prepared; begin complete current-thread admission valid={valid}");
            let before = bounded_verification_entries();
            let result = super::accept_certificate_with_machine_parameters(&context, &goal, &assumptions, &[], &BTreeSet::new(), &proof);
            if valid {
                let accepted = result.expect("exact logical assumption is independently kernel judged");
                assert_eq!(accepted.assumptions.len(), 1);
                assert_eq!(accepted.assumptions[0].index, 0);
                assert!(accepted.assumptions[0].proposition == goal, "the exact source premise remains in the receipt");
                let MathematicalCoreDecision::Judged(receipt) = accepted.mathematical_core else { panic!("bounded rule labels cannot substitute for the kernel judgment"); };
                assert_eq!(receipt.context_depth, 1);
                assert_eq!(receipt.declarations, 0);
                assert_eq!(receipt.assumption_closure, 0);
            } else {
                assert!(matches!(result, Err(ProofError::AssumptionConclusionMismatch(0))), "a changed leaf cannot cite the old complete premise");
            }
            assert_eq!(bounded_verification_entries(), (before.0 + 1, before.1 + usize::from(valid)), "public admission and denotation execute on the default-stack caller");
            eprintln!("complete current-thread admission returned valid={valid}");
        }
    }).join().expect("complete logical admission and owned receipts stay on a default stack");
    }

    #[test]
    fn deep_scalar_assumption_admission_uses_default_stack_and_exact_premise() {
        std::thread::spawn(|| {
        fn statement(depth: usize, right: bool) -> Proposition {
            fn value(depth: usize, seed: bool) -> ScalarTerm {
                let mut value = ScalarTerm::Boolean(seed);
                for _ in 0..depth { value = ScalarTerm::BooleanNot { operand: Box::new(value) }; }
                value
            }
            Proposition::Equal(value(depth, true), value(depth, right))
        }
        let mut arena = crate::TermArena::new();
        let two = arena.insert(crate::Term::Two);
        let zero = arena.insert(crate::Term::TwoZero);
        let one = arena.insert(crate::Term::TwoOne);
        let mut endpoints = [one, one];
        for endpoint in &mut endpoints {
            for _ in 0..8192 {
                let motive = arena.insert(crate::Term::Lambda { domain: two, body: two });
                *endpoint = arena.insert(crate::Term::CaseTwo {
                    motive, zero_branch: one, one_branch: zero, scrutinee: *endpoint,
                });
            }
        }
        let expected = arena.insert(crate::Term::Id { ty: two, left: endpoints[0], right: endpoints[1] });
        let evidence = arena.insert(crate::Term::Variable(0));
        let certificate = crate::MathematicalCertificate::closed(vec![expected], evidence, expected);
        assert!(matches!(crate::verify_mathematical_certificate(&mut arena, &certificate, &mut crate::Budget::default()), Err(crate::CoreError::StepCeiling)));
        for (depth, valid) in [(8192, true), (64, true), (64, false)] {
            let goal = statement(depth, true);
            let context = PropositionContext::default();
            context.validate(&goal).expect("the complete scalar statement is well formed");
            eprintln!("{depth}-node scalar statement admitted; begin owned clone");
            let copied = goal.clone();
            eprintln!("owned clone returned; begin exact value operations");
            assert!(copied == goal);
            assert_eq!(copied.cmp(&goal), std::cmp::Ordering::Equal);
            use std::hash::{Hash, Hasher};
            let mut original_hash = std::collections::hash_map::DefaultHasher::new();
            let mut copy_hash = std::collections::hash_map::DefaultHasher::new();
            goal.hash(&mut original_hash);copied.hash(&mut copy_hash);
            assert_eq!(original_hash.finish(), copy_hash.finish());
            eprintln!("exact value operations returned; begin complete scalar admission valid={valid}");
            let assumptions = [statement(depth, true)];
            let proof = ProofNode { conclusion: statement(depth, valid), rule: ProofRule::Assumption { index: 0 } };
            let before = bounded_verification_entries();
            let result = super::accept_certificate_with_machine_parameters(&context, &goal, &assumptions, &[], &BTreeSet::new(), &proof);
            if depth == 8192 {
                assert!(matches!(result, Err(ProofError::MathematicalCore(ref error)) if matches!(error.as_ref(), crate::BoundedDenotationError::Kernel(crate::CoreError::StepCeiling))), "the deep source question must preserve the independent kernel's resource refusal");
            } else if valid {
                let accepted = result.expect("within the kernel's existing allowance, the complete exact scalar premise is kernel judged");
                assert_eq!(accepted.assumptions.len(), 1);
                assert_eq!(accepted.assumptions[0].index, 0);
                assert!(accepted.assumptions[0].proposition == goal);
                let MathematicalCoreDecision::Judged(receipt) = accepted.mathematical_core else { panic!("bounded rule labels cannot replace the exact kernel judgment"); };
                assert_eq!(receipt.context_depth, 1);
                assert_eq!(receipt.declarations, 0);
                assert_eq!(receipt.assumption_closure, 0);
            } else {
                assert!(matches!(result, Err(ProofError::AssumptionConclusionMismatch(0))));
            }
            assert_eq!(bounded_verification_entries(), (before.0 + 1, before.1 + usize::from(valid)));
            eprintln!("complete scalar admission returned depth={depth} valid={valid}");
        }
    }).join().expect("complete scalar admission and owned receipts stay on a default stack");
    }

    #[test]
    fn deep_open_integer_admission_preserves_exact_premises_on_default_stack() {
        std::thread::spawn(|| {
            let integer = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
            let scalar_type = ScalarType::Integer(integer);
            let id = ValueId::new(1).expect("source scalar");
            let context =
                PropositionContext::from_value_types([(id, scalar_type)]).expect("context");
            let statement = |depth, valid| {
                let mut value = if valid {
                    ScalarTerm::value(id, scalar_type)
                } else {
                    ScalarTerm::integer(integer, IntegerValue::Unsigned(1)).expect("changed leaf")
                };
                for _ in 0..depth {
                    value = ScalarTerm::ExactIntegerAdd {
                        scalar_type: integer,
                        left: Box::new(
                            ScalarTerm::integer(integer, IntegerValue::Unsigned(0)).expect("zero"),
                        ),
                        right: Box::new(value),
                    };
                }
                Proposition::Equal(value, ScalarTerm::value(id, scalar_type))
            };
            for (depth, valid) in [(128, true), (8192, true), (8192, false)] {
                let goal = statement(depth, true);
                context
                    .validate(&goal)
                    .expect("the complete source question is admitted");
                let copied = goal.clone();
                assert!(copied == goal);
                use std::hash::{Hash, Hasher};
                let mut original_hash = std::collections::hash_map::DefaultHasher::new();
                let mut copied_hash = std::collections::hash_map::DefaultHasher::new();
                goal.hash(&mut original_hash);
                copied.hash(&mut copied_hash);
                assert_eq!(original_hash.finish(), copied_hash.finish());
                let assumptions = [statement(depth, true)];
                let proof = ProofNode {
                    conclusion: statement(depth, valid),
                    rule: ProofRule::Assumption { index: 0 },
                };
                let before = bounded_verification_entries();
                let result = super::accept_certificate_with_machine_parameters(
                    &context,
                    &goal,
                    &assumptions,
                    &[],
                    &BTreeSet::new(),
                    &proof,
                );
                if valid {
                    let accepted = result
                        .expect("the independent kernel judges this exact open-integer premise");
                    assert_eq!(accepted.rules, vec![AcceptedProofRule::Assumption]);
                    assert_eq!(accepted.assumptions.len(), 1);
                    assert_eq!(accepted.assumptions[0].index, 0);
                    assert!(accepted.assumptions[0].proposition == goal);
                    assert!(accepted.semantic_axioms.is_empty());
                    let MathematicalCoreDecision::Judged(receipt) = accepted.mathematical_core
                    else {
                        panic!("bounded rule labels cannot replace the complete kernel judgment");
                    };
                    assert_eq!(
                        (
                            receipt.declarations,
                            receipt.assumption_closure,
                            receipt.context_depth
                        ),
                        (4, 4, 1)
                    );
                    assert!(receipt.arena_slots > depth as u32);
                    eprintln!("complete open-integer admission depth={depth} {receipt:?}");
                } else {
                    assert!(
                        matches!(result, Err(ProofError::AssumptionConclusionMismatch(0))),
                        "a changed deepest leaf cannot cite the original complete premise"
                    );
                }
                assert_eq!(
                    bounded_verification_entries(),
                    (before.0 + 1, before.1 + usize::from(valid)),
                    "public admission and denotation execute on the default-stack caller"
                );
            }
        })
        .join()
        .expect("open-integer admission and owned receipts use the platform default stack");
    }
}
