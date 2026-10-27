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

use semantic_vocabulary::{IntegerMathTerm, Proposition, PropositionContext, ScalarTerm, ValueId};

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
    IntegerAddOrder,
    IntegerSubtractAntitone,
    IntegerLessOrEqualTransitivity,
    IntegerStrictOrderTransitivity,
    IntegerOrderSubstitution,
    IntegerMathOrderTranslation,
    IntegerOrderAntisymmetry,
    IntegerAffineBound,
    IntegerExactAddDefinitionBound,
    IntegerExactSubtractDefinitionBound,
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

/// Accepted premises in first-recorded order, beside an index-keyed map of
/// the positions each index occupies.
///
/// The admitted predicate is unchanged -- a citation is new unless some
/// recorded premise shares both its index and its proposition -- but only
/// rows that already share the index can satisfy the first half, so the
/// proposition comparison runs against those rows alone. A certificate
/// citing an N-row roster costs N structural comparisons in the common case
/// of one proposition per index, not N(N-1)/2.
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
        // Admission reserved the stack before the bounded traversal. Kernel
        // recursion has returned; collect the exact closure on that same
        // stack instead of spawning a second worker per accepted certificate.
        Ok(denoted) => Ok(MathematicalCoreDecision::Judged(
            denoted.receipt_on_current_thread(),
        )),
        Err(BoundedDenotationError::Unsupported(family)) => {
            Ok(MathematicalCoreDecision::Refused(family))
        }
        Err(error) => Err(ProofError::MathematicalCore(Box::new(error))),
    }
}

pub(crate) mod add_order;
pub(crate) mod equality_rules;
pub(crate) mod integer_bound_rules;
pub(crate) mod integer_math_normalization;
pub(crate) mod integer_order_rules;
pub(crate) mod order_discreteness;
mod propositional_rules;
pub(crate) mod strict_order_transitivity;
pub(crate) mod subtract_antitone;
pub(crate) mod subtract_order;
mod traversal;

pub use integer_math_normalization::{
    lift_exact_integer_order, lift_fixed_integer_relation, lower_integer_math_relation,
};
use traversal::check_node;

/// The rule-check dispatch preamble. The scoped traversal has already
/// selected the rule's family; the family's check re-matches the variant it
/// was called for — it never receives another — and records the accepted
/// rule on the acceptance in the same breath. Written once here so each of
/// the rule families does not restate the same guard, refusal text and
/// acceptance row per rule.
macro_rules! dispatched_rule {
    ($proof:expr, $acceptance:expr, $pattern:pat => $accepted:ident) => {
        let $pattern = &$proof.rule else {
            unreachable!(concat!("dispatched ", stringify!($accepted)))
        };
        $acceptance.rules.insert(AcceptedProofRule::$accepted);
    };
}
pub(crate) use dispatched_rule;

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
        ProofRule::IntegerAddOrder { .. } => {
            integer_order_rules::check_integer_add_order(proof, acceptance)
        }
        ProofRule::IntegerSubtractAntitone { .. } => {
            integer_order_rules::check_integer_subtract_antitone(proof, acceptance)
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
    /// The bounded rules accepted the certificate but the common
    /// mathematical core did not: the kernel rejected the denoted
    /// judgment, the elaboration bound was exceeded, or the denotation's
    /// structural re-derivation disagreed with the bounded traversal.
    MathematicalCore(Box<BoundedDenotationError>),
}

crate::debug_display!(ProofError);

impl std::error::Error for ProofError {}

#[cfg(test)]
mod disjunction_elimination;

#[cfg(test)]
mod order_substitution_tests;

#[cfg(test)]
mod tests;

/// Build a leaf assumption `ProofNode` for tests: the cited premise as its own
/// conclusion.
#[cfg(test)]
pub(crate) fn assumption_proof(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::Assumption { index },
    }
}

/// Build a leaf semantic-axiom `ProofNode` for tests.
#[cfg(test)]
pub(crate) fn semantic_axiom_proof(conclusion: Proposition, index: usize) -> ProofNode {
    ProofNode {
        conclusion,
        rule: ProofRule::SemanticAxiom { index },
    }
}

/// The scalar `LessThan`/`LessOrEqual` relation spelled as
/// `(strict, left, right)`, or `None` when the proposition is no
/// scalar order — shared by the integer-order rule modules.
pub(crate) fn scalar_order(proposition: &Proposition) -> Option<(bool, &ScalarTerm, &ScalarTerm)> {
    match proposition {
        Proposition::LessThan(left, right) => Some((true, left, right)),
        Proposition::LessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}

/// The mathematical-integer `IntegerMathLessThan`/`IntegerMathLessOrEqual`
/// relation spelled the same way.
pub(crate) fn mathematical_order(
    proposition: &Proposition,
) -> Option<(bool, &IntegerMathTerm, &IntegerMathTerm)> {
    match proposition {
        Proposition::IntegerMathLessThan(left, right) => Some((true, left, right)),
        Proposition::IntegerMathLessOrEqual(left, right) => Some((false, left, right)),
        _ => None,
    }
}
