use std::collections::BTreeSet;

use semantic_vocabulary::{
    IntegerMathTerm, IntegerValue, Proposition, PropositionContext, ScalarTerm, ValueId,
};

use crate::{
    IntegerAffineBoundConversionError, IntegerAffineWitness, IntegerAffineWitnessError,
    IntegerCastBoundConversionError, IntegerCastChainWitness, IntegerCastChainWitnessError,
    IntegerCorrelatedForbiddenRootConversionError, IntegerCorrelatedForbiddenRootWitness,
    IntegerCorrelatedForbiddenRootWitnessError, KernelError, PrimitiveJudgment,
    check_integer_affine_bound_conversion, check_integer_affine_witness,
    check_integer_cast_bound_conversion, check_integer_cast_chain_witness,
    check_integer_correlated_forbidden_root_conversion,
    check_integer_correlated_forbidden_root_witness, decide_primitive, integer_affine_truth_bounds,
    integer_cast_truth_bounds, map_integer_affine_bound,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofNode {
    pub conclusion: Proposition,
    pub rule: ProofRule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofRule {
    Primitive(PrimitiveJudgment),
    /// Cite one verifier-reconstructed semantic axiom.
    SemanticAxiom {
        index: usize,
    },
    Assumption {
        index: usize,
    },
    ConjunctionIntroduction(Vec<ProofNode>),
    ConjunctionElimination {
        conjunction: Box<ProofNode>,
        conjunct: usize,
    },
    DisjunctionIntroduction {
        disjunct: Box<ProofNode>,
        index: usize,
    },
    /// Prove one common conclusion under each ordered disjunct, discharging
    /// the single assumption appended for that branch.
    DisjunctionElimination {
        disjunction: Box<ProofNode>,
        branches: Vec<ProofNode>,
    },
    ImplicationIntroduction {
        body: Box<ProofNode>,
    },
    ImplicationElimination {
        implication: Box<ProofNode>,
        premise: Box<ProofNode>,
    },
    EqualityTransitivity {
        left_equals_middle: Box<ProofNode>,
        middle_equals_right: Box<ProofNode>,
    },
    IntegerLessOrEqualTransitivity {
        left_less_or_equal_middle: Box<ProofNode>,
        middle_less_or_equal_right: Box<ProofNode>,
    },
    IntegerLessOrEqualSubstitution {
        relation: Box<ProofNode>,
        equality: Box<ProofNode>,
        endpoint: usize,
    },
    /// Map one independently proved root bound through an exact, ordered
    /// endpoint-transform witness whose affine or landed-count shift
    /// semantic-axiom custody is rechecked.
    IntegerAffineBound {
        root_bound: Box<ProofNode>,
        witness: IntegerAffineWitness,
    },
    /// Map two independently proved scalar endpoints through one cited prior
    /// exact-add definition. This is distinct from an affine literal sibling.
    IntegerExactAddDefinitionBound {
        left_bound: Box<ProofNode>,
        right_bound: Box<ProofNode>,
        definition_axiom: usize,
    },
    /// Map one independently proved root bound through one checked ordered
    /// word of partial fixed-integer exact casts and strict widening identities.
    IntegerCastBound {
        root_bound: Box<ProofNode>,
        witness: IntegerCastChainWitness,
    },
    /// Prove canonical signed exact-division definedness by replaying two
    /// correlated affine branches over one machine signature parameter.
    IntegerCorrelatedForbiddenRoots {
        witness: IntegerCorrelatedForbiddenRootWitness,
    },
}

/// Proof-rule families exercised by one accepted certificate. The set is a
/// deterministic review projection, not a second proof checker.
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
    IntegerLessOrEqualTransitivity,
    IntegerLessOrEqualSubstitution,
    IntegerAffineBound,
    IntegerExactAddDefinitionBound,
    IntegerCastBound,
    IntegerCorrelatedForbiddenRoots,
}

/// One premise that materially participates in an accepted derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcceptedPremise {
    pub index: usize,
    pub proposition: Proposition,
}

/// Auditable closure produced by the same traversal that accepts a
/// certificate. Consumers must not reconstruct this information by walking
/// the source or guessing from the conclusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CertificateAcceptance {
    pub rules: Vec<AcceptedProofRule>,
    /// Only premises supplied to the certificate, never discharged assumptions
    /// introduced by an implication or case-analysis branch.
    pub assumptions: Vec<AcceptedPremise>,
    pub semantic_axioms: Vec<AcceptedPremise>,
}

#[derive(Default)]
struct AcceptanceBuilder {
    ambient_assumption_count: usize,
    rules: std::collections::BTreeSet<AcceptedProofRule>,
    assumptions: Vec<AcceptedPremise>,
    semantic_axioms: Vec<AcceptedPremise>,
}

impl AcceptanceBuilder {
    fn record_assumption(&mut self, index: usize, proposition: &Proposition) {
        if index < self.ambient_assumption_count {
            record_premise(&mut self.assumptions, index, proposition);
        }
    }

