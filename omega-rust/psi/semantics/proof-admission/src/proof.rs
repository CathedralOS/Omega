//! Certificate checking. `check_certificate` walks a proof in scoped
//! postorder (`traversal`) and `check_node_locally` hands each rule to its
//! family: `propositional_rules`, `equality_rules`, `integer_order_rules`
//! and `integer_bound_rules`. `AcceptanceBuilder` records which rules and
//! premises a certificate used; `integer_math_normalization` bridges
//! fixed-width and mathematical integer relations.
//!
//! Acceptance then consults the common mathematical core: the certificate
//! is denoted into the core's judgment `Γ ⊢ t : ⟦goal⟧` and
//! `verify_mathematical_certificate` re-decides it. That decision is
//! recorded on the acceptance as [`MathematicalCoreDecision`] — the kernel
//! judged the certificate, or refused a construction the denotation cannot
//! cross and the bounded rules stand alone. A covered certificate the
//! kernel rejects is rejected here too: two checkers reading one
//! certificate must agree, and the kernel's rejection is never overridden
//! by the rule labels.

use std::collections::BTreeSet;
pub use terminal_psi::{ProofNode, ProofRule};

use semantic_vocabulary::{Proposition, PropositionContext, ValueId};

use crate::mathematical_core::{
    BoundedDenotationError, Budget, run_on_verification_stack,
    verify_bounded_certificate_on_current_thread,
};
use crate::{
    IntegerAffineBoundConversionError, IntegerAffineWitnessError, IntegerCastBoundConversionError,
    IntegerCastChainWitnessError, IntegerCorrelatedForbiddenRootConversionError,
    IntegerCorrelatedForbiddenRootWitnessError, KernelError,
};

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
    EqualitySymmetry,
    PredicateDenotation,
    ValueEqualityTransport,
    IntegerOrderWeakening,
    IntegerOrderDiscreteness,
    IntegerSubtractOrder,
    IntegerLessOrEqualTransitivity,
    IntegerStrictOrderTransitivity,
    IntegerOrderSubstitution,
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
    /// What the common mathematical core decided about this certificate.
    pub mathematical_core: MathematicalCoreDecision,
}

/// The common mathematical core's part in one accepted certificate.
///
/// This is the receiving checker's own record, never a producer claim: it
/// is written by `accept_certificate` after the kernel ran (or refused),
/// and a receiver reading `Refused` knows the bounded rules alone decided
/// that certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathematicalCoreDecision {
    /// The certificate denoted into the core and the kernel re-decided the
    /// judgment `Γ ⊢ t : ⟦goal⟧`; the receipt measures that judgment.
    Judged(MathematicalJudgmentReceipt),
    /// A valid certificate construction the denotation does not cover —
    /// named by the payload — so the kernel decided nothing and the
    /// bounded rules stand alone. Refusal is never a rejection.
    Refused(&'static str),
}

