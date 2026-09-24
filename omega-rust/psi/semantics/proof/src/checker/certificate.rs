//! Untrusted certificates for bounded integer value legs, independently
//! re-decided by the proof-admission kernel.
//!
//! A bounded leg used to stand on this crate's range derivation and a bare
//! interval comparison: derive the value's interval, test it against the
//! declared target, and report. That verdict was trusted on its own say-so.
//! For the covered shapes here the checker instead emits an explicit
//! [`ProofNode`] certificate and asks [`verify_obligation`] -- the same
//! admission kernel that decides terminal-Psi evidence -- to re-decide it.
//! The derivation is evidence, not authority: a certificate that mis-states a
//! premise or a step is rejected by the kernel, and a leg the producer cannot
//! certify keeps the ordinary trusted verdict while coverage grows.
//!
//! Covered shapes, mirroring the exact decision order of the ordinary
//! derivation:
//!
//! * a wholly anonymous arithmetic expression that the engine lands at the
//!   destination primitive is a closed term -- the producer encodes the
//!   expression, requires the engine's own landing to agree with the term's
//!   mathematical value, and lets the kernel re-evaluate the arithmetic
//!   (call and transition arguments only; the other families never evaluate
//!   anonymous expressions);
//! * an `Integer` literal is a closed mathematical term, so each endpoint
//!   claim is a [`PrimitiveJudgment::ClosedIntegerRelation`] the kernel
//!   re-evaluates (every bounded family);
//! * a value whose declared `IntegerRange` constraints already fit the target
//!   reads as one opaque mathematical atom whose declared interval enters as
//!   an explicit premise pair, and the claim is a checked `<=` transitivity
//!   chain from the target's endpoints through the declared bounds;
//! * a guarded transition argument cites the same opaque atom plus the arm's
//!   guard conjuncts and refuted prior exit guards as premises -- the guard's
//!   `place OP literal` facts restated as exact `<=` relations -- so the
//!   narrowing the trusted guard arithmetic computed is re-derived by the
//!   kernel as transitivity chains from declared/guard premises; and
//! * a `place +- K` guarded argument reads through its operand's atom: the
//!   place's declared and guard-narrowed bounds enter as scalar `<=`
//!   premises, and the kernel's `IntegerAffineBound` rule re-derives each
//!   mapped endpoint under an independently checked `IntegerAffineWitness`.
//!
//! * a state return's joined arrival bound enters as the premise pair when
//!   it already reaches the target -- the arrival answer owns the verdict,
//!   so a bound that misses keeps the trusted failure instead of falling
//!   through to the declared legs;
//! * a state return whose declared interval alone misses the target cites
//!   the surviving `requires` contract conditions' `subject OP literal`
//!   facts on the return atom beside the declared pair -- the same facts
//!   `return_arrival` applies under the same stability gates; and
//! * a guarded transition argument that still misses after the direct and
//!   refold legs cites the arrival-rescue bound as its premise pair, exactly
//!   where the trusted derivation consults that query.
//!
//! Dependent bounds, sibling-length atoms, guard-narrowed assignments,
//! `K - place` and non-literal-operand refolds, contract operand folds on
//! returns, non-literal initializers (the ordinary derivation's only verdict
//! for them is failure -- there is no proved range to certify), non-exact
//! arithmetic, anonymous-landed arguments, guard point exclusions (the
//! kernel has no disequality premise to carry `x != K`), and the floating
//! legs are not covered; those obligations keep the existing derivation.

use arena::HandleSpan;
use numerics::bignum::BigInt;
use proof_admission::IntegerAffineWitness;
use proof_admission::{
    AcceptedFact, AdmissionProfile, CertificateEnvelope, EvidenceError, EvidenceRoute, Obligation,
    ObligationClass, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, IntegerValue,
    ObligationId, Proposition, PropositionContext, ScalarTerm, ScalarType, ValueId,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::checker::AssignmentRangeContext;
use crate::checker::derivation_cache::DerivationConsultation;
use crate::checker::guards::{expressions_equivalent_for_proof, unwrap_true_guard_condition};
use crate::checker::integer_ranges::{
    integer_literal_handle, integer_range_for_return_value, integer_range_from_constraints,
    type_constraints,
};
use crate::checker::measurement::ProofPlanMeasurements;
use crate::checker::requires_conditions;
use crate::obligations::{
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, IntegerRange,
    ProofConstraint, ProofPlan,
};

/// Outcome of the certificate route for one bounded integer value leg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateVerdict {
    /// The admission kernel accepted the emitted certificate; the leg is
    /// discharged on independently checked evidence.
    Certified,
    /// A certificate was emitted but the kernel rejected it; the leg fails
    /// exactly as an unprovable range does.
    Rejected,
    /// No certificate route covers this value shape; the ordinary range
    /// derivation still decides the leg.
    Uncovered,
}

/// The kernel-facing package for one bounded integer value leg: the exact
/// proposition, its declared premises, and the certificate that must derive
/// the conclusion from them. Producers construct it; `verify` asks the
/// admission kernel whether it stands.
#[derive(Debug)]
pub(crate) struct BoundedValueCertificate {
    context: PropositionContext,
    obligation: Obligation,
    assumptions: Vec<Proposition>,
    envelope: CertificateEnvelope,
}

impl BoundedValueCertificate {
    /// Independently re-decide this certificate through the admission kernel.
    /// Producer success never counts; only the kernel's verdict does.
    pub(crate) fn verify(&self) -> Result<AcceptedFact, EvidenceError> {
        verify_obligation(
            &self.context,
            &self.obligation,
            &self.assumptions,
            &[],
            EvidenceRoute::CertificateDerived(self.envelope.clone()),
            &AdmissionProfile::default(),
        )
    }
}

/// Certificate route for a bounded value leg whose value expression is the
/// obligation's own `value`/`argument`: assignments, initializers, call and
/// transition arguments. `anonymous` selects the anonymous-expression leg the
/// call and transition derivations run before the literal and declared legs.
pub(crate) fn bounded_integer_value_verdict(
    proof_plan: &ProofPlan,
    value: ExpressionHandle,
    value_constraints: HandleSpan<ProofConstraint>,
    base_type: TypeReferenceHandle,
    target: &IntegerRange,
    seed: u64,
    anonymous: bool,
    measurements: &mut ProofPlanMeasurements,
    mut derivations: Option<&mut DerivationConsultation<'_>>,
) -> CertificateVerdict {
    // A retained derivation under this obligation's semantic key is evidence
    // the kernel re-decides; an accepted candidate discharges the leg
    // without re-running the producer.
    if let Some(consultation) = derivations.as_deref_mut()
        && let Some(fact) = consultation.recheck()
    {
        measurements.record_accepted_fact(&fact);
        measurements.record_certificate_verdict(CertificateVerdict::Certified);
        return CertificateVerdict::Certified;
    }
    let Some(certificate) = bounded_integer_value(
        proof_plan,
        value,
        value_constraints,
        base_type,
        target,
        seed,
        anonymous,
    ) else {
        measurements.record_certificate_verdict(CertificateVerdict::Uncovered);
        return CertificateVerdict::Uncovered;
    };
    measurements.record_emitted_certificate(&certificate.envelope.proof);
    let verdict = match certificate.verify() {
        Ok(fact) => {
            measurements.record_accepted_fact(&fact);
            CertificateVerdict::Certified
        }
        Err(_) => CertificateVerdict::Rejected,
    };
    measurements.record_certificate_verdict(verdict);
    if verdict == CertificateVerdict::Certified
        && let Some(consultation) = derivations
    {
        consultation.retain(certificate);
    }
    verdict
}