    fn record_semantic_axiom(&mut self, index: usize, proposition: &Proposition) {
        record_premise(&mut self.semantic_axioms, index, proposition);
    }

    fn finish(self) -> CertificateAcceptance {
        CertificateAcceptance {
            rules: self.rules.into_iter().collect(),
            assumptions: self.assumptions,
            semantic_axioms: self.semantic_axioms,
        }
    }
}

fn record_premise(premises: &mut Vec<AcceptedPremise>, index: usize, proposition: &Proposition) {
    if !premises
        .iter()
        .any(|premise| premise.index == index && premise.proposition == *proposition)
    {
        premises.push(AcceptedPremise {
            index,
            proposition: proposition.clone(),
        });
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

pub fn check_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<(), ProofError> {
    accept_certificate_with_machine_parameters(
        context,
        goal,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
        proof,
    )
    .map(|_| ())
}

/// Check a certificate and return its exact premise/rule trust closure.
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

/// Check a certificate with the verifier-reconstructed scalar signature roots
/// that parameter-custody proof rules may name.
pub fn accept_certificate_with_machine_parameters(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<CertificateAcceptance, ProofError> {
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
    Ok(acceptance.finish())
}

mod traversal;
use traversal::check_node;

// The conclusion and children have already been checked by the scoped
// postorder traversal.
fn check_node_locally(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
    acceptance: &mut AcceptanceBuilder,
) -> Result<(), ProofError> {
    match &proof.rule {
        ProofRule::Primitive(judgment) => {
            acceptance.rules.insert(AcceptedProofRule::Primitive);
            decide_primitive(context, &proof.conclusion, *judgment)
                .map_err(ProofError::PrimitiveJudgment)
        }
        ProofRule::SemanticAxiom { index } => {
            acceptance.rules.insert(AcceptedProofRule::SemanticAxiom);
            let axiom = semantic_axioms
                .get(*index)
                .ok_or(ProofError::UnknownSemanticAxiom(*index))?;
            if !propositions_match_under_integer_math_normalization(axiom, &proof.conclusion) {
                return Err(ProofError::SemanticAxiomConclusionMismatch(*index));
            }
            acceptance.record_semantic_axiom(*index, axiom);
            Ok(())
        }
        ProofRule::Assumption { index } => {
            acceptance.rules.insert(AcceptedProofRule::Assumption);
            let assumption = assumptions
                .get(*index)
                .ok_or(ProofError::UnknownAssumption(*index))?;
            if !propositions_match_under_integer_math_normalization(assumption, &proof.conclusion) {
                return Err(ProofError::AssumptionConclusionMismatch(*index));
            }
            acceptance.record_assumption(*index, assumption);
            Ok(())
        }
        ProofRule::ConjunctionIntroduction(conjuncts) => {
            acceptance
                .rules
                .insert(AcceptedProofRule::ConjunctionIntroduction);
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
        ProofRule::ConjunctionElimination {
            conjunction,
            conjunct,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::ConjunctionElimination);
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
        ProofRule::DisjunctionIntroduction { disjunct, index } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::DisjunctionIntroduction);
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
        ProofRule::DisjunctionElimination {
            disjunction,
            branches,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::DisjunctionElimination);
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
        ProofRule::ImplicationIntroduction { body } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::ImplicationIntroduction);
            let Proposition::Implication { conclusion, .. } = &proof.conclusion else {
                return Err(ProofError::RuleConclusionMismatch(
                    "implication introduction",
                ));
            };
            (&body.conclusion == conclusion.as_ref())
                .then_some(())
                .ok_or(ProofError::ImplicationConclusionMismatch)
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::ImplicationElimination);
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
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::EqualityTransitivity);
            match (
                &left_equals_middle.conclusion,
                &middle_equals_right.conclusion,
            ) {
                (
                    Proposition::Equal(left, first_middle),
                    Proposition::Equal(second_middle, right),
                ) => {
                    if first_middle != second_middle {
                        return Err(ProofError::EqualityMiddleMismatch);
                    }
                    let composed = Proposition::Equal(left.clone(), right.clone());
                    if !propositions_match_under_integer_math_normalization(
                        &composed,
                        &proof.conclusion,
                    ) {
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
                    (proof.conclusion == Proposition::IntegerMathEqual(left, right))
                        .then_some(())
                        .ok_or(ProofError::EqualityConclusionMismatch)
                }
                (
                    Proposition::ContentConservation(left_equation),
                    Proposition::ContentConservation(right_equation),
                ) => {
                    let Proposition::ContentConservation(expected) = &proof.conclusion else {
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
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::IntegerLessOrEqualTransitivity);
            match (
                &left_less_or_equal_middle.conclusion,
                &middle_less_or_equal_right.conclusion,
            ) {
                (
                    Proposition::LessOrEqual(left, first_middle),
                    Proposition::LessOrEqual(second_middle, right),
                ) => {
                    if first_middle != second_middle {
                        return Err(ProofError::IntegerOrderMiddleMismatch);
                    }
                    let composed = Proposition::LessOrEqual(left.clone(), right.clone());
                    if !propositions_match_under_integer_math_normalization(
                        &composed,
                        &proof.conclusion,
                    ) {
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
                    (proof.conclusion
                        == Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()))
                    .then_some(())
                    .ok_or(ProofError::IntegerOrderConclusionMismatch)
                }
                _ => Err(ProofError::RulePremiseMismatch("integer <= transitivity")),
            }
        }
        ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::IntegerLessOrEqualSubstitution);
            if let (
                Proposition::IntegerMathLessOrEqual(relation_left, relation_right),
                Proposition::IntegerMathEqual(equality_left, equality_right),
                Proposition::IntegerMathLessOrEqual(conclusion_left, conclusion_right),
            ) = (
                &relation.conclusion,
                &equality.conclusion,
                &proof.conclusion,
            ) {
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
                    endpoint => return Err(ProofError::UnknownIntegerOrderEndpoint(*endpoint)),
                };
                return ((equality_left == old_endpoint && equality_right == new_endpoint)
                    || (equality_right == old_endpoint && equality_left == new_endpoint))
                    .then_some(())
                    .ok_or(ProofError::IntegerOrderSubstitutionMismatch);
            }
            let Proposition::LessOrEqual(relation_left, relation_right) = &relation.conclusion
            else {
                return Err(ProofError::RulePremiseMismatch(
                    "integer <= substitution relation",
                ));
            };
            let Proposition::Equal(equality_left, equality_right) = &equality.conclusion else {
                return Err(ProofError::RulePremiseMismatch(
                    "integer <= substitution equality",
                ));
            };
            let normalized_conclusion = lower_integer_math_relation(&proof.conclusion)
                .unwrap_or_else(|| proof.conclusion.clone());
            let Proposition::LessOrEqual(conclusion_left, conclusion_right) =
                &normalized_conclusion
            else {
                return Err(ProofError::RuleConclusionMismatch(
                    "integer <= substitution",
                ));
            };
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
                endpoint => return Err(ProofError::UnknownIntegerOrderEndpoint(*endpoint)),
            };
            if !((equality_left == old_endpoint && equality_right == new_endpoint)
                || (equality_right == old_endpoint && equality_left == new_endpoint))
            {
                return Err(ProofError::IntegerOrderSubstitutionMismatch);
            }
            Ok(())
        }
        ProofRule::IntegerAffineBound {
            root_bound,
            witness,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::IntegerAffineBound);
            let form = check_integer_affine_witness(context, semantic_axioms, witness)
                .map_err(ProofError::IntegerAffineWitness)?;
            let normalized_conclusion = lower_integer_math_relation(&proof.conclusion)
                .unwrap_or_else(|| proof.conclusion.clone());
            if root_bound.conclusion == Proposition::Truth {
                let bounds = integer_affine_truth_bounds(&form)
                    .map_err(ProofError::IntegerAffineBoundConversion)?;
                if !bounds.contains(&normalized_conclusion) {
                    return Err(ProofError::IntegerAffineBoundConversion(
                        IntegerAffineBoundConversionError::ConclusionMismatch,
                    ));
                }
            } else {
                check_integer_affine_bound_conversion(
                    &form,
                    &root_bound.conclusion,
                    &normalized_conclusion,
                )
                .map_err(ProofError::IntegerAffineBoundConversion)?;
            }
            for (&definition_index, &literal_index) in witness
                .definition_axioms
                .iter()
                .zip(&witness.literal_axioms)
            {
                if let Some(index) = literal_index {
                    let proposition = semantic_axioms
                        .get(index)
                        .ok_or(ProofError::UnknownSemanticAxiom(index))?;
                    acceptance.record_semantic_axiom(index, proposition);
                }
                let proposition = semantic_axioms
                    .get(definition_index)
                    .ok_or(ProofError::UnknownSemanticAxiom(definition_index))?;
                acceptance.record_semantic_axiom(definition_index, proposition);
            }
            Ok(())
        }
        ProofRule::IntegerExactAddDefinitionBound {
            left_bound,
            right_bound,
            definition_axiom,
        } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::IntegerExactAddDefinitionBound);
            let definition = semantic_axioms
                .get(*definition_axiom)
                .ok_or(ProofError::UnknownSemanticAxiom(*definition_axiom))?;
            context
                .validate(definition)
                .map_err(ProofError::MalformedProposition)?;
            let Proposition::Equal(first, second) = definition else {
                return Err(ProofError::RulePremiseMismatch(
                    "integer exact-add definition",
                ));
            };
            let (output, expression) = match (first, second) {
                (ScalarTerm::Value { .. }, ScalarTerm::ExactIntegerAdd { .. }) => (first, second),
                (ScalarTerm::ExactIntegerAdd { .. }, ScalarTerm::Value { .. }) => (second, first),
                _ => {
                    return Err(ProofError::RulePremiseMismatch(
                        "integer exact-add definition",
                    ));
                }
            };
            let ScalarTerm::ExactIntegerAdd {
                scalar_type, left, ..
            } = expression
            else {
                unreachable!("matched exact-add definition")
            };
            if scalar_type.carrier() != semantic_vocabulary::IntegerCarrier::Fixed
                || output.scalar_type() != semantic_vocabulary::ScalarType::Integer(*scalar_type)
            {
                return Err(ProofError::RulePremiseMismatch(
                    "integer exact-add definition type",
                ));
            }
            let witness = IntegerAffineWitness {
                root: left.as_ref().clone(),
                target: expression.clone(),
                definition_axioms: Vec::new(),
                literal_axioms: Vec::new(),
            };
            let form = check_integer_affine_witness(context, semantic_axioms, &witness)
                .map_err(ProofError::IntegerAffineWitness)?;
            let evidence = Proposition::Conjunction(vec![
                left_bound.conclusion.clone(),
                right_bound.conclusion.clone(),
            ]);
            let mapped = map_integer_affine_bound(&form, &evidence)
                .map_err(ProofError::IntegerAffineBoundConversion)?;
            let Proposition::IntegerMathLessOrEqual(mapped_left, mapped_right) = mapped else {
                return Err(ProofError::RulePremiseMismatch(
                    "integer exact-add mapped bound",
                ));
            };
            let (literal, lower) = match (&mapped_left, &mapped_right) {
                (IntegerMathTerm::IntegerLiteral(literal), IntegerMathTerm::Add(_, _)) => {
                    (literal, true)
                }
                (IntegerMathTerm::Add(_, _), IntegerMathTerm::IntegerLiteral(literal)) => {
                    (literal, false)
                }
                _ => {
                    return Err(ProofError::RulePremiseMismatch(
                        "integer exact-add mapped bound",
                    ));
                }
            };
            let value =
                literal
                    .as_integer_value(*scalar_type)
                    .ok_or(ProofError::RulePremiseMismatch(
                        "integer exact-add mapped literal",
                    ))?;
            let literal = ScalarTerm::integer(*scalar_type, value)
                .map_err(|_| ProofError::RulePremiseMismatch("integer exact-add mapped literal"))?;
            let expected = if lower {
                Proposition::LessOrEqual(literal, output.clone())
            } else {
                Proposition::LessOrEqual(output.clone(), literal)
            };
            if proof.conclusion != expected {
                return Err(ProofError::IntegerAffineBoundConversion(
                    IntegerAffineBoundConversionError::ConclusionMismatch,
                ));
            }
            acceptance.record_semantic_axiom(*definition_axiom, definition);
            Ok(())
        }
        ProofRule::IntegerCastBound {
            root_bound,
            witness,
        } => {
            acceptance.rules.insert(AcceptedProofRule::IntegerCastBound);
            let chain = check_integer_cast_chain_witness(context, semantic_axioms, witness)
                .map_err(ProofError::IntegerCastChainWitness)?;
            let normalized_conclusion = lower_integer_math_relation(&proof.conclusion)
                .unwrap_or_else(|| proof.conclusion.clone());
            if root_bound.conclusion == Proposition::Truth {
                let bounds = integer_cast_truth_bounds(&chain)
                    .map_err(ProofError::IntegerCastBoundConversion)?;
                if !bounds.contains(&normalized_conclusion) {
                    return Err(ProofError::IntegerCastBoundConversion(
                        IntegerCastBoundConversionError::ConclusionLiteralMismatch,
                    ));
                }
            } else {
                check_integer_cast_bound_conversion(
                    &chain,
                    &root_bound.conclusion,
                    &normalized_conclusion,
                )
                .map_err(ProofError::IntegerCastBoundConversion)?;
            }
            for &index in &witness.definition_axioms {
                let proposition = semantic_axioms
                    .get(index)
                    .ok_or(ProofError::UnknownSemanticAxiom(index))?;
                acceptance.record_semantic_axiom(index, proposition);
            }
            Ok(())
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            acceptance
                .rules
                .insert(AcceptedProofRule::IntegerCorrelatedForbiddenRoots);
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
            check_integer_correlated_forbidden_root_conversion(&checked, &proof.conclusion)
                .map_err(ProofError::IntegerCorrelatedForbiddenRootConversion)?;

            for branch in [&witness.dividend, &witness.divisor] {
                for step in &branch.steps {
                    if let Some(index) = step.literal_axiom {
                        let proposition = semantic_axioms
                            .get(index)
                            .ok_or(ProofError::UnknownSemanticAxiom(index))?;
                        acceptance.record_semantic_axiom(index, proposition);
                    }
                    let index = step.definition_axiom;
                    let proposition = semantic_axioms
                        .get(index)
                        .ok_or(ProofError::UnknownSemanticAxiom(index))?;
                    acceptance.record_semantic_axiom(index, proposition);
                }
            }
            let (lower_bound, upper_bound) = checked.bound_axioms();
            for ledger_index in [lower_bound, upper_bound] {
                let index = ledger_index
                    .checked_sub(semantic_axioms.len())
                    .ok_or(ProofError::IntegerCorrelatedForbiddenRootRequirementBoundary)?;
                let proposition = assumptions
                    .get(index)
                    .ok_or(ProofError::UnknownAssumption(index))?;
                acceptance.record_assumption(index, proposition);
            }
            Ok(())
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
}

