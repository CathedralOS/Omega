//! Canonical structural validation for decoded proof bundles.

use super::{
    MAX_CONTENT_TERM_DEPTH, MAX_PROOF_DEPTH, MAX_PROPOSITION_DEPTH, MAX_SCALAR_TERM_DEPTH,
    ProofCodecError,
};
use psi_core::{ContentTerm, EvidenceIdentity, IntegerMathTerm, Proposition, ScalarTerm};
use psi_proof_admission::{EvidenceRoute, ProofNode, ProofRule};
use psi_terminal_verifier::{EvidenceProducerRealization, ProofBundle};

pub(super) fn validate_bundle(bundle: &ProofBundle) -> Result<(), ProofCodecError> {
    let mut previous = None;
    for evidence in &bundle.evidence {
        if previous.is_some_and(|previous| previous >= evidence.obligation) {
            return Err(ProofCodecError::NonCanonicalEvidenceOrder);
        }
        previous = Some(evidence.obligation);
        if let EvidenceRoute::CertificateDerived(certificate) = &evidence.route {
            validate_proof_node(&certificate.proof, 0)?;
        }
    }
    let mut previous_term = None;
    for (index, producer) in bundle.evidence_producers.iter().enumerate() {
        let expected = EvidenceIdentity::new(
            u64::try_from(index)
                .expect("producer provenance count fits u64")
                .checked_add(1)
                .expect("one-based producer provenance identity fits u64"),
        )
        .expect("one-based producer provenance identity is nonzero");
        if producer.id != expected {
            return Err(ProofCodecError::NonCanonicalEvidenceProducerOrder);
        }
        if previous_term.is_some_and(|previous| previous >= producer.term) {
            return Err(ProofCodecError::NonCanonicalEvidenceProducerOrder);
        }
        previous_term = Some(producer.term);
        if producer.conformance_identity.is_empty() || producer.evidence_trait_identity.is_empty() {
            return Err(ProofCodecError::InvalidEvidenceProducer);
        }
        let mut previous_row = None;
        for row in &producer.rows {
            if row.declaring_trait_identity.is_empty()
                || row.requirement_identity.is_empty()
                || row.realization_machine_identity.is_empty()
                || row.realization_state_identity.is_empty()
            {
                return Err(ProofCodecError::InvalidEvidenceProducer);
            }
            if previous_row.is_some_and(|previous: &EvidenceProducerRealization| previous >= row) {
                return Err(ProofCodecError::NonCanonicalEvidenceProducerRows);
            }
            previous_row = Some(row);
        }
    }
    Ok(())
}

fn validate_proof_node(node: &ProofNode, depth: usize) -> Result<(), ProofCodecError> {
    if depth > MAX_PROOF_DEPTH {
        return Err(ProofCodecError::ProofNestingTooDeep);
    }
    validate_proposition(&node.conclusion, 0)?;
    match &node.rule {
        ProofRule::Primitive(_)
        | ProofRule::SemanticAxiom { .. }
        | ProofRule::Assumption { .. } => Ok(()),
        ProofRule::ConjunctionIntroduction(nodes) => {
            for node in nodes {
                validate_proof_node(node, depth + 1)?;
            }
            Ok(())
        }
        ProofRule::ConjunctionElimination { conjunction, .. }
        | ProofRule::ImplicationIntroduction { body: conjunction } => {
            validate_proof_node(conjunction, depth + 1)
        }
        ProofRule::DisjunctionIntroduction { disjunct, .. } => {
            validate_proof_node(disjunct, depth + 1)
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            validate_proof_node(implication, depth + 1)?;
            validate_proof_node(premise, depth + 1)
        }
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        }
        | ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: left_equals_middle,
            middle_less_or_equal_right: middle_equals_right,
        }
        | ProofRule::IntegerLessOrEqualSubstitution {
            relation: left_equals_middle,
            equality: middle_equals_right,
            ..
        } => {
            validate_proof_node(left_equals_middle, depth + 1)?;
            validate_proof_node(middle_equals_right, depth + 1)
        }
        ProofRule::IntegerAffineBound {
            root_bound,
            witness,
        } => {
            validate_proof_node(root_bound, depth + 1)?;
            validate_scalar_term_depth(&witness.root, 0)?;
            validate_scalar_term_depth(&witness.target, 0)
        }
        ProofRule::IntegerCastBound {
            root_bound,
            witness,
        } => {
            validate_proof_node(root_bound, depth + 1)?;
            validate_scalar_term_depth(&witness.root, 0)?;
            validate_scalar_term_depth(&witness.target, 0)
        }
        ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
            validate_scalar_term_depth(&witness.dividend.root, 0)?;
            validate_scalar_term_depth(&witness.dividend.target, 0)?;
            validate_scalar_term_depth(&witness.divisor.root, 0)?;
            validate_scalar_term_depth(&witness.divisor.target, 0)?;
            validate_proposition(&witness.conclusion, 0)
        }
    }
}