/// Certificate route for a bounded state return. Three legs mirror the
/// trusted `return_arrival` decision order: a computed arrival bound enters
/// as the premise pair when it reaches the target, the declared-or-literal
/// legs follow, and the `requires`-contract leg cites the surviving authored
/// conditions' literal facts on the return atom. Malformed obligations, a
/// missed arrival bound, contract operand folds and every other shape stay
/// uncovered and keep the ordinary derivation.
pub(crate) fn state_return_integer_verdict<'program>(
    proof_plan: &ProofPlan<'program>,
    obligation: &BoundedStateReturnObligation,
    context: &AssignmentRangeContext<'program>,
    target: &IntegerRange,
    seed: u64,
    measurements: &mut ProofPlanMeasurements,
    mut derivations: Option<&mut DerivationConsultation<'_>>,
) -> CertificateVerdict {
    if let Some(consultation) = derivations.as_deref_mut()
        && let Some(fact) = consultation.recheck()
    {
        measurements.record_accepted_fact(&fact);
        measurements.record_certificate_verdict(CertificateVerdict::Certified);
        return CertificateVerdict::Certified;
    }
    let Some(certificate) = state_return_certificate(proof_plan, obligation, context, target, seed)
    else {
        measurements.record_certificate_verdict(CertificateVerdict::Uncovered);
        return CertificateVerdict::Uncovered;
    };
    measurements.record_emitted_certificate(&certificate.envelope.proof);
    let verdict = match certificate.verify() {
        Ok(fact) => {
            measurements.record_accepted_fact(&fact);
            CertificateVerdict::Certified
        }
        Err(_) => CertificateVerdict::Rejected,
    };
    measurements.record_certificate_verdict(verdict);
    if verdict == CertificateVerdict::Certified
        && let Some(consultation) = derivations
    {
        consultation.retain(certificate);
    }
    verdict
}

fn state_return_certificate<'program>(
    proof_plan: &ProofPlan<'program>,
    obligation: &BoundedStateReturnObligation,
    context: &AssignmentRangeContext<'program>,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let program = proof_plan.program;
    // A malformed obligation is the checker's diagnostic, not a certificate
    // question; the statement-identity gate mirrors `return_arrival`.
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    if !std::ptr::eq(program, context.program)
        || !matches!(
            statements.get(obligation.statement_index),
            Some(StatementNode::Expression(value)) if *value == obligation.value
        )
    {
        return None;
    }
    if let Some((minimum, maximum)) = validation::arrival_integer_expression_bounds(
        program,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
        obligation.value,
    ) {
        // The joined arrival bound owns this verdict when it answers. Cite
        // it as the premise pair only when it reaches the target; a bound
        // that misses keeps the trusted derivation's identical failure
        // rather than falling through to the declared legs it supersedes.
        return interval_premise_certificate(
            proof_plan,
            obligation.base_type,
            minimum,
            maximum,
            target,
            seed,
        );
    }
    bounded_integer_value(
        proof_plan,
        obligation.value,
        obligation.value_constraints,
        obligation.base_type,
        target,
        seed,
        false,
    )
    .or_else(|| {
        contract_refined_certificate(
            proof_plan, obligation, machine, state, context, target, seed,
        )
    })
}

/// A `(minimum, maximum)` interval an outside query established for the
/// value -- the arrival join's bound -- cited as the same premise pair the
/// declared leg uses. Emits only when the interval already reaches the
/// target; a gap leaves the leg uncovered and the trusted derivation keeps
/// its own verdict.
fn interval_premise_certificate(
    proof_plan: &ProofPlan,
    base_type: TypeReferenceHandle,
    minimum: i64,
    maximum: i64,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let interval = IntegerRange {
        minimum: BigInt::from_i64(minimum),
        maximum: BigInt::from_i64(maximum),
    };
    if interval.minimum < target.minimum || interval.maximum > target.maximum {
        return None;
    }
    let integer_type = fixed_integer_type(proof_plan.program.primitive_type_reference(base_type)?)?;
    declared_interval_certificate(
        &interval,
        integer_type,
        bigint_math_literal(&target.minimum)?,
        bigint_math_literal(&target.maximum)?,
        seed,
    )
}

/// The Requires-contract leg for a state return: each authored condition
/// whose operands survive the state prefix contributes the same `subject OP
/// literal` facts `requires_conditions::apply_condition` applies, restated as
/// `<=` premises on the return atom beside the declared pair. The trusted
/// path's operand-level refold stays uncovered, as does every leg when the
/// declared carrier interval is absent.
fn contract_refined_certificate<'program>(
    proof_plan: &ProofPlan<'program>,
    obligation: &BoundedStateReturnObligation,
    machine: &'program Machine,
    state: &'program State,
    context: &AssignmentRangeContext<'program>,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let program = proof_plan.program;
    // The trusted path refines this same interval and fails without it; the
    // certificate keeps that gate.
    let declared = integer_range_for_return_value(proof_plan, obligation)?;
    let integer_type = fixed_integer_type(program.primitive_type_reference(obligation.base_type)?)?;

    let conditions = requires_conditions::surviving_conditions(
        proof_plan,
        context,
        machine.symbol,
        state.symbol,
        obligation.statement_index,
        obligation.value,
        obligation.binary_operands.as_ref(),
    );
    if conditions.is_empty() {
        return None;
    }

    let mut facts = Vec::new();
    push_bound(&mut facts, declared.minimum.clone(), true);
    push_bound(&mut facts, declared.maximum.clone(), false);
    for condition in &conditions {
        collect_contract_bounds(proof_plan, obligation.value, *condition, &mut facts);
    }
    // The trusted path discards a contradictory refinement and reports the
    // declared interval instead; the certificate keeps only the declared
    // pair when the collected facts cannot hold together.
    let mut narrowed = declared.clone();
    for fact in &facts[2..] {
        if fact.lower {
            narrowed.minimum = narrowed.minimum.max(fact.bound.clone());
        } else {
            narrowed.maximum = narrowed.maximum.min(fact.bound.clone());
        }
    }
    if narrowed.minimum > narrowed.maximum {
        facts.truncate(2);
    }

    let value = ValueId::new(seed)?;
    let atom = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value,
    };
    let proposition_context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))]).ok()?;
    let mut assumptions = Vec::with_capacity(facts.len());
    for fact in &facts {
        push_math_premise(&mut assumptions, &fact.bound, &atom, fact.lower);
    }
    guarded_bounds_certificate(atom, assumptions, proposition_context, target, seed)
}

/// `condition`'s `subject OP literal` conjuncts restated as `<=` facts on
/// the subject -- the certificate mirror of `requires_conditions::apply_condition`:
/// one `== true` unwrap, `&&` splits, and a side spelling the subject as the
/// SAME stable place owns the comparison. Unlike the guard collectors there
/// is no `!` arm: contract conditions are assumptions, not fall-through
/// complements.
fn collect_contract_bounds(
    proof_plan: &ProofPlan,
    subject: ExpressionHandle,
    condition: ExpressionHandle,
    facts: &mut Vec<GuardBound>,
) {
    let program = proof_plan.program;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(condition) else {
        return;
    };
    if binary.operator == BinaryOperator::Equal {
        if matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return collect_contract_bounds(proof_plan, subject, binary.left, facts);
        }
        if matches!(
            program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return collect_contract_bounds(proof_plan, subject, binary.right, facts);
        }
    }
    if binary.operator == BinaryOperator::And {
        collect_contract_bounds(proof_plan, subject, binary.left, facts);
        collect_contract_bounds(proof_plan, subject, binary.right, facts);
        return;
    }
    if requires_conditions::same_place(proof_plan, binary.left, subject) {
        if let Some(value) = integer_literal_handle(proof_plan, binary.right) {
            push_right_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
        }
        return;
    }
    if requires_conditions::same_place(proof_plan, binary.right, subject)
        && let Some(value) = integer_literal_handle(proof_plan, binary.left)
    {
        push_left_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
    }
}

/// Certificate route for a guarded transition argument: the arm's own guard
/// conjuncts and every refuted prior exit guard enter as explicit premises,
/// so a bound the guard establishes is re-decided by the kernel from those
/// premises instead of standing on the trusted narrowing arithmetic. Two legs
/// mirror the trusted derivation: the direct leg states the whole argument as
/// one opaque atom and cites the `argument OP literal` facts
/// `guards::apply_handle_condition`/`apply_handle_condition_complement` apply
/// to it; the refold leg reads a `place +- K` argument through its operand's
/// atom and lets the kernel's affine rule re-derive the shifted bound.
/// Anonymous-owned arguments, non-exact arithmetic, `K - place` refolds,
/// point exclusions, and the arrival rescue stay uncovered and keep the
/// ordinary derivation.
pub fn guarded_transition_integer_verdict(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target: &IntegerRange,
    seed: u64,
) -> CertificateVerdict {
    let mut measurements = ProofPlanMeasurements::default();
    guarded_transition_integer_verdict_measured(
        proof_plan,
        obligation,
        target,
        seed,
        &mut measurements,
        None,
    )
}