/// Canonically embed a relation over fixed-width value/literal terms into the
/// mathematical-integer relation vocabulary. Compound machine terms are not
/// silently reinterpreted by this bridge.
pub fn lift_fixed_integer_relation(proposition: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match proposition {
        Proposition::Equal(left, right) => (0, left, right),
        Proposition::LessThan(left, right) => (1, left, right),
        Proposition::LessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
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

/// Inverse of [`lift_fixed_integer_relation`] for the canonical carrier-bound
/// shapes used by exact-cast representability.
pub fn lower_integer_math_relation(proposition: &Proposition) -> Option<Proposition> {
    let (kind, left, right) = match proposition {
        Proposition::IntegerMathEqual(left, right) => (0, left, right),
        Proposition::IntegerMathLessThan(left, right) => (1, left, right),
        Proposition::IntegerMathLessOrEqual(left, right) => (2, left, right),
        _ => return None,
    };
    let source_type = [left, right].into_iter().find_map(|term| match term {
        IntegerMathTerm::MathValue { source_type, .. } => Some(*source_type),
        _ => None,
    })?;
    let lower = |term: &IntegerMathTerm| match term {
        IntegerMathTerm::MathValue {
            source_type: actual,
            value,
        } if *actual == source_type => Some(ScalarTerm::value(
            *value,
            semantic_vocabulary::ScalarType::Integer(source_type),
        )),
        IntegerMathTerm::IntegerLiteral(literal) => {
            ScalarTerm::integer(source_type, literal.as_integer_value(source_type)?).ok()
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

fn propositions_match_under_integer_math_normalization(
    retained: &Proposition,
    requested: &Proposition,
) -> bool {
    retained == requested
        || lift_fixed_integer_relation(retained).as_ref() == Some(requested)
        || lower_integer_math_relation(retained).as_ref() == Some(requested)
}

fn lift_fixed_integer_term(term: &ScalarTerm) -> Option<IntegerMathTerm> {
    match term {
        ScalarTerm::Value {
            id,
            scalar_type: semantic_vocabulary::ScalarType::Integer(source_type),
        } if !source_type.is_address() => Some(IntegerMathTerm::MathValue {
            source_type: *source_type,
            value: *id,
        }),
        ScalarTerm::Integer { scalar_type, value } if !scalar_type.is_address() => {
            debug_assert!(matches!(
                value,
                IntegerValue::Signed(_) | IntegerValue::Unsigned(_)
            ));
            Some(IntegerMathTerm::literal(*value))
        }
        _ => None,
    }
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProofError {}

#[cfg(test)]
mod disjunction_elimination;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness};
    use semantic_vocabulary::PropositionId;

    #[test]
    fn existing_assumption_rule_checks_fixed_carrier_normalization_to_math() {
        let integer_type =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 16)
                .expect("i16");
        let value_id = semantic_vocabulary::ValueId::new(88).expect("value");
        let value = ScalarTerm::value(
            value_id,
            semantic_vocabulary::ScalarType::Integer(integer_type),
        );
        let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("zero");
        let assumption = Proposition::LessOrEqual(zero, value);
        let goal = lift_fixed_integer_relation(&assumption).expect("mathematical carrier relation");
        let context = PropositionContext::from_value_types([(
            value_id,
            semantic_vocabulary::ScalarType::Integer(integer_type),
        )])
        .expect("context");
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        assert!(check_certificate(&context, &goal, &[assumption], &[], &proof).is_ok());

        let wrong = Proposition::IntegerMathLessOrEqual(
            IntegerMathTerm::literal(IntegerValue::Signed(1)),
            IntegerMathTerm::MathValue {
                source_type: integer_type,
                value: value_id,
            },
        );
        let tampered = ProofNode {
            conclusion: wrong.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        assert!(
            check_certificate(
                &context,
                &wrong,
                &[Proposition::LessOrEqual(
                    ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).expect("zero"),
                    ScalarTerm::value(
                        value_id,
                        semantic_vocabulary::ScalarType::Integer(integer_type)
                    ),
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
                disjunct: Box::new(ProofNode {
                    conclusion: proposition,
                    rule: ProofRule::Assumption { index: assumption },
                }),
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
                vec![AcceptedPremise {
                    index: usize::from(expected == right),
                    proposition: expected,
                }]
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
                        disjunct: Box::new(ProofNode {
                            conclusion: left,
                            rule: ProofRule::Assumption { index: 0 },
                        }),
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
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = ScalarTerm::value(
            ValueId::new(1).expect("divisor"),
            ScalarType::Integer(integer),
        );
        let literal =
            |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
        let negative_two_bound = Proposition::LessOrEqual(divisor.clone(), literal(-2));
        let negative_one_bound = Proposition::LessOrEqual(divisor.clone(), literal(-1));
        let positive_bound = Proposition::LessOrEqual(literal(1), divisor.clone());
        let goal = Proposition::Disjunction(vec![negative_one_bound.clone(), positive_bound]);
        let weakened = ProofNode {
            conclusion: negative_one_bound,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(ProofNode {
                    conclusion: negative_two_bound.clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                middle_less_or_equal_right: Box::new(ProofNode {
                    conclusion: Proposition::LessOrEqual(literal(-2), literal(-1)),
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
            },
        };
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::DisjunctionIntroduction {
                disjunct: Box::new(weakened),
                index: 0,
            },
        };
        let context = PropositionContext::from_value_types([(
            ValueId::new(1).expect("divisor"),
            ScalarType::Integer(integer),
        )])
        .expect("context");
        let acceptance = accept_certificate(
            &context,
            &goal,
            &[],
            std::slice::from_ref(&negative_two_bound),
            &proof,
        )
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
            vec![AcceptedPremise {
                index: 0,
                proposition: negative_two_bound,
            }]
        );
    }

    #[test]
    fn integer_order_transitivity_requires_exact_middle_endpoints_and_relations() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let left = ScalarTerm::value(ValueId::new(1).expect("left"), ScalarType::Integer(integer));
        let literal =
            |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
        let first = Proposition::LessOrEqual(left.clone(), literal(-2));
        let context = PropositionContext::from_value_types([(
            ValueId::new(1).expect("left"),
            ScalarType::Integer(integer),
        )])
        .expect("context");
        let proof = |second: Proposition, conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(ProofNode {
                    conclusion: first.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                middle_less_or_equal_right: Box::new(ProofNode {
                    conclusion: second,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
            },
        };

        let expected = Proposition::LessOrEqual(left.clone(), literal(-1));
        assert_eq!(
            check_certificate(
                &context,
                &expected,
                std::slice::from_ref(&first),
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
                std::slice::from_ref(&first),
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
                std::slice::from_ref(&first),
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
                std::slice::from_ref(&first),
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
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = ScalarTerm::value(
            ValueId::new(1).expect("divisor"),
            ScalarType::Integer(integer),
        );
        let literal =
            |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
        let context = PropositionContext::from_value_types([(
            ValueId::new(1).expect("divisor"),
            ScalarType::Integer(integer),
        )])
        .expect("context");

        let positive_literal = Proposition::LessOrEqual(literal(1), literal(5));
        let positive_equality = Proposition::Equal(divisor.clone(), literal(5));
        let positive = Proposition::LessOrEqual(literal(1), divisor.clone());
        let positive_proof = ProofNode {
            conclusion: positive.clone(),
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: positive_literal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(ProofNode {
                    conclusion: positive_equality.clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                endpoint: 1,
            },
        };
        let accepted = accept_certificate(
            &context,
            &positive,
            &[],
            std::slice::from_ref(&positive_equality),
            &positive_proof,
        )
        .expect("a literal equality transports the right endpoint");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Primitive,
                AcceptedProofRule::SemanticAxiom,
                AcceptedProofRule::IntegerLessOrEqualSubstitution,
            ]
        );
        assert_eq!(
            accepted.semantic_axioms,
            vec![AcceptedPremise {
                index: 0,
                proposition: positive_equality,
            }]
        );

        let negative_literal = Proposition::LessOrEqual(literal(-2), literal(-1));
        let reverse_negative_equality = Proposition::Equal(literal(-2), divisor.clone());
        let negative = Proposition::LessOrEqual(divisor, literal(-1));
        let negative_proof = ProofNode {
            conclusion: negative.clone(),
            rule: ProofRule::IntegerLessOrEqualSubstitution {
                relation: Box::new(ProofNode {
                    conclusion: negative_literal,
                    rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
                }),
                equality: Box::new(ProofNode {
                    conclusion: reverse_negative_equality.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
                endpoint: 0,
            },
        };
        let accepted = accept_certificate(
            &context,
            &negative,
            std::slice::from_ref(&reverse_negative_equality),
            &[],
            &negative_proof,
        )
        .expect("a reverse equality transports the left endpoint");
        assert_eq!(
            accepted.rules,
            vec![
                AcceptedProofRule::Primitive,
                AcceptedProofRule::Assumption,
                AcceptedProofRule::IntegerLessOrEqualSubstitution,
            ]
        );
    }

    #[test]
    fn integer_order_substitution_rejects_every_shape_and_endpoint_mismatch() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let divisor = ScalarTerm::value(
            ValueId::new(1).expect("divisor"),
            ScalarType::Integer(integer),
        );
        let literal =
            |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
        let context = PropositionContext::from_value_types([(
            ValueId::new(1).expect("divisor"),
            ScalarType::Integer(integer),
        )])
        .expect("context");
        let relation = Proposition::LessOrEqual(literal(1), literal(5));
        let equality = Proposition::Equal(literal(5), divisor.clone());
        let conclusion = Proposition::LessOrEqual(literal(1), divisor.clone());
        let child = |conclusion: Proposition, rule: ProofRule| ProofNode { conclusion, rule };
        let proof = |relation: ProofNode,
                     equality: ProofNode,
                     endpoint: usize,
                     conclusion: Proposition| ProofNode {
            conclusion,
            rule: ProofRule::IntegerLessOrEqualSubstitution {
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
                std::slice::from_ref(&equality),
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
                std::slice::from_ref(&equality),
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
                std::slice::from_ref(&unrelated),
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
                std::slice::from_ref(&equality),
            ),
            Err(ProofError::RulePremiseMismatch(
                "integer <= substitution relation"
            )),
        );
        assert_eq!(
            check(
                &proof(relation_child(), truth.clone(), 1, conclusion.clone()),
                &conclusion,
                &[],
            ),
            Err(ProofError::RulePremiseMismatch(
                "integer <= substitution equality"
            )),
        );
        assert_eq!(
            check(
                &proof(relation_child(), equality_child(), 1, Proposition::Truth),
                &Proposition::Truth,
                std::slice::from_ref(&equality),
            ),
            Err(ProofError::RuleConclusionMismatch(
                "integer <= substitution"
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
                std::slice::from_ref(&equality),
            ),
            Err(ProofError::UnknownSemanticAxiom(1)),
        );
    }

    #[test]
    fn integer_affine_bound_checks_root_proof_normalization_and_exact_custody() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

        let integer = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root = ScalarTerm::value(ValueId::new(1).expect("root"), ScalarType::Integer(integer));
        let target = ScalarTerm::value(
            ValueId::new(2).expect("target"),
            ScalarType::Integer(integer),
        );
        let sibling = ScalarTerm::value(
            ValueId::new(3).expect("sibling"),
            ScalarType::Integer(integer),
        );
        let literal =
            |value| ScalarTerm::integer(integer, IntegerValue::Signed(value)).expect("i8 literal");
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).expect("root"), ScalarType::Integer(integer)),
            (
                ValueId::new(2).expect("target"),
                ScalarType::Integer(integer),
            ),
            (
                ValueId::new(3).expect("sibling"),
                ScalarType::Integer(integer),
            ),
        ])
        .expect("context");
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
                    root_bound: Box::new(ProofNode {
                        conclusion: root_bound.clone(),
                        rule: ProofRule::Assumption { index: 0 },
                    }),
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
            std::slice::from_ref(&root_bound),
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
                AcceptedPremise {
                    index: 0,
                    proposition: landing,
                },
                AcceptedPremise {
                    index: 1,
                    proposition: definition.clone(),
                },
            ]
        );
        assert_eq!(
            check_certificate(
                &context,
                &conclusion,
                std::slice::from_ref(&root_bound),
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
                std::slice::from_ref(&root_bound),
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
                root_bound: Box::new(ProofNode {
                    conclusion: Proposition::Truth,
                    rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
                }),
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
                std::slice::from_ref(&root_bound),
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
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let root = ScalarTerm::value(
            ValueId::new(1).expect("root"),
            ScalarType::Integer(i16_type),
        );
        let target = ScalarTerm::value(
            ValueId::new(2).expect("target"),
            ScalarType::Integer(i8_type),
        );
        let middle = ScalarTerm::value(
            ValueId::new(3).expect("middle"),
            ScalarType::Integer(u16_type),
        );
        let context = PropositionContext::from_value_types([
            (
                ValueId::new(1).expect("root"),
                ScalarType::Integer(i16_type),
            ),
            (
                ValueId::new(2).expect("target"),
                ScalarType::Integer(i8_type),
            ),
            (
                ValueId::new(3).expect("middle"),
                ScalarType::Integer(u16_type),
            ),
        ])
        .expect("context");
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
                root_bound: Box::new(ProofNode {
                    conclusion: root_bound.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
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
            std::slice::from_ref(&root_bound),
            std::slice::from_ref(&definition),
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
            vec![AcceptedPremise {
                index: 0,
                proposition: definition.clone(),
            }]
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
            std::slice::from_ref(&root_bound),
            &multi_axioms,
            &proof(vec![0, 1], conclusion.clone()),
        )
        .expect("the complete contiguous cast word maps the root bound");
        assert_eq!(
            accepted.semantic_axioms,
            vec![
                AcceptedPremise {
                    index: 0,
                    proposition: first_definition,
                },
                AcceptedPremise {
                    index: 1,
                    proposition: second_definition,
                },
            ],
        );
        assert_eq!(
            check_certificate(
                &context,
                &conclusion,
                std::slice::from_ref(&root_bound),
                std::slice::from_ref(&definition),
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
                std::slice::from_ref(&root_bound),
                std::slice::from_ref(&definition),
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
                body: Box::new(ProofNode {
                    conclusion: proposition,
                    rule: ProofRule::Assumption { index: 0 },
                }),
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
    fn accepted_trust_closure_binds_the_exact_cited_premise() {
        let cited = Proposition::Atom(PropositionId::new(1).expect("cited atom"));
        let replacement = Proposition::Atom(PropositionId::new(2).expect("replacement atom"));
        let proof = ProofNode {
            conclusion: cited.clone(),
            rule: ProofRule::Assumption { index: 0 },
        };
        let accepted = accept_certificate(
            &PropositionContext::default(),
            &cited,
            std::slice::from_ref(&cited),
            &[],
            &proof,
        )
        .expect("exact cited premise");
        assert_eq!(
            accepted.assumptions,
            vec![AcceptedPremise {
                index: 0,
                proposition: cited.clone(),
            }]
        );
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
                body: Box::new(ProofNode {
                    conclusion: proposition.clone(),
                    rule: ProofRule::Assumption { index: 0 },
                }),
            },
        };
        let goal = Proposition::Conjunction(vec![implication(&first), implication(&second)]);
        let proof = ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ConjunctionIntroduction(vec![branch(&first), branch(&second)]),
        };
        let accepted = accept_certificate(&PropositionContext::default(), &goal, &[], &[], &proof)
            .expect("both nested implication premises");
        assert!(accepted.assumptions.is_empty());
    }

    #[test]
    fn semantic_equalities_compose_only_through_the_same_middle_term() {
        use semantic_vocabulary::{
            IntegerSign, IntegerType, IntegerValue, ScalarTerm, ScalarType, ValueId,
        };

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
                left_equals_middle: Box::new(ProofNode {
                    conclusion: axioms[0].clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: axioms[1].clone(),
                    rule: ProofRule::SemanticAxiom { index: 1 },
                }),
            },
        };
        let context = PropositionContext::from_value_types([
            (ValueId::new(1).expect("a"), ScalarType::Integer(integer)),
            (ValueId::new(2).expect("b"), ScalarType::Integer(integer)),
        ])
        .expect("context");
        let accepted =
            accept_certificate(&context, &goal, &[], &axioms, &proof).expect("transitive equality");
        assert_eq!(
            accepted.semantic_axioms,
            vec![
                AcceptedPremise {
                    index: 0,
                    proposition: axioms[0].clone(),
                },
                AcceptedPremise {
                    index: 1,
                    proposition: axioms[1].clone(),
                },
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
                left_equals_middle: Box::new(ProofNode {
                    conclusion: axioms[0].clone(),
                    rule: ProofRule::SemanticAxiom { index: 0 },
                }),
                middle_equals_right: Box::new(ProofNode {
                    conclusion: axioms[1].clone(),
                    rule: ProofRule::SemanticAxiom { index: 1 },
                }),
            },
        };
        let context = PropositionContext::from_value_types_and_places(
            [],
            [(
                root,
                StructuralPlaceKind::Parameter {
                    position: 0,
                    is_self: false,
                },
            )],
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
        let integer_type =
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 8)
                .expect("signed carrier");
        let scalar_type = semantic_vocabulary::ScalarType::Integer(integer_type);
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
                        steps: vec![CorrelatedAffineStepWitness {
                            definition_axiom: 0,
                            literal_axiom: None,
                        }],
                    },
                    divisor: CorrelatedAffineBranchWitness {
                        root: root.clone(),
                        target: divisor,
                        steps: vec![
                            CorrelatedAffineStepWitness {
                                definition_axiom: 1,
                                literal_axiom: None,
                            },
                            CorrelatedAffineStepWitness {
                                definition_axiom: 2,
                                literal_axiom: None,
                            },
                        ],
                    },
                    definition_axiom_count: 3,
                    lower_bound_axiom: 3,
                    upper_bound_axiom: 4,
                    conclusion: Proposition::Conjunction(requirements.clone()),
                },
            },
        };
        let context = PropositionContext::from_value_types((100..=103).map(|raw| {
            (
                ValueId::new(raw).expect("value"),
                semantic_vocabulary::ScalarType::Integer(integer_type),
            )
        }))
        .expect("context");
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

        let forged = ProofNode {
            conclusion: Proposition::Truth,
            rule: proof.rule,
        };
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
}
