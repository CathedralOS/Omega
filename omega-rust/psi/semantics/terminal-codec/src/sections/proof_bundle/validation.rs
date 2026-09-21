//! Canonical structural validation for decoded proof bundles.

use super::ProofCodecError;
use crate::sections::proof_bundle::proof_node_codec::MAX_PROOF_DEPTH;
use crate::sections::proof_bundle::proposition_codec::{
    MAX_CONTENT_TERM_DEPTH, MAX_PROPOSITION_DEPTH,
};
use crate::sections::proof_bundle::scalar_term_codec::MAX_SCALAR_TERM_DEPTH;
use proof_admission::{EvidenceRoute, ProofNode, ProofRule};
use semantic_vocabulary::{
    ContentTerm, EvidenceIdentity, IntegerMathTerm, Proposition, ScalarTerm,
};
use terminal_verifier::{EvidenceProducerRealization, ProofBundle};

pub(super) fn validate_bundle(bundle: &ProofBundle) -> Result<(), ProofCodecError> {
    let mut previous = None;
    for evidence in &bundle.evidence {
        if previous.is_some_and(|previous| previous >= evidence.obligation) {
            return Err(ProofCodecError::NonCanonicalEvidenceOrder);
        }
        previous = Some(evidence.obligation);
        validate_evidence_route(&evidence.route)?;
    }
    let mut previous_component = None;
    for component in &bundle.recursive_components {
        if previous_component.is_some_and(|previous| previous >= component.component) {
            return Err(ProofCodecError::NonCanonicalRecursiveComponentEvidence);
        }
        previous_component = Some(component.component);
        validate_evidence_route(&component.certificate.well_foundedness)?;
        let mut previous_edge = None;
        for edge in &component.certificate.edges {
            if previous_edge.is_some_and(|previous| previous >= edge.obligation) {
                return Err(ProofCodecError::NonCanonicalRecursiveComponentEvidence);
            }
            previous_edge = Some(edge.obligation);
            validate_evidence_route(&edge.evidence)?;
        }
    }
    let mut previous_component = None;
    for component in &bundle.control_cycles {
        if previous_component.is_some_and(|previous| previous >= component.component) {
            return Err(ProofCodecError::NonCanonicalControlCycleEvidence);
        }
        previous_component = Some(component.component);
        validate_evidence_route(&component.certificate.well_foundedness)?;
        let mut previous_edge = None;
        for edge in &component.certificate.edges {
            if previous_edge.is_some_and(|previous| previous >= edge.obligation) {
                return Err(ProofCodecError::NonCanonicalControlCycleEvidence);
            }
            previous_edge = Some(edge.obligation);
            validate_evidence_route(&edge.evidence)?;
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

fn validate_evidence_route(route: &EvidenceRoute) -> Result<(), ProofCodecError> {
    if let EvidenceRoute::CertificateDerived(certificate) = route {
        validate_proof_node(&certificate.proof)?;
    }
    Ok(())
}

fn validate_proof_node(node: &ProofNode) -> Result<(), ProofCodecError> {
    // The proof depth guard walks its tree through an explicit worklist rather
    // than the call stack, matching the canonical-order guards in
    // `semantic_module/canonical_order.rs`: a deeply nested proof must reach
    // the depth bound and reject instead of overflowing the stack on hosts
    // with small thread stacks. `ScalarTerm` steps keep the witness-term
    // checks in the same depth-first order the recursive validator used.
    enum Step<'a> {
        Node(&'a ProofNode, usize),
        ScalarTerm(&'a ScalarTerm),
    }
    let mut pending = vec![Step::Node(node, 0)];
    while let Some(step) = pending.pop() {
        match step {
            Step::Node(node, depth) => {
                if depth > MAX_PROOF_DEPTH {
                    return Err(ProofCodecError::ProofNestingTooDeep);
                }
                validate_proposition(&node.conclusion)?;
                match &node.rule {
                    ProofRule::Primitive(_)
                    | ProofRule::SemanticAxiom { .. }
                    | ProofRule::Assumption { .. } => {}
                    ProofRule::ConjunctionIntroduction(nodes) => {
                        for node in nodes.iter().rev() {
                            pending.push(Step::Node(node, depth + 1));
                        }
                    }
                    ProofRule::ValueEqualityTransport {
                        premise,
                        equalities,
                    } => {
                        for equality in equalities.iter().rev() {
                            pending.push(Step::Node(equality, depth + 1));
                        }
                        pending.push(Step::Node(premise, depth + 1));
                    }
                    ProofRule::PredicateDenotation {
                        premise: conjunction,
                    }
                    | ProofRule::EqualitySymmetry {
                        equality: conjunction,
                    }
                    | ProofRule::IntegerOrderWeakening {
                        relation: conjunction,
                    }
                    | ProofRule::IntegerOrderDiscreteness {
                        relation: conjunction,
                    }
                    | ProofRule::ConjunctionElimination { conjunction, .. }
                    | ProofRule::ImplicationIntroduction { body: conjunction } => {
                        pending.push(Step::Node(conjunction, depth + 1));
                    }
                    ProofRule::DisjunctionIntroduction { disjunct, .. } => {
                        pending.push(Step::Node(disjunct, depth + 1));
                    }
                    ProofRule::DisjunctionElimination {
                        disjunction,
                        branches,
                    } => {
                        for branch in branches.iter().rev() {
                            pending.push(Step::Node(branch, depth + 1));
                        }
                        pending.push(Step::Node(disjunction, depth + 1));
                    }
                    ProofRule::IntegerSubtractOrder {
                        difference,
                        positive,
                    } => {
                        pending.push(Step::Node(positive, depth + 1));
                        pending.push(Step::Node(difference, depth + 1));
                    }
                    ProofRule::ImplicationElimination {
                        implication,
                        premise,
                    } => {
                        pending.push(Step::Node(premise, depth + 1));
                        pending.push(Step::Node(implication, depth + 1));
                    }
                    ProofRule::EqualityTransitivity {
                        left_equals_middle,
                        middle_equals_right,
                    }
                    | ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: left_equals_middle,
                        middle_less_or_equal_right: middle_equals_right,
                    }
                    | ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle: left_equals_middle,
                        middle_to_right: middle_equals_right,
                    }
                    | ProofRule::IntegerOrderSubstitution {
                        relation: left_equals_middle,
                        equality: middle_equals_right,
                        ..
                    }
                    | ProofRule::IntegerExactAddDefinitionBound {
                        left_bound: left_equals_middle,
                        right_bound: middle_equals_right,
                        ..
                    } => {
                        pending.push(Step::Node(middle_equals_right, depth + 1));
                        pending.push(Step::Node(left_equals_middle, depth + 1));
                    }
                    ProofRule::IntegerAffineBound {
                        root_bound,
                        witness,
                    } => {
                        pending.push(Step::ScalarTerm(&witness.target));
                        pending.push(Step::ScalarTerm(&witness.root));
                        pending.push(Step::Node(root_bound, depth + 1));
                    }
                    ProofRule::IntegerCastBound {
                        root_bound,
                        witness,
                    } => {
                        pending.push(Step::ScalarTerm(&witness.target));
                        pending.push(Step::ScalarTerm(&witness.root));
                        pending.push(Step::Node(root_bound, depth + 1));
                    }
                    ProofRule::IntegerCorrelatedForbiddenRoots { witness } => {
                        validate_scalar_term_depth(&witness.dividend.root)?;
                        validate_scalar_term_depth(&witness.dividend.target)?;
                        validate_scalar_term_depth(&witness.divisor.root)?;
                        validate_scalar_term_depth(&witness.divisor.target)?;
                        validate_proposition(&witness.conclusion)?;
                    }
                }
            }
            Step::ScalarTerm(term) => validate_scalar_term_depth(term)?,
        }
    }
    Ok(())
}

fn validate_proposition(proposition: &Proposition) -> Result<(), ProofCodecError> {
    // Same stack-safety contract as the proof-node guard: nested conjunctions,
    // disjunctions, and implications walk an explicit worklist. `Finalize`
    // steps keep each proposition's own `validate()` in the post-order
    // position the recursive validator gave it, after its children.
    enum Step<'a> {
        Validate(&'a Proposition, usize),
        Finalize(&'a Proposition),
    }
    let mut pending = vec![Step::Validate(proposition, 0)];
    while let Some(step) = pending.pop() {
        match step {
            Step::Validate(proposition, depth) => {
                if depth > MAX_PROPOSITION_DEPTH {
                    return Err(ProofCodecError::PropositionNestingTooDeep);
                }
                match proposition {
                    Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => {
                        pending.push(Step::Finalize(proposition));
                    }
                    Proposition::Equal(left, right)
                    | Proposition::LessThan(left, right)
                    | Proposition::LessOrEqual(left, right)
                    | Proposition::ScalarIeeeFloatComparison { left, right, .. } => {
                        validate_scalar_term_depth(left)?;
                        validate_scalar_term_depth(right)?;
                        pending.push(Step::Finalize(proposition));
                    }
                    Proposition::IntegerMathEqual(left, right)
                    | Proposition::IntegerMathLessThan(left, right)
                    | Proposition::IntegerMathLessOrEqual(left, right) => {
                        validate_integer_math_term_depth(left)?;
                        validate_integer_math_term_depth(right)?;
                        pending.push(Step::Finalize(proposition));
                    }
                    Proposition::IeeeFloatComparison { .. }
                    | Proposition::ByteSequenceEqual { .. }
                    | Proposition::StructuralCaseMembership { .. } => {
                        pending.push(Step::Finalize(proposition));
                    }
                    Proposition::Conjunction(propositions)
                    | Proposition::Disjunction(propositions) => {
                        pending.push(Step::Finalize(proposition));
                        for proposition in propositions.iter().rev() {
                            pending.push(Step::Validate(proposition, depth + 1));
                        }
                    }
                    Proposition::Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(Step::Finalize(proposition));
                        pending.push(Step::Validate(conclusion, depth + 1));
                        pending.push(Step::Validate(premise, depth + 1));
                    }
                    Proposition::ContentConservation(conservation) => {
                        validate_content_term_depth(conservation.left())?;
                        validate_content_term_depth(conservation.right())?;
                        pending.push(Step::Finalize(proposition));
                    }
                }
            }
            Step::Finalize(proposition) => proposition
                .validate()
                .map_err(ProofCodecError::MalformedProposition)?,
        }
    }
    Ok(())
}

fn validate_integer_math_term_depth(term: &IntegerMathTerm) -> Result<(), ProofCodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(ProofCodecError::ScalarTermNestingTooDeep);
        }
        match term {
            IntegerMathTerm::MathValue { .. } | IntegerMathTerm::IntegerLiteral(_) => {}
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right) => {
                pending.push((right, depth + 1));
                pending.push((left, depth + 1));
            }
            IntegerMathTerm::ShiftLeft { value, count } => {
                pending.push((count, depth + 1));
                pending.push((value, depth + 1));
            }
        }
    }
    Ok(())
}

fn validate_content_term_depth(term: &ContentTerm) -> Result<(), ProofCodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_CONTENT_TERM_DEPTH {
            return Err(ProofCodecError::ContentTermNestingTooDeep);
        }
        if let ContentTerm::Separate(terms) = term {
            for term in terms.iter().rev() {
                pending.push((term, depth + 1));
            }
        }
    }
    Ok(())
}

fn validate_scalar_term_depth(term: &ScalarTerm) -> Result<(), ProofCodecError> {
    let mut pending = vec![(term, 0_usize)];
    while let Some((term, depth)) = pending.pop() {
        if depth > MAX_SCALAR_TERM_DEPTH {
            return Err(ProofCodecError::ScalarTermNestingTooDeep);
        }
        match term {
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => {
                pending.push((operand, depth + 1));
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
                pending.push((right, depth + 1));
                pending.push((left, depth + 1));
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                pending.push((count, depth + 1));
                pending.push((value, depth + 1));
            }
            ScalarTerm::Value { .. }
            | ScalarTerm::BooleanField { .. }
            | ScalarTerm::IntegerField { .. }
            | ScalarTerm::Boolean(_)
            | ScalarTerm::Integer { .. } => {}
        }
    }
    Ok(())
}
