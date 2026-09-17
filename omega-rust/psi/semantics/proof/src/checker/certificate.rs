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
//!   chain from the target's endpoints through the declared bounds.
//!
//! Guard narrowing, dependent bounds, sibling-length atoms, arrival-bound
//! returns and the floating legs are not covered; those obligations keep the
//! existing derivation.

use arena::HandleSpan;
use numerics::bignum::BigInt;
use proof_admission::{
    AcceptedFact, AdmissionProfile, CertificateEnvelope, EvidenceError, EvidenceRoute, Obligation,
    ObligationClass, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker, verify_obligation,
};
use semantic_vocabulary::{
    EvidenceIdentity, IntegerMathLiteral, IntegerMathTerm, IntegerSign, IntegerType, ObligationId,
    Proposition, PropositionContext, ScalarType, ValueId,
};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

use crate::checker::integer_ranges::{
    integer_literal_handle, integer_range_from_constraints, type_constraints,
};
use crate::obligations::{BoundedStateReturnObligation, IntegerRange, ProofConstraint, ProofPlan};

/// Outcome of the certificate route for one bounded integer value leg.
pub(crate) enum CertificateVerdict {
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
struct BoundedValueCertificate {
    context: PropositionContext,
    obligation: Obligation,
    assumptions: Vec<Proposition>,
    envelope: CertificateEnvelope,
}

impl BoundedValueCertificate {
    /// Independently re-decide this certificate through the admission kernel.
    /// Producer success never counts; only the kernel's verdict does.
    fn verify(&self) -> Result<AcceptedFact, EvidenceError> {
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
) -> CertificateVerdict {
    let Some(certificate) = bounded_integer_value(
        proof_plan,
        value,
        value_constraints,
        base_type,
        target,
        seed,
        anonymous,
    ) else {
        return CertificateVerdict::Uncovered;
    };
    match certificate.verify() {
        Ok(_) => CertificateVerdict::Certified,
        Err(_) => CertificateVerdict::Rejected,
    }
}

/// Certificate route for a bounded state return. The arrival machinery owns
/// the verdict whenever it applies -- its joined bounds and authored
/// assumptions are richer evidence than the declared interval this producer
/// cites -- so a certificate is emitted only when `return_arrival` would
/// reach the declared path itself: the statement-identity gate must hold and
/// the arrival arithmetic query must return no bounds.
pub(crate) fn state_return_integer_verdict(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target: &IntegerRange,
    seed: u64,
) -> CertificateVerdict {
    let Some(certificate) = state_return_certificate(proof_plan, obligation, target, seed) else {
        return CertificateVerdict::Uncovered;
    };
    match certificate.verify() {
        Ok(_) => CertificateVerdict::Certified,
        Err(_) => CertificateVerdict::Rejected,
    }
}

fn state_return_certificate(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target: &IntegerRange,
    seed: u64,
) -> Option<BoundedValueCertificate> {
    if return_arrival_owns_verdict(proof_plan, obligation) {
        return None;
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
}

/// Whether `return_arrival::integer_range` decides this leg through anything
/// other than the declared-or-literal value interval: a malformed obligation
/// (its verdict is the checker diagnostic, not a certificate question) or a
/// bound the arrival query already computed. Mirrors the front of
/// `return_arrival::integer_range`.
fn return_arrival_owns_verdict(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> bool {
    let program = proof_plan.program;
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == obligation.machine_symbol)
    else {
        return true;
    };
    let Some(state) = program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == obligation.state_symbol)
    else {
        return true;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    if !matches!(
        statements.get(obligation.statement_index),
        Some(StatementNode::Expression(value)) if *value == obligation.value
    ) {
        return true;
    }
    validation::arrival_integer_expression_bounds(
        program,
        obligation.machine_symbol,
        obligation.state_symbol,
        obligation.statement_index,
        obligation.value,
    )
    .is_some()
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