/// `guarded_transition_integer_verdict` with the run's measurements recorded
/// into the caller's recorder.
pub(crate) fn guarded_transition_integer_verdict_measured(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target: &IntegerRange,
    seed: u64,
    measurements: &mut ProofPlanMeasurements,
    mut derivations: Option<&mut DerivationConsultation<'_>>,
) -> CertificateVerdict {
    if let Some(consultation) = derivations.as_deref_mut()
        && let Some(fact) = consultation.recheck()
    {
        measurements.record_accepted_fact(&fact);
        measurements.record_certificate_verdict(CertificateVerdict::Certified);
        return CertificateVerdict::Certified;
    }
    let Some(certificate) = guarded_transition_certificate(proof_plan, obligation, target, seed)
    else {
        measurements.record_certificate_verdict(CertificateVerdict::Uncovered);
        return CertificateVerdict::Uncovered;
    };
    measurements.record_emitted_certificate(&certificate.envelope.proof);
    let verdict = match certificate.verify() {
        Ok(fact) => {
            measurements.record_accepted_fact(&fact);
            CertificateVerdict::Certified
        }
        Err(_) => CertificateVerdict::Rejected,
    };
    measurements.record_certificate_verdict(verdict);
    if verdict == CertificateVerdict::Certified
        && let Some(consultation) = derivations
    {
        consultation.retain(certificate);
    }
    verdict
}

/// The kernel-facing package for a guarded transition argument, in the order
/// the trusted narrowing tries them: the DIRECT leg states the argument as
/// one opaque atom and cites every `argument OP literal` guard fact as a
/// premise; the REFOLD leg reads a `place +- K` argument through its
/// operand's atom and lets the kernel's affine rule re-derive the shifted
/// bound from the premises the guard narrows on the place. Either emits only
/// when its anchors already reach the target endpoints -- anything else
/// leaves the trusted derivation's verdict untouched.
fn guarded_transition_certificate(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    // The anonymous leg owns any argument it lands (see `bounded_integer_value`):
    // when this route runs, that leg already kept the trusted verdict -- an
    // unencodable term or a closed encoding that disagreed with the landing.
    // Its constraint interval derives from truncating `integer_binary_range`
    // semantics, which the exact-rational landing overrides, so re-citing it
    // here would re-litigate a verdict the anonymous leg deliberately left to
    // the ordinary path (`7 / 2 * 2` lands at 7; its derived interval says 6).
    let anonymous_owned = proof_plan
        .program
        .primitive_type_reference(obligation.base_type)
        .and_then(|primitive| {
            validation::land_anonymous_integer_expression(
                proof_plan.program,
                obligation.argument,
                primitive,
                |expression| {
                    validation::has_anonymous_operator_meaning(proof_plan.program, expression)
                },
            )
        })
        .is_some();
    if anonymous_owned {
        return None;
    }
    guarded_direct_certificate(proof_plan, obligation, target, seed)
        .or_else(|| guarded_refold_certificate(proof_plan, obligation, target, seed))
        .or_else(|| arrival_argument_certificate(proof_plan, obligation, target, seed))
}

/// The arrival-rescue leg for a transition argument: the bound the trusted
/// derivation consults when its guarded range misses the target enters as
/// the premise pair. Runs after the direct and refold legs decline, exactly
/// where the trusted path consults the arrival query.
fn arrival_argument_certificate(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let (minimum, maximum) = validation::arrival_integer_expression_bounds(
        proof_plan.program,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
        obligation.argument,
    )?;
    interval_premise_certificate(
        proof_plan,
        obligation.base_type,
        minimum,
        maximum,
        target,
        seed,
    )
}

/// The direct leg: the whole argument is one opaque mathematical atom and the
/// declared interval plus every guard fact collected on the argument
/// expression enter as `<=` premises on it. Covers `accept(value)` under
/// `value > 0` and its refuted/`&&`-nested equivalents.
fn guarded_direct_certificate(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let integer_type = fixed_integer_type(
        proof_plan
            .program
            .primitive_type_reference(obligation.base_type)?,
    )?;
    let value = ValueId::new(seed)?;
    let atom = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value,
    };
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))]).ok()?;

    let mut facts = Vec::new();
    // Declared interval endpoints are the same premise pair the declared leg
    // cites; the guard facts below narrow them where the declared range alone
    // did not fit.
    if let Some(declared) = integer_range_from_constraints(type_constraints(
        proof_plan,
        obligation.argument_constraints,
    )) {
        push_bound(&mut facts, declared.minimum.clone(), true);
        push_bound(&mut facts, declared.maximum.clone(), false);
    }
    if let TransitionGuardNode::When(condition) = &obligation.guard {
        collect_guard_bounds(proof_plan, obligation.argument, *condition, &mut facts);
    }
    for refuted in &obligation.refuted_exit_guards {
        collect_refuted_bounds(proof_plan, obligation.argument, *refuted, &mut facts);
    }
    let mut assumptions = Vec::with_capacity(facts.len());
    for fact in &facts {
        push_math_premise(&mut assumptions, &fact.bound, &atom, fact.lower);
    }
    guarded_bounds_certificate(atom, assumptions, context, target, seed)
}

/// The pure constructor behind [`guarded_transition_certificate`]: from the
/// premise roster pick the strongest `K <= atom` and `atom <= K` anchors and
/// chain each to its target endpoint through one closed `<=` step, exactly as
/// the declared leg's transitivity chains do. `None` when no anchor pair
/// reaches the target -- the leg stays uncovered and the trusted derivation
/// decides it.
fn guarded_bounds_certificate(
    atom: IntegerMathTerm,
    assumptions: Vec<Proposition>,
    context: PropositionContext,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let mut lower_anchor: Option<(usize, BigInt)> = None;
    let mut upper_anchor: Option<(usize, BigInt)> = None;
    for (index, assumption) in assumptions.iter().enumerate() {
        let Proposition::IntegerMathLessOrEqual(left, right) = assumption else {
            continue;
        };
        if *right == atom
            && let IntegerMathTerm::IntegerLiteral(bound) = left
        {
            let bound = math_literal_bignum(*bound);
            if lower_anchor
                .as_ref()
                .is_none_or(|(_, strongest)| bound > *strongest)
            {
                lower_anchor = Some((index, bound));
            }
        }
        if *left == atom
            && let IntegerMathTerm::IntegerLiteral(bound) = right
        {
            let bound = math_literal_bignum(*bound);
            if upper_anchor
                .as_ref()
                .is_none_or(|(_, strongest)| bound < *strongest)
            {
                upper_anchor = Some((index, bound));
            }
        }
    }
    let (lower_index, lower_bound) = lower_anchor?;
    let (upper_index, upper_bound) = upper_anchor?;
    // Each cited anchor must reach its target endpoint; a gap leaves the leg
    // uncovered rather than emitting a certificate the kernel must refuse.
    if target.minimum > lower_bound || target.maximum < upper_bound {
        return None;
    }
    let lower_leg = transitivity_leg(
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&target.minimum)?),
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&lower_bound)?),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        },
        ProofNode {
            conclusion: assumptions[lower_index].clone(),
            rule: ProofRule::Assumption { index: lower_index },
        },
    );
    let upper_leg = transitivity_leg(
        ProofNode {
            conclusion: assumptions[upper_index].clone(),
            rule: ProofRule::Assumption { index: upper_index },
        },
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&upper_bound)?),
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&target.maximum)?),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        },
    );
    let conclusion = Proposition::Conjunction(vec![
        lower_leg.conclusion.clone(),
        upper_leg.conclusion.clone(),
    ]);
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::ConjunctionIntroduction(vec![lower_leg, upper_leg]),
    };
    Some(BoundedValueCertificate {
        context,
        obligation: Obligation {
            id: ObligationId::new(seed)?,
            proposition: conclusion,
            class: ObligationClass::Derivable,
        },
        assumptions,
        envelope: envelope(seed, proof)?,
    })
}