fn validate_proposition(proposition: &Proposition, depth: usize) -> Result<(), ProofCodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(ProofCodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {}
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            validate_scalar_term_depth(left, 0)?;
            validate_scalar_term_depth(right, 0)?;
        }
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            validate_integer_math_term_depth(left, 0)?;
            validate_integer_math_term_depth(right, 0)?;
        }
        Proposition::IeeeFloatComparison { .. }
        | Proposition::ByteSequenceEqual { .. }
        | Proposition::StructuralCaseMembership { .. } => {}
        Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
            for proposition in propositions {
                validate_proposition(proposition, depth + 1)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            validate_proposition(premise, depth + 1)?;
            validate_proposition(conclusion, depth + 1)?;
        }
        Proposition::ContentConservation(conservation) => {
            validate_content_term_depth(conservation.left(), 0)?;
            validate_content_term_depth(conservation.right(), 0)?;
        }
    }
    proposition
        .validate()
        .map_err(ProofCodecError::MalformedProposition)
}

fn validate_integer_math_term_depth(
    term: &IntegerMathTerm,
    depth: usize,
) -> Result<(), ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    match term {
        IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_) => {}
        IntegerMathTerm::Add(left, right)
        | IntegerMathTerm::Subtract(left, right)
        | IntegerMathTerm::Multiply(left, right) => {
            validate_integer_math_term_depth(left, depth + 1)?;
            validate_integer_math_term_depth(right, depth + 1)?;
        }
        IntegerMathTerm::ShiftLeft { value, count } => {
            validate_integer_math_term_depth(value, depth + 1)?;
            validate_integer_math_term_depth(count, depth + 1)?;
        }
    }
    Ok(())
}

fn validate_content_term_depth(term: &ContentTerm, depth: usize) -> Result<(), ProofCodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(ProofCodecError::ContentTermNestingTooDeep);
    }
    if let ContentTerm::Separate(terms) = term {
        for term in terms {
            validate_content_term_depth(term, depth + 1)?;
        }
    }
    Ok(())
}

fn validate_scalar_term_depth(term: &ScalarTerm, depth: usize) -> Result<(), ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => {
            validate_scalar_term_depth(operand, depth + 1)?;
        }
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. }
        | ScalarTerm::ExactIntegerDivide { left, right, .. }
        | ScalarTerm::ExactIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerDivide { left, right, .. }
        | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
        | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
        | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            validate_scalar_term_depth(left, depth + 1)?;
            validate_scalar_term_depth(right, depth + 1)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            validate_scalar_term_depth(value, depth + 1)?;
            validate_scalar_term_depth(count, depth + 1)?;
        }
        ScalarTerm::Value { .. }
        | ScalarTerm::BooleanField { .. }
        | ScalarTerm::IntegerField { .. }
        | ScalarTerm::Boolean(_)
        | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}