/// Measurements of one judgment the kernel decided. These are the
/// storage and closure figures the board item asks to be measured rather
/// than assumed; none of them is authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MathematicalJudgmentReceipt {
    /// Declarations in the judgment's signature: interned atoms, scalar
    /// carriers, scalar terms and bounded primitive decisions.
    pub declarations: u32,
    /// Assumption constants the judgment's exact closure commits to,
    /// computed over the stored signature, never from what conversion
    /// unfolded.
    pub assumption_closure: u32,
    /// Premises bound in the judgment's context: the ambient assumption
    /// roster followed by the semantic-axiom roster.
    pub context_depth: u32,
    /// Arena slots in use once the kernel has checked the judgment: the
    /// elaborated terms plus the working terms checking inserted, so this
    /// is a checking-cost measurement rather than a term size.
    pub arena_slots: u32,
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

    fn finish(self, mathematical_core: MathematicalCoreDecision) -> CertificateAcceptance {
        CertificateAcceptance {
            rules: self.rules.into_iter().collect(),
            assumptions: self.assumptions,
            semantic_axioms: self.semantic_axioms,
            mathematical_core,
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
    // The bounded traversal recurses over the certificate's proof tree and
    // the mathematical-core route recurses over the elaborated judgment;
    // producer certificates nest far deeper than the default thread stack
    // admits. Run the whole admission on the verification stack so depth
    // is a resource limit, never a crash.
    run_on_verification_stack(|| {
        accept_certificate_on_current_thread(
            context,
            goal,
            assumptions,
            semantic_axioms,
            machine_parameter_values,
            proof,
        )
    })
}

/// [`accept_certificate_with_machine_parameters`] on the caller's stack.
fn accept_certificate_on_current_thread(
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

/// Denote a certificate the bounded rules already accepted into the common
/// mathematical core and let the kernel re-decide it.
///
/// The bounded traversal above has established every rule's structural
/// premise/conclusion relation, so the only outcomes here are the
/// kernel's own: `Judged` when the denotation covers the certificate and
/// `check_type` accepts the elaborated judgment, `Refused` when the
/// certificate uses a construction the denotation cannot cross —
/// currently none of the families the bounded checker decides. Any
/// other error is a disagreement between the two checkers — a kernel
/// rejection, an elaboration bound, or a structural check the denotation
/// re-derives differently — and rejects the certificate rather than
/// letting the rule labels outvote the kernel.
fn re_decide_in_mathematical_core(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
    proof: &ProofNode,
) -> Result<MathematicalCoreDecision, ProofError> {
    match verify_bounded_certificate_on_current_thread(
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

pub(crate) mod equality_rules;
pub(crate) mod integer_bound_rules;
pub(crate) mod integer_math_normalization;
pub(crate) mod integer_order_rules;
pub(crate) mod order_discreteness;
mod propositional_rules;
pub(crate) mod strict_order_transitivity;
pub(crate) mod subtract_order;
mod traversal;

pub use integer_math_normalization::{lift_fixed_integer_relation, lower_integer_math_relation};
use traversal::check_node;

/// What every rule check reads: the proposition context, the certificate's
/// assumption and semantic-axiom rosters, and the machine parameter values.
#[derive(Clone, Copy)]
struct RuleScope<'a> {
    context: &'a PropositionContext,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    machine_parameter_values: &'a BTreeSet<ValueId>,
}

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
    let scope = RuleScope {
        context,
        assumptions,
        semantic_axioms,
        machine_parameter_values,
    };
    match &proof.rule {
        ProofRule::Primitive(_) => propositional_rules::check_primitive(&scope, proof, acceptance),
        ProofRule::SemanticAxiom { .. } => {
            propositional_rules::check_semantic_axiom(&scope, proof, acceptance)
        }
        ProofRule::Assumption { .. } => {
            propositional_rules::check_assumption(&scope, proof, acceptance)
        }
        ProofRule::ConjunctionIntroduction(_) => {
            propositional_rules::check_conjunction_introduction(proof, acceptance)
        }
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
        ProofRule::EqualitySymmetry { .. } => {
            equality_rules::check_equality_symmetry(proof, acceptance)
        }
        ProofRule::PredicateDenotation { .. } => {
            equality_rules::check_predicate_denotation(&scope, proof, acceptance)
        }
        ProofRule::ValueEqualityTransport { .. } => {
            equality_rules::check_value_equality_transport(&scope, proof, acceptance)
        }
        ProofRule::EqualityTransitivity { .. } => {
            equality_rules::check_equality_transitivity(proof, acceptance)
        }
        ProofRule::IntegerSubtractOrder { .. } => {
            integer_order_rules::check_integer_subtract_order(proof, acceptance)
        }
        ProofRule::IntegerOrderDiscreteness { .. } => {
            integer_order_rules::check_integer_order_discreteness(proof, acceptance)
        }
        ProofRule::IntegerOrderWeakening { .. } => {
            integer_order_rules::check_integer_order_weakening(proof, acceptance)
        }
        ProofRule::IntegerLessOrEqualTransitivity { .. } => {
            integer_order_rules::check_integer_less_or_equal_transitivity(proof, acceptance)
        }
        ProofRule::IntegerStrictOrderTransitivity { .. } => {
            integer_order_rules::check_integer_strict_order_transitivity(proof, acceptance)
        }
        ProofRule::IntegerOrderSubstitution { .. } => {
            integer_order_rules::check_integer_order_substitution(proof, acceptance)
        }
        ProofRule::IntegerAffineBound { .. } => {
            integer_bound_rules::check_integer_affine_bound(&scope, proof, acceptance)
        }
        ProofRule::IntegerExactAddDefinitionBound { .. } => {
            integer_bound_rules::check_integer_exact_add_definition_bound(&scope, proof, acceptance)
        }
        ProofRule::IntegerCastBound { .. } => {
            integer_bound_rules::check_integer_cast_bound(&scope, proof, acceptance)
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
    /// The bounded rules accepted the certificate but the common
    /// mathematical core did not: the kernel rejected the denoted
    /// judgment, the elaboration bound was exceeded, or the denotation's
    /// structural re-derivation disagreed with the bounded traversal.
    MathematicalCore(Box<BoundedDenotationError>),
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
mod order_substitution_tests;

#[cfg(test)]
mod tests {
    use semantic_vocabulary::{IntegerMathTerm, IntegerValue, ScalarTerm};

    use super::{
        AcceptedPremise, AcceptedProofRule, BTreeSet, IntegerAffineBoundConversionError,
        IntegerAffineWitnessError, IntegerCastBoundConversionError, IntegerCastChainWitnessError,
        IntegerCorrelatedForbiddenRootConversionError, IntegerCorrelatedForbiddenRootWitnessError,
        ProofError, ProofNode, ProofRule, Proposition, PropositionContext, ValueId,
        accept_certificate, accept_certificate_with_machine_parameters, check_certificate,
        lift_fixed_integer_relation,
    };
    use crate::IntegerAffineWitness;
    use crate::{
        CorrelatedAffineBranchWitness, CorrelatedAffineStepWitness, IntegerCastChainWitness,
        IntegerCorrelatedForbiddenRootWitness, PrimitiveJudgment,
    };
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
            rule: ProofRule::IntegerOrderSubstitution {
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
                AcceptedProofRule::IntegerOrderSubstitution,
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
            rule: ProofRule::IntegerOrderSubstitution {
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
                AcceptedProofRule::IntegerOrderSubstitution,
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
                std::slice::from_ref(&equality),
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
    fn recursive_component_preserves_exact_machine_parameter_scope() {
        use crate::{
            AdmissionProfile, CertificateEnvelope, CertificateObligation, EvidenceRoute,
            Obligation, ObligationClass, RecursiveComponentCertificate,
            RecursiveComponentObligation, RecursiveEdgeCertificate, RecursiveEdgeObligation,
            verify_recursive_component, verify_recursive_component_with_machine_parameters,
        };
        use semantic_vocabulary::{BlockId, EvidenceIdentity, ObligationId, RankingRelationId};

        let (context, axioms, requirements, parameters, proof) = correlated_division_fixture();
        let member = BlockId::new(1).unwrap();
        let relation = RankingRelationId::new(1).unwrap();
        let obligation = ObligationId::new(1).unwrap();
        let question = CertificateObligation {
            obligation: Obligation {
                id: obligation,
                proposition: proof.conclusion.clone(),
                class: ObligationClass::Derivable,
            },
            assumptions: requirements,
            semantic_axioms: axioms,
        };
        // The generic envelope checks caller-reconstructed propositions. The
        // fixture exercises parameter-only proof scope, not a ranking theory.
        let component = RecursiveComponentObligation {
            members: vec![member],
            ranking_relation: Some(relation),
            well_foundedness: question.clone(),
            edges: vec![RecursiveEdgeObligation {
                caller: member,
                callee: member,
                decrease: question,
            }],
        };
        let route = EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: EvidenceIdentity::new(1).unwrap(),
            proof_system_marker: terminal_psi::ProofSystemMarker::CURRENT,
            proof,
        });
        let certificate = RecursiveComponentCertificate {
            identity: EvidenceIdentity::new(1).unwrap(),
            ranking_relation: relation,
            well_foundedness: route.clone(),
            edges: vec![RecursiveEdgeCertificate {
                obligation,
                evidence: route,
            }],
        };
        let profile = AdmissionProfile::default();
        verify_recursive_component_with_machine_parameters(
            &context,
            &component,
            &parameters,
            certificate.clone(),
            &profile,
        )
        .expect("actual invocation parameter supports the reconstructed proof");
        assert!(
            verify_recursive_component(&context, &component, certificate.clone(), &profile,)
                .is_err()
        );
        assert!(
            verify_recursive_component_with_machine_parameters(
                &context,
                &component,
                &BTreeSet::from([ValueId::new(101).unwrap()]),
                certificate,
                &profile,
            )
            .is_err(),
            "a loop-local value cannot replace the invocation parameter"
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