/// One fact a guard or declaration establishes on a subject expression:
/// `bound <= subject` for `lower`, `subject <= bound` otherwise.
struct GuardBound {
    bound: BigInt,
    lower: bool,
}

fn push_bound(facts: &mut Vec<GuardBound>, bound: BigInt, lower: bool) {
    facts.push(GuardBound { bound, lower });
}

/// Every `subject OP literal` or `literal OP subject` conjunct the guard
/// condition establishes, restated as the exact `<=` facts it contributes on
/// the subject. Mirrors `guards::apply_handle_condition`: `== true` unwraps,
/// `&&` splits, `!` defers to the refuted table, and a side equivalent to the
/// subject owns the comparison -- anything else contributes nothing.
fn collect_guard_bounds(
    proof_plan: &ProofPlan,
    subject: ExpressionHandle,
    condition: ExpressionHandle,
    facts: &mut Vec<GuardBound>,
) {
    let program = proof_plan.program;
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
        && unary.operator == UnaryOperator::LogicalNot
    {
        collect_refuted_bounds(proof_plan, subject, unary.operand, facts);
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(condition) else {
        return;
    };
    if binary.operator == BinaryOperator::Equal {
        if matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return collect_guard_bounds(proof_plan, subject, binary.left, facts);
        }
        if matches!(
            program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return collect_guard_bounds(proof_plan, subject, binary.right, facts);
        }
    }
    if binary.operator == BinaryOperator::And {
        collect_guard_bounds(proof_plan, subject, binary.left, facts);
        collect_guard_bounds(proof_plan, subject, binary.right, facts);
        return;
    }
    if expressions_equivalent_for_proof(proof_plan, binary.left, subject) {
        if let Some(value) = integer_literal_handle(proof_plan, binary.right) {
            push_right_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
        }
        return;
    }
    if expressions_equivalent_for_proof(proof_plan, binary.right, subject)
        && let Some(value) = integer_literal_handle(proof_plan, binary.left)
    {
        push_left_literal_bounds(binary.operator, BigInt::from_i64(value), facts);
    }
}

/// The refuted twin of [`collect_guard_bounds`], mirroring
/// `guards::apply_handle_condition_complement`: one `== true` unwrap, a
/// double negation folds back to the positive table, and the comparison must
/// spell `subject OP literal`. `!(x == K)` excludes only the point `K` -- a
/// bound it tightens has no flat `<=` restatement, so it contributes nothing
/// and a leg needing the exclusion stays uncovered.
fn collect_refuted_bounds(
    proof_plan: &ProofPlan,
    subject: ExpressionHandle,
    condition: ExpressionHandle,
    facts: &mut Vec<GuardBound>,
) {
    let program = proof_plan.program;
    let condition = unwrap_true_guard_condition(proof_plan, condition);
    if let ExpressionNode::Unary(unary) = program.expression_table.expression(condition)
        && unary.operator == UnaryOperator::LogicalNot
    {
        collect_guard_bounds(proof_plan, subject, unary.operand, facts);
        return;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(condition) else {
        return;
    };
    if !expressions_equivalent_for_proof(proof_plan, binary.left, subject) {
        return;
    }
    let Some(value) = integer_literal_handle(proof_plan, binary.right) else {
        return;
    };
    let value = BigInt::from_i64(value);
    let one = BigInt::from_i64(1);
    match binary.operator {
        BinaryOperator::Equal => {}
        BinaryOperator::Less => push_bound(facts, value, true),
        BinaryOperator::LessOrEqual => push_bound(facts, value.add(&one), true),
        BinaryOperator::Greater => push_bound(facts, value, false),
        BinaryOperator::GreaterOrEqual => push_bound(facts, value.sub(&one), false),
        _ => {}
    }
}

/// `subject OP K` restated as `<=` facts on the subject -- the right-literal
/// table. Strict comparisons restate at the exact adjacent integer and
/// equality as its antisymmetric pair; `!=` and every non-order operator
/// contribute nothing, matching `guards::apply_right_literal_guard`.
fn push_right_literal_bounds(operator: BinaryOperator, value: BigInt, facts: &mut Vec<GuardBound>) {
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Equal => {
            push_bound(facts, value.clone(), true);
            push_bound(facts, value, false);
        }
        BinaryOperator::Greater => push_bound(facts, value.add(&one), true),
        BinaryOperator::GreaterOrEqual => push_bound(facts, value, true),
        BinaryOperator::Less => push_bound(facts, value.sub(&one), false),
        BinaryOperator::LessOrEqual => push_bound(facts, value, false),
        _ => {}
    }
}

/// `K OP subject` -- the same restatement with the literal on the left,
/// matching `guards::apply_left_literal_guard`.
fn push_left_literal_bounds(operator: BinaryOperator, value: BigInt, facts: &mut Vec<GuardBound>) {
    let one = BigInt::from_i64(1);
    match operator {
        BinaryOperator::Equal => {
            push_bound(facts, value.clone(), true);
            push_bound(facts, value, false);
        }
        BinaryOperator::Greater => push_bound(facts, value.sub(&one), false),
        BinaryOperator::GreaterOrEqual => push_bound(facts, value, false),
        BinaryOperator::Less => push_bound(facts, value.add(&one), true),
        BinaryOperator::LessOrEqual => push_bound(facts, value, true),
        _ => {}
    }
}

/// `bound <= atom` for `lower_side`, `atom <= bound` otherwise -- one premise
/// on the shared mathematical atom. An unencodable bound simply contributes
/// nothing.
fn push_math_premise(
    assumptions: &mut Vec<Proposition>,
    bound: &BigInt,
    atom: &IntegerMathTerm,
    lower_side: bool,
) {
    let Some(bound) = bigint_math_literal(bound) else {
        return;
    };
    let bound = IntegerMathTerm::IntegerLiteral(bound);
    assumptions.push(if lower_side {
        Proposition::IntegerMathLessOrEqual(bound, atom.clone())
    } else {
        Proposition::IntegerMathLessOrEqual(atom.clone(), bound)
    });
}

/// The refold leg: the argument is `place + K` or `place - K` under exact
/// arithmetic, so its denotation is the affine image of the place's atom.
/// The place's declared interval and every `place OP literal` guard fact
/// enter as scalar `<=` premises; each endpoint's strongest premise feeds the
/// kernel's direct affine-bound map, and one closed `<=` step carries the
/// mapped endpoint to the target's. `K - place`, point exclusions and
/// non-literal operands stay uncovered and keep the ordinary derivation.
fn guarded_refold_certificate(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let program = proof_plan.program;
    let ExpressionNode::Binary(binary) = program.expression_table.expression(obligation.argument)
    else {
        return None;
    };
    let (place, literal, subtract) = match binary.operator {
        BinaryOperator::Add => {
            if let Some(literal) = integer_literal_handle(proof_plan, binary.right) {
                (binary.left, literal, false)
            } else {
                let literal = integer_literal_handle(proof_plan, binary.left)?;
                (binary.right, literal, false)
            }
        }
        BinaryOperator::Subtract => {
            let literal = integer_literal_handle(proof_plan, binary.right)?;
            (binary.left, literal, true)
        }
        _ => return None,
    };
    // A literal place belongs to the closed-term legs, not the affine refold.
    if integer_literal_handle(proof_plan, place).is_some() {
        return None;
    }
    // The certificate claims the argument denotes exact mathematical
    // arithmetic; a wrapping or saturating domain would falsify that
    // denotation, so those stay uncovered.
    let exact = type_constraints(proof_plan, obligation.argument_constraints)
        .iter()
        .all(|constraint| match constraint {
            ProofConstraint::ArithmeticDomain(domain) => {
                *domain == numerics::arithmetic::ArithmeticDomain::Exact
            }
            ProofConstraint::Named(name) => name.as_str() != "wrapping",
            _ => true,
        });
    if !exact {
        return None;
    }

    // The place's own carrier and declared interval come from the program's
    // scope at this state -- the same facts `expression_constraints` resolves
    // for the narrowing derivation.
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)?;
    let state = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)?;
    let place_type = crate::obligations::expression_type_reference(program, machine, state, place)?;
    let integer_type = fixed_integer_type(program.primitive_type_reference(place_type)?)?;
    let literal_bigint = BigInt::from_i64(literal);

    let place_constraints: Vec<ProofConstraint> =
        crate::obligations::expression_constraints(program, machine, state, place)
            .into_iter()
            .collect();
    let mut facts = Vec::new();
    if let Some(declared) = integer_range_from_constraints(&place_constraints) {
        push_bound(&mut facts, declared.minimum.clone(), true);
        push_bound(&mut facts, declared.maximum.clone(), false);
    }
    if let TransitionGuardNode::When(condition) = &obligation.guard {
        collect_guard_bounds(proof_plan, place, *condition, &mut facts);
    }
    for refuted in &obligation.refuted_exit_guards {
        collect_refuted_bounds(proof_plan, place, *refuted, &mut facts);
    }

    let value = ValueId::new(seed)?;
    let place_scalar = ScalarTerm::value(value, ScalarType::Integer(integer_type));
    let place_math = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value,
    };
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))]).ok()?;

    // Render every collected fact as the scalar `<=` premise the affine
    // rule's endpoint evidence requires; a bound the place's carrier cannot
    // spell contributes nothing (it could not tighten that carrier anyway).
    let mut assumptions = Vec::with_capacity(facts.len());
    let mut lower_anchor: Option<(usize, BigInt)> = None;
    let mut upper_anchor: Option<(usize, BigInt)> = None;
    for fact in &facts {
        let index = assumptions.len();
        if !push_scalar_premise(
            &mut assumptions,
            &fact.bound,
            &place_scalar,
            fact.lower,
            integer_type,
        ) {
            continue;
        }
        if fact.lower {
            if lower_anchor
                .as_ref()
                .is_none_or(|(_, strongest)| fact.bound > *strongest)
            {
                lower_anchor = Some((index, fact.bound.clone()));
            }
        } else if upper_anchor
            .as_ref()
            .is_none_or(|(_, strongest)| fact.bound < *strongest)
        {
            upper_anchor = Some((index, fact.bound.clone()));
        }
    }
    let (lower_index, lower_bound) = lower_anchor?;
    let (upper_index, upper_bound) = upper_anchor?;
    refold_bounds_certificate(
        place_scalar,
        place_math,
        integer_type,
        literal_bigint,
        subtract,
        assumptions,
        (lower_index, lower_bound),
        (upper_index, upper_bound),
        context,
        target,
        seed,
    )
}

/// The pure constructor behind [`guarded_refold_certificate`]: each selected
/// premise endpoint maps through `place +- K` under an independently checked
/// `IntegerAffineWitness`, and one closed `<=` step carries the mapped
/// endpoint to the target's. `None` when the mapped anchors do not reach the
/// target -- the leg stays uncovered and the trusted derivation decides it.
fn refold_bounds_certificate(
    place_scalar: ScalarTerm,
    place_math: IntegerMathTerm,
    integer_type: IntegerType,
    literal: BigInt,
    subtract: bool,
    assumptions: Vec<Proposition>,
    lower_anchor: (usize, BigInt),
    upper_anchor: (usize, BigInt),
    context: PropositionContext,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let (lower_index, lower_bound) = lower_anchor;
    let (upper_index, upper_bound) = upper_anchor;

    // The mapped endpoint is the anchor's image through `place +- K`; each
    // must reach its target endpoint or the leg stays uncovered.
    let mapped_lower = if subtract {
        lower_bound.sub(&literal)
    } else {
        lower_bound.add(&literal)
    };
    let mapped_upper = if subtract {
        upper_bound.sub(&literal)
    } else {
        upper_bound.add(&literal)
    };
    if target.minimum > mapped_lower || target.maximum < mapped_upper {
        return None;
    }

    let literal_math = IntegerMathTerm::IntegerLiteral(bigint_math_literal(&literal)?);
    let argument_math = if subtract {
        IntegerMathTerm::Subtract(Box::new(place_math), Box::new(literal_math))
    } else {
        IntegerMathTerm::Add(Box::new(place_math), Box::new(literal_math))
    };
    let literal_scalar = scalar_integer_term(integer_type, &literal)?;
    let target_scalar = if subtract {
        ScalarTerm::exact_integer_subtract(integer_type, place_scalar.clone(), literal_scalar)
    } else {
        ScalarTerm::exact_integer_add(integer_type, place_scalar.clone(), literal_scalar)
    }
    .ok()?;
    let witness = IntegerAffineWitness {
        root: place_scalar,
        target: target_scalar,
        definition_axioms: Vec::new(),
        literal_axioms: Vec::new(),
    };

    let lower_leg = transitivity_leg(
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&target.minimum)?),
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&mapped_lower)?),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        },
        affine_leg(
            &witness,
            &assumptions,
            lower_index,
            &mapped_lower,
            &argument_math,
            true,
        )?,
    );
    let upper_leg = transitivity_leg(
        affine_leg(
            &witness,
            &assumptions,
            upper_index,
            &mapped_upper,
            &argument_math,
            false,
        )?,
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&mapped_upper)?),
                IntegerMathTerm::IntegerLiteral(bigint_math_literal(&target.maximum)?),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        },
    );
    let conclusion = Proposition::Conjunction(vec![
        lower_leg.conclusion.clone(),
        upper_leg.conclusion.clone(),
    ]);
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::ConjunctionIntroduction(vec![lower_leg, upper_leg]),
    };
    Some(BoundedValueCertificate {
        context,
        obligation: Obligation {
            id: ObligationId::new(seed)?,
            proposition: conclusion,
            class: ObligationClass::Derivable,
        },
        assumptions,
        envelope: envelope(seed, proof)?,
    })
}

/// One affine endpoint step: the kernel's direct `place +- K` bound map from
/// the cited `place` premise plus a `Truth` leaf for the literal operand --
/// the exact `root_bound` conjunction `map_direct_add_bound`/
/// `map_direct_subtract_bound` destructure. For `lower` the conclusion is
/// `mapped <= argument`; otherwise `argument <= mapped`.
fn affine_leg(
    witness: &IntegerAffineWitness,
    assumptions: &[Proposition],
    anchor_index: usize,
    mapped: &BigInt,
    argument_math: &IntegerMathTerm,
    lower: bool,
) -> Option<ProofNode> {
    let premise = assumptions.get(anchor_index)?.clone();
    let root_bound = ProofNode {
        conclusion: Proposition::Conjunction(vec![premise.clone(), Proposition::Truth]),
        rule: ProofRule::ConjunctionIntroduction(vec![
            ProofNode {
                conclusion: premise,
                rule: ProofRule::Assumption {
                    index: anchor_index,
                },
            },
            ProofNode {
                conclusion: Proposition::Truth,
                rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
            },
        ]),
    };
    let mapped = IntegerMathTerm::IntegerLiteral(bigint_math_literal(mapped)?);
    let conclusion = if lower {
        Proposition::IntegerMathLessOrEqual(mapped, argument_math.clone())
    } else {
        Proposition::IntegerMathLessOrEqual(argument_math.clone(), mapped)
    };
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound),
            witness: witness.clone(),
        },
    })
}

/// `bound <= atom` for `lower_side`, `atom <= bound` otherwise -- one scalar
/// `<=` premise on the place's atom, in the form the affine rule's endpoint
/// evidence reads. Returns false when the bound does not fit the carrier; a
/// bound the carrier cannot spell cannot tighten it.
fn push_scalar_premise(
    assumptions: &mut Vec<Proposition>,
    bound: &BigInt,
    atom: &ScalarTerm,
    lower_side: bool,
    integer_type: IntegerType,
) -> bool {
    let Some(bound) = scalar_integer_term(integer_type, bound) else {
        return false;
    };
    assumptions.push(if lower_side {
        Proposition::LessOrEqual(bound, atom.clone())
    } else {
        Proposition::LessOrEqual(atom.clone(), bound)
    });
    true
}

/// A scalar integer literal admitted by `integer_type` -- the same value the
/// mathematical `bigint_math_literal` encodes, at the place's carrier.
fn scalar_integer_term(integer_type: IntegerType, bound: &BigInt) -> Option<ScalarTerm> {
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(bound.to_string().parse().ok()?),
        IntegerSign::Unsigned => {
            if bound.is_negative() {
                return None;
            }
            IntegerValue::Unsigned(bound.to_string().parse().ok()?)
        }
    };
    ScalarTerm::integer(integer_type, value).ok()
}

/// Emit a certificate for the value legs this producer covers, mirroring the
/// ordinary derivation's decision order so a covered certificate always
/// carries the same verdict the trusted path would reach. Anything else
/// returns `None` and keeps the trusted derivation's verdict.
fn bounded_integer_value(
    proof_plan: &ProofPlan,
    value: ExpressionHandle,
    value_constraints: HandleSpan<ProofConstraint>,
    base_type: TypeReferenceHandle,
    target: &IntegerRange,
    seed: u64,
    anonymous: bool,
) -> Option<BoundedValueCertificate> {
    let lower = bigint_math_literal(&target.minimum)?;
    let upper = bigint_math_literal(&target.maximum)?;
    let node = proof_plan.program.expression_table.expression(value);
    if anonymous {
        let program = proof_plan.program;
        let landed = program
            .primitive_type_reference(base_type)
            .and_then(|primitive| {
                validation::land_anonymous_integer_expression(
                    program,
                    value,
                    primitive,
                    |expression| validation::has_anonymous_operator_meaning(program, expression),
                )
            });
        if let Some(landed) = landed {
            // The anonymous leg owns this verdict -- including a landed value
            // that fails the target -- so a landed expression must never fall
            // through to the literal or declared legs. The certificate covers
            // it only when the encoded term agrees with the engine's landing;
            // anything else keeps the trusted verdict.
            let term = closed_math_term(proof_plan, value)?;
            if eval_math_term(&term)? != landed.value_bignum()? {
                return None;
            }
            return closed_bounds_certificate(term, lower, upper, seed);
        }
    }
    if let ExpressionNode::Integer(literal) = node {
        // The literal leg never falls through to declared constraints; an
        // unencodable literal keeps the trusted verdict the same way.
        let term = literal_math_term(literal)?;
        return closed_bounds_certificate(term, lower, upper, seed);
    }
    declared_bounds_certificate(
        proof_plan,
        value_constraints,
        base_type,
        target,
        lower,
        upper,
        seed,
    )
}

/// `lower <= term <= upper` proved by closed evaluation at both endpoints.
/// The whole obligation re-evaluates in the kernel; there are no premises.
fn closed_bounds_certificate(
    term: IntegerMathTerm,
    lower: IntegerMathLiteral,
    upper: IntegerMathLiteral,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let lower_conjunct =
        Proposition::IntegerMathLessOrEqual(IntegerMathTerm::IntegerLiteral(lower), term.clone());
    let upper_conjunct =
        Proposition::IntegerMathLessOrEqual(term, IntegerMathTerm::IntegerLiteral(upper));
    let conclusion = Proposition::Conjunction(vec![lower_conjunct.clone(), upper_conjunct.clone()]);
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::ConjunctionIntroduction(vec![
            ProofNode {
                conclusion: lower_conjunct,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            },
            ProofNode {
                conclusion: upper_conjunct,
                rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
            },
        ]),
    };
    Some(BoundedValueCertificate {
        context: PropositionContext::default(),
        obligation: Obligation {
            id: ObligationId::new(seed)?,
            proposition: conclusion,
            class: ObligationClass::Derivable,
        },
        assumptions: Vec::new(),
        envelope: envelope(seed, proof)?,
    })
}

/// `lower <= v <= upper` for one opaque mathematical value `v` carrying the
/// declared interval, proved by `<=` transitivity through the declared
/// endpoints. The declared bounds enter as an explicit premise pair -- the
/// certificate states exactly which declared facts the leg consumed -- and
/// each endpoint step is a closed primitive the kernel re-evaluates.
fn declared_bounds_certificate(
    proof_plan: &ProofPlan,
    value_constraints: HandleSpan<ProofConstraint>,
    base_type: TypeReferenceHandle,
    target: &IntegerRange,
    lower: IntegerMathLiteral,
    upper: IntegerMathLiteral,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let declared = integer_range_from_constraints(type_constraints(proof_plan, value_constraints))?;
    // A certificate only exists where the declared interval alone proves the
    // target. Guard narrowing, refuted complements, dependent bounds and the
    // arrival refinements stay uncovered and keep the ordinary verdict.
    if declared.minimum < target.minimum || declared.maximum > target.maximum {
        return None;
    }
    let integer_type = fixed_integer_type(proof_plan.program.primitive_type_reference(base_type)?)?;
    declared_interval_certificate(&declared, integer_type, lower, upper, seed)
}

/// The pure constructor behind [`declared_bounds_certificate`]: one opaque
/// atom of `integer_type`, premises `declared <= atom` and `atom <= declared`,
/// and a transitivity chain at each endpoint.
fn declared_interval_certificate(
    declared: &IntegerRange,
    integer_type: IntegerType,
    lower: IntegerMathLiteral,
    upper: IntegerMathLiteral,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    let value = ValueId::new(seed)?;
    let atom = IntegerMathTerm::MathValue {
        source_type: integer_type,
        value,
    };
    let context =
        PropositionContext::from_value_types([(value, ScalarType::Integer(integer_type))]).ok()?;
    let declared_lower = bigint_math_literal(&declared.minimum)?;
    let declared_upper = bigint_math_literal(&declared.maximum)?;

    let lower_premise = Proposition::IntegerMathLessOrEqual(
        IntegerMathTerm::IntegerLiteral(declared_lower),
        atom.clone(),
    );
    let upper_premise = Proposition::IntegerMathLessOrEqual(
        atom.clone(),
        IntegerMathTerm::IntegerLiteral(declared_upper),
    );

    let lower_leg = transitivity_leg(
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(lower),
                IntegerMathTerm::IntegerLiteral(declared_lower),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        },
        ProofNode {
            conclusion: lower_premise.clone(),
            rule: ProofRule::Assumption { index: 0 },
        },
    );
    let upper_leg = transitivity_leg(
        ProofNode {
            conclusion: upper_premise.clone(),
            rule: ProofRule::Assumption { index: 1 },
        },
        ProofNode {
            conclusion: Proposition::IntegerMathLessOrEqual(
                IntegerMathTerm::IntegerLiteral(declared_upper),
                IntegerMathTerm::IntegerLiteral(upper),
            ),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        },
    );
    let conclusion = Proposition::Conjunction(vec![
        lower_leg.conclusion.clone(),
        upper_leg.conclusion.clone(),
    ]);
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::ConjunctionIntroduction(vec![lower_leg, upper_leg]),
    };
    Some(BoundedValueCertificate {
        context,
        obligation: Obligation {
            id: ObligationId::new(seed)?,
            proposition: conclusion,
            class: ObligationClass::Derivable,
        },
        assumptions: vec![lower_premise, upper_premise],
        envelope: envelope(seed, proof)?,
    })
}

/// One `<=` transitivity step: the children's shared middle composes the
/// node's conclusion.
fn transitivity_leg(
    left_less_or_equal_middle: ProofNode,
    middle_less_or_equal_right: ProofNode,
) -> ProofNode {
    let (
        Proposition::IntegerMathLessOrEqual(left, _),
        Proposition::IntegerMathLessOrEqual(_, right),
    ) = (
        &left_less_or_equal_middle.conclusion,
        &middle_less_or_equal_right.conclusion,
    )
    else {
        unreachable!("bounded-value legs only chain mathematical <= premises")
    };
    ProofNode {
        conclusion: Proposition::IntegerMathLessOrEqual(left.clone(), right.clone()),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
            middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
        },
    }
}

/// Encode the anonymous integer expressions the producer understands as one
/// closed mathematical term: integer literals, the `u32::MAX` constant name
/// the engine lands, and `+`/`-`/`*` trees whose operators carry builtin
/// meaning. Division, modulo, shifts and every named value stay unencoded.
fn closed_math_term(
    proof_plan: &ProofPlan,
    expression: ExpressionHandle,
) -> Option<IntegerMathTerm> {
    let program = proof_plan.program;
    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(literal) => literal_math_term(literal),
        ExpressionNode::Name(_) => {
            let value = integer_literal_handle(proof_plan, expression)?;
            math_literal_term(BigInt::from_i64(value))
        }
        ExpressionNode::Binary(binary) => {
            if !validation::has_anonymous_operator_meaning(program, expression) {
                return None;
            }
            let left = closed_math_term(proof_plan, binary.left)?;
            let right = closed_math_term(proof_plan, binary.right)?;
            match binary.operator {
                BinaryOperator::Add => Some(IntegerMathTerm::Add(Box::new(left), Box::new(right))),
                BinaryOperator::Subtract => {
                    Some(IntegerMathTerm::Subtract(Box::new(left), Box::new(right)))
                }
                BinaryOperator::Multiply => {
                    Some(IntegerMathTerm::Multiply(Box::new(left), Box::new(right)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Evaluate a closed mathematical term. Used only for the producer's own
/// consistency gate against the engine's landing; the kernel re-evaluates the
/// emitted term independently and remains the authority.
fn eval_math_term(term: &IntegerMathTerm) -> Option<BigInt> {
    match term {
        IntegerMathTerm::IntegerLiteral(literal) => Some(math_literal_bignum(*literal)),
        IntegerMathTerm::Add(left, right) => {
            Some(eval_math_term(left)?.add(&eval_math_term(right)?))
        }
        IntegerMathTerm::Subtract(left, right) => {
            Some(eval_math_term(left)?.sub(&eval_math_term(right)?))
        }
        IntegerMathTerm::Multiply(left, right) => {
            Some(eval_math_term(left)?.mul(&eval_math_term(right)?))
        }
        IntegerMathTerm::ShiftLeft { .. } | IntegerMathTerm::MathValue { .. } => None,
    }
}

fn literal_math_term(literal: &numerics::literals::IntegerLiteral) -> Option<IntegerMathTerm> {
    math_literal_term(literal.value_bignum()?)
}

fn math_literal_term(value: BigInt) -> Option<IntegerMathTerm> {
    Some(IntegerMathTerm::IntegerLiteral(bigint_math_literal(
        &value,
    )?))
}

fn math_literal_bignum(literal: IntegerMathLiteral) -> BigInt {
    let magnitude = BigInt::from_u128(literal.magnitude());
    if literal.negative() {
        magnitude.negate()
    } else {
        magnitude
    }
}

fn bigint_math_literal(value: &BigInt) -> Option<IntegerMathLiteral> {
    let magnitude: u128 = value.abs().to_string().parse().ok()?;
    IntegerMathLiteral::new(value.is_negative(), magnitude).ok()
}

fn fixed_integer_type(primitive: PrimitiveType) -> Option<IntegerType> {
    let (sign, bits) = match primitive {
        PrimitiveType::I8 => (IntegerSign::Signed, 8),
        PrimitiveType::I16 => (IntegerSign::Signed, 16),
        PrimitiveType::I32 => (IntegerSign::Signed, 32),
        PrimitiveType::I64 => (IntegerSign::Signed, 64),
        PrimitiveType::U8 => (IntegerSign::Unsigned, 8),
        PrimitiveType::U16 => (IntegerSign::Unsigned, 16),
        PrimitiveType::U32 => (IntegerSign::Unsigned, 32),
        PrimitiveType::U64 => (IntegerSign::Unsigned, 64),
        PrimitiveType::Bool | PrimitiveType::F32 | PrimitiveType::F64 | PrimitiveType::Addr => {
            return None;
        }
    };
    IntegerType::new(sign, bits).ok()
}

fn envelope(seed: u64, proof: ProofNode) -> Option<CertificateEnvelope> {
    Some(CertificateEnvelope {
        identity: EvidenceIdentity::new(seed)?,
        proof_system_marker: ProofSystemMarker::CURRENT,
        proof,
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests {
    //! The new legs' premise collectors and constructors, exercised against a
    //! minimal typed program: one machine `Main` with one entry state `entry`
    //! returning the state parameter `pending`, under a `requires pending <=
    //! 200` contract condition. The kernel -- not the producer -- decides
    //! every emitted package.

    use super::{
        CertificateVerdict, GuardBound, collect_contract_bounds, contract_refined_certificate,
        interval_premise_certificate, state_return_integer_verdict,
    };
    use crate::checker::AssignmentRangeContext;
    use crate::checker::measurement::ProofPlanMeasurements;
    use crate::checker::requires_conditions::{same_place, stable_expression};
    use crate::obligations::{
        BoundedStateReturnObligation, IntegerRange, ProofConstraint, ProofPlan,
    };
    use arena::HandleSpan;
    use numerics::bignum::BigInt;
    use numerics::literals::IntegerLiteral;
    use proof_admission::AcceptedFactRoute;
    use symbols::{SymbolHandle, SymbolKind, SymbolNameRef, SymbolTableBuilder};
    use typed_trees::TypedTrees;
    use typed_trees::domain::ProofFact;
    use typed_trees::expression::{
        BinaryOperator, ExpressionHandle, ExpressionNode, TableBinaryExpression, TableNamePath,
    };
    use typed_trees::machine::Machine;
    use typed_trees::name::Identifier;
    use typed_trees::signature::{SignatureContract, SignatureContractKind, StateParameter};
    use typed_trees::state::State;
    use typed_trees::statement::StatementNode;
    use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

    struct ReturnProgram {
        program: TypedTrees,
        machine: SymbolHandle,
        state: SymbolHandle,
        carrier: TypeReferenceHandle,
        value: ExpressionHandle,
        condition: ExpressionHandle,
    }

    fn name_path(program: &mut TypedTrees, symbol: SymbolHandle) -> TableNamePath {
        let mut members = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member(&mut members, Identifier::generated("pending"));
        let mut member_symbols = HandleSpan::empty();
        program
            .expression_table
            .push_name_path_member_symbol(&mut member_symbols, symbol);
        TableNamePath {
            members,
            member_symbols,
            head_symbol: symbol,
            symbol,
        }
    }

    fn binary(
        program: &mut TypedTrees,
        left: ExpressionHandle,
        operator: BinaryOperator,
        right: ExpressionHandle,
    ) -> ExpressionHandle {
        program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left,
                operator,
                right,
            }))
    }

    /// `machine Main { entry(pending: u8[0..=255]) requires pending <= 200 {
    ///     pending } }` -- the declared interval alone misses a `[0..=220]`
    /// return target; the contract condition is what reaches it.
    fn return_program() -> ReturnProgram {
        let mut builder = SymbolTableBuilder::new();
        let root = builder.insert_root(SymbolKind::Root, SymbolNameRef::Static("root"));
        // A hierarchy arena parent accepts exactly one child range: the
        // builtins and the machine share root's single batch.
        let root_children = symbols::builtin_type_symbols()
            .into_iter()
            .chain([(SymbolKind::Machine, SymbolNameRef::Static("Main"))]);
        let handles =
            SymbolTableBuilder::child_handles(builder.insert_children(root, root_children))
                .collect::<Vec<_>>();
        let u8_symbol = handles[symbols::BuiltinTypeAtom::U8.ordinal()];
        let machine = *handles.last().expect("machine");
        let state = SymbolTableBuilder::child_handles(builder.insert_children(
            machine,
            [(SymbolKind::State, SymbolNameRef::Static("entry"))],
        ))
        .next()
        .expect("state");
        let pending = SymbolTableBuilder::child_handles(builder.insert_children(
            state,
            [(SymbolKind::Parameter, SymbolNameRef::Static("pending"))],
        ))
        .next()
        .expect("parameter");

        let mut program = TypedTrees {
            symbols: builder.finish(),
            ..TypedTrees::default()
        };
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: u8_symbol,
                name: Identifier::generated("u8"),
            });
        let pending_path = name_path(&mut program, pending);
        let value = program
            .expression_table
            .insert(ExpressionNode::Name(pending_path));
        let bound = program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(200)));
        let condition = binary(&mut program, value, BinaryOperator::LessOrEqual, bound);

        let mut state_node = State {
            symbol: state,
            name: Identifier::generated("entry"),
            ..State::default()
        };
        program.push_state_parameter(
            &mut state_node,
            StateParameter {
                symbol: pending,
                name: Identifier::generated("pending"),
                type_reference: carrier,
                ..StateParameter::default()
            },
        );
        let statement = program
            .statement_table
            .insert(StatementNode::Expression(value));
        state_node.statement_nodes = HandleSpan::from_parts(statement, 1);
        let facts = program
            .proof_facts
            .insert_many([ProofFact::Expression(condition)]);
        program.push_state_contract(
            &mut state_node,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts,
                ..SignatureContract::default()
            },
        );
        let mut machine_node = Machine {
            symbol: machine,
            name: Identifier::generated("Main"),
            ..Machine::default()
        };
        program.push_machine_state(&mut machine_node, state_node);
        program.push_machine(machine_node);
        ReturnProgram {
            program,
            machine,
            state,
            carrier,
            value,
            condition,
        }
    }

    fn return_obligation(
        plan: &mut ProofPlan,
        fixture: &ReturnProgram,
    ) -> BoundedStateReturnObligation {
        let value_constraints =
            plan.type_constraints
                .insert_many([ProofConstraint::IntegerRange {
                    minimum: BigInt::from_i64(0),
                    maximum: BigInt::from_i64(255),
                }]);
        BoundedStateReturnObligation {
            machine_symbol: fixture.machine,
            machine: Identifier::generated("Main"),
            state_symbol: fixture.state,
            state: Identifier::generated("entry"),
            statement_index: 0,
            value: fixture.value,
            value_constraints,
            base_type: fixture.carrier,
            constraints: HandleSpan::empty(),
            binary_operands: None,
        }
    }

    #[test]
    fn arrival_interval_certificate_verifies_under_the_kernel() {
        let fixture = return_program();
        let plan = ProofPlan::new(&fixture.program);
        let certificate = interval_premise_certificate(
            &plan,
            fixture.carrier,
            0,
            200,
            &IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(220),
            },
            71,
        )
        .expect("certificate");
        let fact = certificate.verify().expect("kernel accepts");
        assert!(matches!(
            fact.route,
            AcceptedFactRoute::CertificateDerived { .. }
        ));
    }

    #[test]
    fn arrival_interval_gap_stays_uncovered() {
        let fixture = return_program();
        let plan = ProofPlan::new(&fixture.program);
        // The arrival bound misses the target; the leg emits nothing rather
        // than falling through to declared legs the arrival answer supersedes.
        assert!(
            interval_premise_certificate(
                &plan,
                fixture.carrier,
                0,
                255,
                &IntegerRange {
                    minimum: BigInt::from_i64(0),
                    maximum: BigInt::from_i64(10),
                },
                72,
            )
            .is_none()
        );
    }

    #[test]
    fn contract_condition_contributes_its_literal_facts() {
        let mut fixture = return_program();
        let less = fixture
            .program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(7)));
        let conjunct = binary(
            &mut fixture.program,
            fixture.value,
            BinaryOperator::GreaterOrEqual,
            less,
        );
        let both = binary(
            &mut fixture.program,
            conjunct,
            BinaryOperator::And,
            fixture.condition,
        );
        let plan = ProofPlan::new(&fixture.program);
        let mut facts: Vec<GuardBound> = Vec::new();
        collect_contract_bounds(&plan, fixture.value, both, &mut facts);
        assert!(
            matches!(
                facts.as_slice(),
                [GuardBound { bound: lower, lower: true }, GuardBound { bound: upper, lower: false }]
                    if *lower == BigInt::from_i64(7) && *upper == BigInt::from_i64(200)
            ),
            "pending >= 7 && pending <= 200 contributes `7 <= pending` and `pending <= 200`"
        );
    }

    #[test]
    fn contract_point_exclusion_contributes_no_fact() {
        let mut fixture = return_program();
        let five = fixture
            .program
            .expression_table
            .insert(ExpressionNode::Integer(IntegerLiteral::from_value(5)));
        let exclusion = binary(
            &mut fixture.program,
            fixture.value,
            BinaryOperator::NotEqual,
            five,
        );
        // Equality, by contrast, enters as its antisymmetric pair.
        let equality = binary(
            &mut fixture.program,
            fixture.value,
            BinaryOperator::Equal,
            five,
        );
        let plan = ProofPlan::new(&fixture.program);
        let mut facts: Vec<GuardBound> = Vec::new();
        collect_contract_bounds(&plan, fixture.value, exclusion, &mut facts);
        assert!(facts.is_empty(), "a `!=` exclusion has no `<=` restatement");
        collect_contract_bounds(&plan, fixture.value, equality, &mut facts);
        assert_eq!(facts.len(), 2);
    }

    #[test]
    fn contract_same_place_and_stability_gates() {
        let mut fixture = return_program();
        let foreign = fixture
            .program
            .expression_table
            .insert(ExpressionNode::Name(TableNamePath::default()));
        let projection = fixture
            .program
            .expression_table
            .insert(ExpressionNode::ZeroValue(fixture.carrier));
        let plan = ProofPlan::new(&fixture.program);
        assert!(same_place(&plan, fixture.value, fixture.value));
        assert!(!same_place(&plan, foreign, fixture.value));
        assert!(stable_expression(&plan, fixture.condition));
        assert!(!stable_expression(&plan, projection));
    }

    #[test]
    fn contract_refined_return_certifies_where_the_declared_leg_misses() {
        let fixture = return_program();
        let mut plan = ProofPlan::new(&fixture.program);
        let obligation = return_obligation(&mut plan, &fixture);
        let context = AssignmentRangeContext::new(&plan);
        let machine = &fixture.program.machines()[0];
        let state = &fixture.program.machine_states(machine)[0];
        let certificate = contract_refined_certificate(
            &plan,
            &obligation,
            machine,
            state,
            &context,
            &IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(220),
            },
            73,
        )
        .expect("the `pending <= 200` premise reaches the target");
        // The declared pair plus the condition fact are the cited roster.
        assert_eq!(certificate.assumptions.len(), 3);
        certificate.verify().expect("kernel accepts");
    }

    #[test]
    fn contract_refined_return_gap_stays_uncovered() {
        let fixture = return_program();
        let mut plan = ProofPlan::new(&fixture.program);
        let obligation = return_obligation(&mut plan, &fixture);
        let context = AssignmentRangeContext::new(&plan);
        let machine = &fixture.program.machines()[0];
        let state = &fixture.program.machine_states(machine)[0];
        assert!(
            contract_refined_certificate(
                &plan,
                &obligation,
                machine,
                state,
                &context,
                &IntegerRange {
                    minimum: BigInt::from_i64(0),
                    maximum: BigInt::from_i64(100),
                },
                74,
            )
            .is_none(),
            "neither declared 255 nor the `pending <= 200` fact reaches a [0..=100] target"
        );
    }

    #[test]
    fn state_return_verdict_discharges_the_refined_return() {
        let fixture = return_program();
        let mut plan = ProofPlan::new(&fixture.program);
        let obligation = return_obligation(&mut plan, &fixture);
        let context = AssignmentRangeContext::new(&plan);
        let mut measurements = ProofPlanMeasurements::default();
        let verdict = state_return_integer_verdict(
            &plan,
            &obligation,
            &context,
            &IntegerRange {
                minimum: BigInt::from_i64(0),
                maximum: BigInt::from_i64(220),
            },
            75,
            &mut measurements,
            None,
        );
        assert_eq!(verdict, CertificateVerdict::Certified);
        assert_eq!(measurements.certificate_certified, 1);
    }
}
