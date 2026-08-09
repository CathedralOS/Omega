use psi_core::{
    ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    IntegerCarrier, IntegerSign, IntegerType, IntegerValue, Proposition, PropositionError,
    PropositionId, PsiSemanticId, ScalarTerm, ScalarType,
};
use psi_proof_kernel::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, PrimitiveJudgment,
    ProofNode, ProofRule, ProofSystemVersion,
};
use psi_terminal_verifier::{ObligationEvidence, ProofBundle};
use sha2::{Digest, Sha256};

const MAGIC: &[u8; 8] = b"PSIPRF\0\0";
const FORMAT_VERSION_V1: u16 = 1;
const FORMAT_VERSION_V2: u16 = 2;
const FORMAT_VERSION_V3: u16 = 3;
const FORMAT_VERSION_V4: u16 = 4;
const FORMAT_VERSION_V5: u16 = 5;
const FORMAT_VERSION_V6: u16 = 6;
const FORMAT_VERSION_V7: u16 = 7;
const FORMAT_VERSION_V8: u16 = 8;
const FORMAT_VERSION_V9: u16 = 9;
const FORMAT_VERSION_V10: u16 = 10;
const FORMAT_VERSION_V11: u16 = 11;
const FORMAT_VERSION_V12: u16 = 12;
const FORMAT_VERSION_V13: u16 = 13;
const FORMAT_VERSION_V14: u16 = 14;
const FORMAT_VERSION_V15: u16 = 15;
const FORMAT_VERSION_V16: u16 = 16;
const FORMAT_VERSION_V17: u16 = 17;
const FORMAT_VERSION_V18: u16 = 18;
const FORMAT_VERSION_V19: u16 = 19;
const FORMAT_VERSION_V20: u16 = 20;
const FORMAT_VERSION_V21: u16 = 21;
const FORMAT_VERSION_V22: u16 = 22;
const FORMAT_VERSION_V23: u16 = 23;
const FORMAT_VERSION_V24: u16 = 24;
const FORMAT_VERSION_V25: u16 = 25;
const FORMAT_VERSION_V26: u16 = 26;
const FORMAT_VERSION_V27: u16 = 27;
const FORMAT_VERSION_V28: u16 = 28;
const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-proof-bundle-fingerprint-v1\0";
const MAX_PROPOSITION_DEPTH: usize = 256;
const MAX_SCALAR_TERM_DEPTH: usize = 256;
const MAX_CONTENT_TERM_DEPTH: usize = 256;
const MAX_PROOF_DEPTH: usize = 256;
const MAX_CONTENT_IDENTITY_BYTES: usize = 1 << 20;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProofBundleFingerprint([u8; 32]);

impl ProofBundleFingerprint {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for ProofBundleFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, formatter)
    }
}

impl std::fmt::Display for ProofBundleFingerprint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

pub fn encode_proof_bundle(bundle: &ProofBundle) -> Result<Vec<u8>, ProofCodecError> {
    validate_bundle(bundle)?;
    encode_raw(bundle, required_format_version(bundle))
}

pub fn decode_proof_bundle(bytes: &[u8]) -> Result<ProofBundle, ProofCodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(ProofCodecError::InvalidMagic);
    }
    let format_version = reader.u16()?;
    if !matches!(
        format_version,
        FORMAT_VERSION_V1
            | FORMAT_VERSION_V2
            | FORMAT_VERSION_V3
            | FORMAT_VERSION_V4
            | FORMAT_VERSION_V5
            | FORMAT_VERSION_V6
            | FORMAT_VERSION_V7
            | FORMAT_VERSION_V8
            | FORMAT_VERSION_V9
            | FORMAT_VERSION_V10
            | FORMAT_VERSION_V11
            | FORMAT_VERSION_V12
            | FORMAT_VERSION_V13
            | FORMAT_VERSION_V14
            | FORMAT_VERSION_V15
            | FORMAT_VERSION_V16
            | FORMAT_VERSION_V17
            | FORMAT_VERSION_V18
            | FORMAT_VERSION_V19
            | FORMAT_VERSION_V20
            | FORMAT_VERSION_V21
            | FORMAT_VERSION_V22
            | FORMAT_VERSION_V23
            | FORMAT_VERSION_V24
            | FORMAT_VERSION_V25
            | FORMAT_VERSION_V26
            | FORMAT_VERSION_V27
            | FORMAT_VERSION_V28
    ) {
        return Err(ProofCodecError::UnsupportedFormatVersion(format_version));
    }
    let evidence_count = reader.count()?;
    let mut evidence = Vec::new();
    for _ in 0..evidence_count {
        evidence.push(decode_evidence(&mut reader, format_version)?);
    }
    if reader.remaining() != 0 {
        return Err(ProofCodecError::TrailingBytes(reader.remaining()));
    }
    let bundle = ProofBundle { evidence };
    validate_bundle(&bundle)?;
    if required_format_version(&bundle) != format_version
        || encode_raw(&bundle, format_version)? != bytes
    {
        return Err(ProofCodecError::NonCanonicalEncoding);
    }
    Ok(bundle)
}

pub fn proof_bundle_fingerprint(
    bundle: &ProofBundle,
) -> Result<ProofBundleFingerprint, ProofCodecError> {
    let bytes = encode_proof_bundle(bundle)?;
    let mut digest = Sha256::new();
    digest.update(FINGERPRINT_DOMAIN);
    let byte_len = u64::try_from(bytes.len()).expect("proof-bundle bytes fit the digest domain");
    digest.update(byte_len.to_le_bytes());
    digest.update(bytes);
    Ok(ProofBundleFingerprint(digest.finalize().into()))
}

fn validate_bundle(bundle: &ProofBundle) -> Result<(), ProofCodecError> {
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
        } => {
            validate_proof_node(left_equals_middle, depth + 1)?;
            validate_proof_node(middle_equals_right, depth + 1)
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
        Proposition::Conjunction(conjuncts) => {
            for conjunct in conjuncts {
                validate_proposition(conjunct, depth + 1)?;
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
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
    }
    Ok(())
}

fn required_format_version(bundle: &ProofBundle) -> u16 {
    if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v28_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V28
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v27_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V27
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v26_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V26
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v25_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V25
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v24_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V24
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v23_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V23
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v22_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V22
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v21_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V21
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v20_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V20
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v19_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V19
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v18_address(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V18
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v17_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V17
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v16_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V16
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v15_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V15
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v14_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V14
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v13_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V13
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v12_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V12
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v11_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V11
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v10_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V10
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v9_content_case(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V9
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v8_content(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V8
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v7_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V7
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v6_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V6
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v5_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V5
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v4_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V4
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v3_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V3
    } else if bundle.evidence.iter().any(|evidence| {
        matches!(
            &evidence.route,
            EvidenceRoute::CertificateDerived(certificate)
                if proof_uses_v2_term(&certificate.proof)
        )
    }) {
        FORMAT_VERSION_V2
    } else {
        FORMAT_VERSION_V1
    }
}

fn proof_uses_v28_term(node: &ProofNode) -> bool {
    proposition_uses_v28_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v28_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v28_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v28_term(implication) || proof_uses_v28_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v28_term(left_equals_middle) || proof_uses_v28_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v28_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v28(left) || scalar_term_uses_v28(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v28_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v28_term(premise) || proposition_uses_v28_term(conclusion),
    }
}

fn scalar_term_uses_v28(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v28(operand),
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
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v28(left) || scalar_term_uses_v28(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v28(value) || scalar_term_uses_v28(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v27_term(node: &ProofNode) -> bool {
    proposition_uses_v27_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v27_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v27_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v27_term(implication) || proof_uses_v27_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v27_term(left_equals_middle) || proof_uses_v27_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v27_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v27(left) || scalar_term_uses_v27(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v27_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v27_term(premise) || proposition_uses_v27_term(conclusion),
    }
}

fn scalar_term_uses_v27(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v27(operand),
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
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v27(left) || scalar_term_uses_v27(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v27(value) || scalar_term_uses_v27(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v26_term(node: &ProofNode) -> bool {
    proposition_uses_v26_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v26_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v26_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v26_term(implication) || proof_uses_v26_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v26_term(left_equals_middle) || proof_uses_v26_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v26_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v26(left) || scalar_term_uses_v26(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v26_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v26_term(premise) || proposition_uses_v26_term(conclusion),
    }
}

fn scalar_term_uses_v26(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v26(operand),
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
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v26(left) || scalar_term_uses_v26(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v26(value) || scalar_term_uses_v26(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v25_term(node: &ProofNode) -> bool {
    proposition_uses_v25_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v25_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v25_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v25_term(implication) || proof_uses_v25_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v25_term(left_equals_middle) || proof_uses_v25_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v25_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v25(left) || scalar_term_uses_v25(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v25_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v25_term(premise) || proposition_uses_v25_term(conclusion),
    }
}

fn scalar_term_uses_v25(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v25(operand),
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
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v25(left) || scalar_term_uses_v25(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v25(value) || scalar_term_uses_v25(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v24_term(node: &ProofNode) -> bool {
    proposition_uses_v24_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v24_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v24_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v24_term(implication) || proof_uses_v24_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v24_term(left_equals_middle) || proof_uses_v24_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v24_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v24(left) || scalar_term_uses_v24(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v24_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v24_term(premise) || proposition_uses_v24_term(conclusion),
    }
}

fn scalar_term_uses_v24(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v24(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v24(left) || scalar_term_uses_v24(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v24(value) || scalar_term_uses_v24(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v23_term(node: &ProofNode) -> bool {
    proposition_uses_v23_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v23_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v23_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v23_term(implication) || proof_uses_v23_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v23_term(left_equals_middle) || proof_uses_v23_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v23_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v23(left) || scalar_term_uses_v23(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v23_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v23_term(premise) || proposition_uses_v23_term(conclusion),
    }
}

fn scalar_term_uses_v23(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v23(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v23(left) || scalar_term_uses_v23(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v23(value) || scalar_term_uses_v23(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v22_term(node: &ProofNode) -> bool {
    proposition_uses_v22_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v22_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v22_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v22_term(implication) || proof_uses_v22_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v22_term(left_equals_middle) || proof_uses_v22_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v22_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v22(left) || scalar_term_uses_v22(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v22_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v22_term(premise) || proposition_uses_v22_term(conclusion),
    }
}

fn scalar_term_uses_v22(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v22(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v22(left) || scalar_term_uses_v22(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v22(value) || scalar_term_uses_v22(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v21_term(node: &ProofNode) -> bool {
    proposition_uses_v21_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v21_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v21_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v21_term(implication) || proof_uses_v21_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v21_term(left_equals_middle) || proof_uses_v21_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v21_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v21(left) || scalar_term_uses_v21(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v21_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v21_term(premise) || proposition_uses_v21_term(conclusion),
    }
}

fn scalar_term_uses_v21(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v21(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v21(left) || scalar_term_uses_v21(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v21(value) || scalar_term_uses_v21(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v20_term(node: &ProofNode) -> bool {
    proposition_uses_v20_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v20_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v20_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v20_term(implication) || proof_uses_v20_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v20_term(left_equals_middle) || proof_uses_v20_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v20_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v20(left) || scalar_term_uses_v20(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v20_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v20_term(premise) || proposition_uses_v20_term(conclusion),
    }
}

fn scalar_term_uses_v20(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftRight { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v20(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v20(left) || scalar_term_uses_v20(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. } => {
            scalar_term_uses_v20(value) || scalar_term_uses_v20(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v18_address(node: &ProofNode) -> bool {
    proposition_uses_address(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v18_address),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v18_address(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v18_address(implication) || proof_uses_v18_address(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v18_address(left_equals_middle)
                    || proof_uses_v18_address(middle_equals_right)
            }
        }
}

fn proof_uses_v19_term(node: &ProofNode) -> bool {
    proposition_uses_v19_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v19_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v19_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v19_term(implication) || proof_uses_v19_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v19_term(left_equals_middle) || proof_uses_v19_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v19_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v19(left) || scalar_term_uses_v19(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v19_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v19_term(premise) || proposition_uses_v19_term(conclusion),
    }
}

fn scalar_term_uses_v19(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::IntegerExactCast { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. } => scalar_term_uses_v19(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v19(left) || scalar_term_uses_v19(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
        | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v19(value) || scalar_term_uses_v19(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proposition_uses_address(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_address(left) || scalar_term_uses_address(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_address),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_address(premise) || proposition_uses_address(conclusion),
    }
}

fn integer_type_is_address(integer_type: IntegerType) -> bool {
    integer_type.carrier() == IntegerCarrier::Address
}

fn scalar_type_uses_address(scalar_type: ScalarType) -> bool {
    matches!(scalar_type, ScalarType::Integer(integer_type) if integer_type_is_address(integer_type))
}

fn scalar_term_uses_address(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::Value { scalar_type, .. } => scalar_type_uses_address(*scalar_type),
        ScalarTerm::Boolean(_) => false,
        ScalarTerm::BooleanNot { operand } => scalar_term_uses_address(operand),
        ScalarTerm::BooleanEqual { left, right } => {
            scalar_term_uses_address(left) || scalar_term_uses_address(right)
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            integer_type_is_address(*source_type)
                || integer_type_is_address(*target_type)
                || scalar_term_uses_address(operand)
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            integer_type_is_address(*source_type)
                || integer_type_is_address(*target_type)
                || scalar_term_uses_address(operand)
        }
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        }
        | ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        }
        | ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        }
        | ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            integer_type_is_address(*value_type)
                || integer_type_is_address(*count_type)
                || scalar_term_uses_address(value)
                || scalar_term_uses_address(count)
        }
        ScalarTerm::Integer { scalar_type, .. }
        | ScalarTerm::IntegerBitwiseNot { scalar_type, .. } => {
            integer_type_is_address(*scalar_type)
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            integer_type_is_address(*scalar_type)
                || scalar_term_uses_address(left)
                || scalar_term_uses_address(right)
        }
    }
}

fn proof_uses_v17_term(node: &ProofNode) -> bool {
    proposition_uses_v17_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v17_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v17_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v17_term(implication) || proof_uses_v17_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v17_term(left_equals_middle) || proof_uses_v17_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v17_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v17(left) || scalar_term_uses_v17(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v17_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v17_term(premise) || proposition_uses_v17_term(conclusion),
    }
}

fn scalar_term_uses_v17(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::IntegerWiden { .. } => true,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v17(value) || scalar_term_uses_v17(count)
        }
        ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v17(operand),
        ScalarTerm::BooleanNot { operand } | ScalarTerm::IntegerBitwiseNot { operand, .. } => {
            scalar_term_uses_v17(operand)
        }
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v17(left) || scalar_term_uses_v17(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v17(value) || scalar_term_uses_v17(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v16_term(node: &ProofNode) -> bool {
    proposition_uses_v16_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v16_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v16_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v16_term(implication) || proof_uses_v16_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v16_term(left_equals_middle) || proof_uses_v16_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v16_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v16(left) || scalar_term_uses_v16(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v16_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v16_term(premise) || proposition_uses_v16_term(conclusion),
    }
}

fn scalar_term_uses_v16(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::IntegerBitwiseNot { .. } => true,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v16(value) || scalar_term_uses_v16(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v16(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v16(left) || scalar_term_uses_v16(right)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v16(value) || scalar_term_uses_v16(count)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v15_term(node: &ProofNode) -> bool {
    proposition_uses_v15_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v15_term),
            ProofRule::ConjunctionElimination { conjunction, .. }
            | ProofRule::ImplicationIntroduction { body: conjunction } => {
                proof_uses_v15_term(conjunction)
            }
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v15_term(implication) || proof_uses_v15_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v15_term(left_equals_middle) || proof_uses_v15_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v15_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v15(left) || scalar_term_uses_v15(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v15_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v15_term(premise) || proposition_uses_v15_term(conclusion),
    }
}

fn scalar_term_uses_v15(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::WrappingIntegerShiftLeft { .. }
        | ScalarTerm::WrappingIntegerShiftRight { .. } => true,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v15(value) || scalar_term_uses_v15(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v15(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v15(left) || scalar_term_uses_v15(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v14_term(node: &ProofNode) -> bool {
    proposition_uses_v14_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v14_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v14_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v14_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v14_term(implication) || proof_uses_v14_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v14_term(left_equals_middle) || proof_uses_v14_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v14_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v14(left) || scalar_term_uses_v14(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v14_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v14_term(premise) || proposition_uses_v14_term(conclusion),
    }
}

fn scalar_term_uses_v14(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v14(value) || scalar_term_uses_v14(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v14(value) || scalar_term_uses_v14(count)
        }
        ScalarTerm::IntegerBitwiseAnd { .. }
        | ScalarTerm::IntegerBitwiseOr { .. }
        | ScalarTerm::IntegerBitwiseXor { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v14(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v14(left) || scalar_term_uses_v14(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v13_term(node: &ProofNode) -> bool {
    proposition_uses_v13_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v13_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v13_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v13_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v13_term(implication) || proof_uses_v13_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v13_term(left_equals_middle) || proof_uses_v13_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v13_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v13(left) || scalar_term_uses_v13(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v13_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v13_term(premise) || proposition_uses_v13_term(conclusion),
    }
}

fn scalar_term_uses_v13(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v13(value) || scalar_term_uses_v13(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v13(value) || scalar_term_uses_v13(count)
        }
        ScalarTerm::IntegerLessThan { .. } | ScalarTerm::IntegerLessOrEqual { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v13(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v13(left) || scalar_term_uses_v13(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v12_term(node: &ProofNode) -> bool {
    proposition_uses_v12_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v12_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v12_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v12_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v12_term(implication) || proof_uses_v12_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v12_term(left_equals_middle) || proof_uses_v12_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v12_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v12(left) || scalar_term_uses_v12(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v12_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v12_term(premise) || proposition_uses_v12_term(conclusion),
    }
}

fn scalar_term_uses_v12(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v12(value) || scalar_term_uses_v12(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v12(value) || scalar_term_uses_v12(count)
        }
        ScalarTerm::IntegerEqual { .. } => true,
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v12(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. }
        | ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v12(left) || scalar_term_uses_v12(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v11_term(node: &ProofNode) -> bool {
    proposition_uses_v11_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v11_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v11_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v11_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v11_term(implication) || proof_uses_v11_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v11_term(left_equals_middle) || proof_uses_v11_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v11_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v11(left) || scalar_term_uses_v11(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v11_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v11_term(premise) || proposition_uses_v11_term(conclusion),
    }
}

fn scalar_term_uses_v11(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v11(value) || scalar_term_uses_v11(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v11(value) || scalar_term_uses_v11(count)
        }
        ScalarTerm::BooleanEqual { .. } => true,
        ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v11(left) || scalar_term_uses_v11(right)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v11(operand),
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v11(left) || scalar_term_uses_v11(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v10_term(node: &ProofNode) -> bool {
    proposition_uses_v10_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v10_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v10_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v10_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v10_term(implication) || proof_uses_v10_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v10_term(left_equals_middle) || proof_uses_v10_term(middle_equals_right)
            }
        }
}

fn proposition_uses_v10_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v10(left) || scalar_term_uses_v10(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v10_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v10_term(premise) || proposition_uses_v10_term(conclusion),
    }
}

fn scalar_term_uses_v10(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v10(value) || scalar_term_uses_v10(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v10(value) || scalar_term_uses_v10(count)
        }
        ScalarTerm::BooleanNot { .. } => true,
        ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v10(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v10(left) || scalar_term_uses_v10(right)
        }
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v10(left) || scalar_term_uses_v10(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v9_content_case(node: &ProofNode) -> bool {
    proposition_uses_v9_content_case(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => {
                nodes.iter().any(proof_uses_v9_content_case)
            }
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v9_content_case(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v9_content_case(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v9_content_case(implication) || proof_uses_v9_content_case(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v9_content_case(left_equals_middle)
                    || proof_uses_v9_content_case(middle_equals_right)
            }
        }
}

fn proposition_uses_v9_content_case(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::ContentConservation(conservation) => {
            content_term_uses_case(conservation.left())
                || content_term_uses_case(conservation.right())
        }
        Proposition::Conjunction(conjuncts) => {
            conjuncts.iter().any(proposition_uses_v9_content_case)
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            proposition_uses_v9_content_case(premise)
                || proposition_uses_v9_content_case(conclusion)
        }
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::Equal(_, _)
        | Proposition::LessThan(_, _)
        | Proposition::LessOrEqual(_, _) => false,
    }
}

fn content_term_uses_case(term: &ContentTerm) -> bool {
    match term {
        ContentTerm::Projection { subject, .. } => subject
            .segments
            .iter()
            .any(|segment| matches!(segment, ContentPlaceSegment::Case(_))),
        ContentTerm::Separate(terms) => terms.iter().any(content_term_uses_case),
    }
}

fn proof_uses_v8_content(node: &ProofNode) -> bool {
    proposition_uses_v8_content(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v8_content),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v8_content(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v8_content(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v8_content(implication) || proof_uses_v8_content(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => {
                proof_uses_v8_content(left_equals_middle)
                    || proof_uses_v8_content(middle_equals_right)
            }
        }
}

fn proposition_uses_v8_content(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::ContentConservation(_) => true,
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v8_content),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v8_content(premise) || proposition_uses_v8_content(conclusion),
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::Equal(_, _)
        | Proposition::LessThan(_, _)
        | Proposition::LessOrEqual(_, _) => false,
    }
}

fn proof_uses_v7_term(node: &ProofNode) -> bool {
    proposition_uses_v7_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v7_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v7_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v7_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v7_term(implication) || proof_uses_v7_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_uses_v7_term(left_equals_middle) || proof_uses_v7_term(middle_equals_right),
        }
}

fn proposition_uses_v7_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v7(left) || scalar_term_uses_v7(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v7_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v7_term(premise) || proposition_uses_v7_term(conclusion),
    }
}

fn scalar_term_uses_v7(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v7(value) || scalar_term_uses_v7(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v7(value) || scalar_term_uses_v7(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v7(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v7(left) || scalar_term_uses_v7(right)
        }
        ScalarTerm::SaturatingIntegerMultiply { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v7(left) || scalar_term_uses_v7(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v6_term(node: &ProofNode) -> bool {
    proposition_uses_v6_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v6_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v6_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v6_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v6_term(implication) || proof_uses_v6_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_uses_v6_term(left_equals_middle) || proof_uses_v6_term(middle_equals_right),
        }
}

fn proposition_uses_v6_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v6(left) || scalar_term_uses_v6(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v6_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v6_term(premise) || proposition_uses_v6_term(conclusion),
    }
}

fn scalar_term_uses_v6(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v6(value) || scalar_term_uses_v6(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v6(value) || scalar_term_uses_v6(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v6(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v6(left) || scalar_term_uses_v6(right)
        }
        ScalarTerm::WrappingIntegerMultiply { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v6(left) || scalar_term_uses_v6(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v5_term(node: &ProofNode) -> bool {
    proposition_uses_v5_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v5_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v5_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v5_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v5_term(implication) || proof_uses_v5_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_uses_v5_term(left_equals_middle) || proof_uses_v5_term(middle_equals_right),
        }
}

fn proposition_uses_v5_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v5(left) || scalar_term_uses_v5(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v5_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v5_term(premise) || proposition_uses_v5_term(conclusion),
    }
}

fn scalar_term_uses_v5(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v5(value) || scalar_term_uses_v5(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v5(value) || scalar_term_uses_v5(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v5(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v5(left) || scalar_term_uses_v5(right)
        }
        ScalarTerm::SaturatingIntegerSubtract { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
        | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
        | ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v5(left) || scalar_term_uses_v5(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v4_term(node: &ProofNode) -> bool {
    proposition_uses_v4_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v4_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v4_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v4_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v4_term(implication) || proof_uses_v4_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_uses_v4_term(left_equals_middle) || proof_uses_v4_term(middle_equals_right),
        }
}

fn proposition_uses_v4_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v4(left) || scalar_term_uses_v4(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v4_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v4_term(premise) || proposition_uses_v4_term(conclusion),
    }
}

fn scalar_term_uses_v4(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v4(value) || scalar_term_uses_v4(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v4(value) || scalar_term_uses_v4(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v4(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v4(left) || scalar_term_uses_v4(right)
        }
        ScalarTerm::WrappingIntegerSubtract { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            scalar_term_uses_v4(left) || scalar_term_uses_v4(right)
        }
        ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
            scalar_term_uses_v4(left) || scalar_term_uses_v4(right)
        }
        ScalarTerm::WrappingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v4(left) || scalar_term_uses_v4(right)
        }
        ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v4(left) || scalar_term_uses_v4(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v3_term(node: &ProofNode) -> bool {
    proposition_uses_v3_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v3_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v3_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v3_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v3_term(implication) || proof_uses_v3_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_uses_v3_term(left_equals_middle) || proof_uses_v3_term(middle_equals_right),
        }
}

fn proposition_uses_v3_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v3_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v3_term(premise) || proposition_uses_v3_term(conclusion),
    }
}

fn scalar_term_uses_v3(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v3(value) || scalar_term_uses_v3(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v3(value) || scalar_term_uses_v3(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v3(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::SaturatingIntegerAdd { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::WrappingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn proof_uses_v2_term(node: &ProofNode) -> bool {
    proposition_uses_v2_term(&node.conclusion)
        || match &node.rule {
            ProofRule::Primitive(_)
            | ProofRule::SemanticAxiom { .. }
            | ProofRule::Assumption { .. } => false,
            ProofRule::ConjunctionIntroduction(nodes) => nodes.iter().any(proof_uses_v2_term),
            ProofRule::ConjunctionElimination { conjunction, .. } => {
                proof_uses_v2_term(conjunction)
            }
            ProofRule::ImplicationIntroduction { body } => proof_uses_v2_term(body),
            ProofRule::ImplicationElimination {
                implication,
                premise,
            } => proof_uses_v2_term(implication) || proof_uses_v2_term(premise),
            ProofRule::EqualityTransitivity {
                left_equals_middle,
                middle_equals_right,
            } => proof_uses_v2_term(left_equals_middle) || proof_uses_v2_term(middle_equals_right),
        }
}

fn proposition_uses_v2_term(proposition: &Proposition) -> bool {
    match proposition {
        Proposition::Truth
        | Proposition::Falsehood
        | Proposition::Atom(_)
        | Proposition::ContentConservation(_) => false,
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        Proposition::Conjunction(conjuncts) => conjuncts.iter().any(proposition_uses_v2_term),
        Proposition::Implication {
            premise,
            conclusion,
        } => proposition_uses_v2_term(premise) || proposition_uses_v2_term(conclusion),
    }
}

fn scalar_term_uses_v2(term: &ScalarTerm) -> bool {
    match term {
        ScalarTerm::WrappingIntegerRemainder { .. } => false,
        ScalarTerm::WrappingIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerRemainder { .. } => false,
        ScalarTerm::ExactIntegerDivide { .. } => false,
        ScalarTerm::ExactIntegerMultiply { .. } => false,
        ScalarTerm::ExactIntegerSubtract { .. } => false,
        ScalarTerm::ExactIntegerAdd { .. } => false,
        ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
        | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v2(value) || scalar_term_uses_v2(count)
        }
        ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
        | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
            scalar_term_uses_v2(value) || scalar_term_uses_v2(count)
        }
        ScalarTerm::BooleanNot { operand }
        | ScalarTerm::IntegerBitwiseNot { operand, .. }
        | ScalarTerm::IntegerWiden { operand, .. }
        | ScalarTerm::IntegerExactCast { operand, .. } => scalar_term_uses_v2(operand),
        ScalarTerm::BooleanEqual { left, right }
        | ScalarTerm::IntegerEqual { left, right, .. }
        | ScalarTerm::IntegerLessThan { left, right, .. }
        | ScalarTerm::IntegerLessOrEqual { left, right, .. }
        | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
        | ScalarTerm::IntegerBitwiseOr { left, right, .. }
        | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::WrappingIntegerAdd { .. } => true,
        ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::SaturatingIntegerSubtract { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::WrappingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::SaturatingIntegerMultiply { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::Value { .. } | ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => false,
    }
}

fn encode_raw(bundle: &ProofBundle, format_version: u16) -> Result<Vec<u8>, ProofCodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(format_version);
    writer.len("evidence", bundle.evidence.len())?;
    for evidence in &bundle.evidence {
        encode_evidence(&mut writer, evidence, format_version)?;
    }
    Ok(writer.finish())
}

fn encode_evidence(
    writer: &mut Writer,
    evidence: &ObligationEvidence,
    format_version: u16,
) -> Result<(), ProofCodecError> {
    writer.id(evidence.obligation);
    match &evidence.route {
        EvidenceRoute::KernelDerived(judgment) => {
            writer.u8(1);
            encode_primitive(writer, *judgment);
        }
        EvidenceRoute::CertificateDerived(certificate) => {
            writer.u8(2);
            writer.id(certificate.identity);
            writer.u16(certificate.proof_system_version.get());
            encode_proof_node(writer, &certificate.proof, 0, format_version)?;
        }
        EvidenceRoute::Admitted(evidence) => {
            writer.u8(3);
            writer.id(evidence.site);
            encode_admission_kind(writer, evidence.kind);
            writer.id(evidence.authority_identity);
            writer.id(evidence.evidence_identity);
            writer.id(evidence.profile_decision);
        }
    }
    Ok(())
}

fn encode_proof_node(
    writer: &mut Writer,
    node: &ProofNode,
    depth: usize,
    format_version: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_PROOF_DEPTH {
        return Err(ProofCodecError::ProofNestingTooDeep);
    }
    encode_proposition(writer, &node.conclusion, 0, format_version)?;
    match &node.rule {
        ProofRule::Primitive(judgment) => {
            writer.u8(1);
            encode_primitive(writer, *judgment);
        }
        ProofRule::SemanticAxiom { index } => {
            writer.u8(2);
            writer.index("semantic axiom index", *index)?;
        }
        ProofRule::Assumption { index } => {
            writer.u8(3);
            writer.index("assumption index", *index)?;
        }
        ProofRule::ConjunctionIntroduction(nodes) => {
            writer.u8(4);
            writer.len("conjunction proofs", nodes.len())?;
            for node in nodes {
                encode_proof_node(writer, node, depth + 1, format_version)?;
            }
        }
        ProofRule::ConjunctionElimination {
            conjunction,
            conjunct,
        } => {
            writer.u8(5);
            encode_proof_node(writer, conjunction, depth + 1, format_version)?;
            writer.index("conjunct index", *conjunct)?;
        }
        ProofRule::ImplicationIntroduction { body } => {
            writer.u8(6);
            encode_proof_node(writer, body, depth + 1, format_version)?;
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            writer.u8(7);
            encode_proof_node(writer, implication, depth + 1, format_version)?;
            encode_proof_node(writer, premise, depth + 1, format_version)?;
        }
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        } => {
            writer.u8(8);
            encode_proof_node(writer, left_equals_middle, depth + 1, format_version)?;
            encode_proof_node(writer, middle_equals_right, depth + 1, format_version)?;
        }
    }
    Ok(())
}

fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
    depth: usize,
    format_version: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(ProofCodecError::PropositionNestingTooDeep);
    }
    match proposition {
        Proposition::Truth => writer.u8(1),
        Proposition::Falsehood => writer.u8(2),
        Proposition::Atom(id) => {
            writer.u8(3);
            writer.id(*id);
        }
        Proposition::Equal(left, right) => {
            writer.u8(4);
            encode_scalar_term(writer, left, 0, format_version)?;
            encode_scalar_term(writer, right, 0, format_version)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left, 0, format_version)?;
            encode_scalar_term(writer, right, 0, format_version)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left, 0, format_version)?;
            encode_scalar_term(writer, right, 0, format_version)?;
        }
        Proposition::Conjunction(conjuncts) => {
            writer.u8(7);
            writer.len("proof proposition conjuncts", conjuncts.len())?;
            for conjunct in conjuncts {
                encode_proposition(writer, conjunct, depth + 1, format_version)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            writer.u8(8);
            encode_proposition(writer, premise, depth + 1, format_version)?;
            encode_proposition(writer, conclusion, depth + 1, format_version)?;
        }
        Proposition::ContentConservation(conservation) => {
            if format_version < FORMAT_VERSION_V8 {
                return Err(ProofCodecError::UnsupportedContentPropositionForFormat);
            }
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left(), 0, format_version)?;
            encode_content_term(writer, conservation.right(), 0, format_version)?;
        }
    }
    Ok(())
}

fn encode_content_algebra(
    writer: &mut Writer,
    algebra: &ContentAlgebra,
) -> Result<(), ProofCodecError> {
    writer.u8(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 1,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    writer.string("content algebra parameter", &algebra.parameter)
}

fn encode_content_term(
    writer: &mut Writer,
    term: &ContentTerm,
    depth: usize,
    format_version: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(ProofCodecError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            writer.u8(1);
            writer.id(projection.domain);
            writer.u64(projection.projection_fingerprint);
            writer.u8(match subject.version {
                ContentPlaceVersion::Entry => 1,
                ContentPlaceVersion::Current => 2,
            });
            writer.id(subject.root);
            writer.len("content place segments", subject.segments.len())?;
            for segment in &subject.segments {
                match segment {
                    ContentPlaceSegment::Case(name) => {
                        if format_version < FORMAT_VERSION_V9 {
                            return Err(ProofCodecError::UnsupportedContentCasePathForFormat);
                        }
                        writer.u8(3);
                        writer.string("content case", name)?;
                    }
                    ContentPlaceSegment::Field(name) => {
                        writer.u8(1);
                        writer.string("content field", name)?;
                    }
                    ContentPlaceSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                }
            }
        }
        ContentTerm::Separate(terms) => {
            writer.u8(2);
            writer.len("separated content terms", terms.len())?;
            for term in terms {
                encode_content_term(writer, term, depth + 1, format_version)?;
            }
        }
    }
    Ok(())
}

fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
    format_version: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::Value { id, scalar_type } => {
            writer.u8(1);
            writer.id(*id);
            encode_scalar_type(writer, *scalar_type);
        }
        ScalarTerm::Boolean(value) => {
            writer.u8(2);
            writer.u8(u8::from(*value));
        }
        ScalarTerm::BooleanNot { operand } => {
            if format_version < FORMAT_VERSION_V10 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(10);
            encode_scalar_term(writer, operand, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V22 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(25);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V23 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(26);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V24 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(27);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V25 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(28);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V26 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(29);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V27 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(30);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V28 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(31);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            if format_version < FORMAT_VERSION_V21 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(24);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_version)?;
            encode_scalar_term(writer, count, depth + 1, format_version)?;
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            if format_version < FORMAT_VERSION_V20 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(23);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_version)?;
            encode_scalar_term(writer, count, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            if format_version < FORMAT_VERSION_V19 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(22);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1, format_version)?;
        }
        ScalarTerm::BooleanEqual { left, right } => {
            if format_version < FORMAT_VERSION_V11 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(11);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V12 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(12);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V13 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(13);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V13 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(14);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerBitwiseAnd {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseOr {
            scalar_type,
            left,
            right,
        }
        | ScalarTerm::IntegerBitwiseXor {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V14 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(match term {
                ScalarTerm::IntegerBitwiseAnd { .. } => 15,
                ScalarTerm::IntegerBitwiseOr { .. } => 16,
                ScalarTerm::IntegerBitwiseXor { .. } => 17,
                _ => unreachable!(),
            });
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        }
        | ScalarTerm::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            if format_version < FORMAT_VERSION_V15 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(match term {
                ScalarTerm::WrappingIntegerShiftLeft { .. } => 18,
                ScalarTerm::WrappingIntegerShiftRight { .. } => 19,
                _ => unreachable!(),
            });
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_version)?;
            encode_scalar_term(writer, count, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            if format_version < FORMAT_VERSION_V16 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(20);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, operand, depth + 1, format_version)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            if format_version < FORMAT_VERSION_V17 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(21);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1, format_version)?;
        }
        ScalarTerm::Integer { scalar_type, value } => {
            writer.u8(3);
            encode_integer_type(writer, *scalar_type);
            encode_integer_value(writer, *value);
        }
        ScalarTerm::WrappingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V2 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(4);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V3 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(5);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V4 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(6);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V5 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(7);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V6 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(8);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            if format_version < FORMAT_VERSION_V7 {
                return Err(ProofCodecError::UnsupportedScalarTermForFormat);
            }
            writer.u8(9);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_version)?;
            encode_scalar_term(writer, right, depth + 1, format_version)?;
        }
    }
    Ok(())
}

fn encode_scalar_type(writer: &mut Writer, scalar_type: ScalarType) {
    match scalar_type {
        ScalarType::Boolean => writer.u8(1),
        ScalarType::Integer(integer_type) => {
            writer.u8(2);
            encode_integer_type(writer, integer_type);
        }
    }
}

fn encode_integer_type(writer: &mut Writer, integer_type: IntegerType) {
    writer.u8(match (integer_type.carrier(), integer_type.sign()) {
        (IntegerCarrier::Fixed, IntegerSign::Signed) => 1,
        (IntegerCarrier::Fixed, IntegerSign::Unsigned) => 2,
        (IntegerCarrier::Address, IntegerSign::Unsigned) => 3,
        (IntegerCarrier::Address, IntegerSign::Signed) => {
            unreachable!("address carriers are unsigned")
        }
    });
    writer.u16(integer_type.bits());
}

fn encode_integer_value(writer: &mut Writer, value: IntegerValue) {
    match value {
        IntegerValue::Signed(value) => {
            writer.u8(1);
            writer.bytes(&value.to_le_bytes());
        }
        IntegerValue::Unsigned(value) => {
            writer.u8(2);
            writer.bytes(&value.to_le_bytes());
        }
    }
}

fn encode_primitive(writer: &mut Writer, judgment: PrimitiveJudgment) {
    writer.u8(match judgment {
        PrimitiveJudgment::Truth => 1,
        PrimitiveJudgment::ReflexiveEquality => 2,
        PrimitiveJudgment::ClosedIntegerRelation => 3,
    });
}

fn encode_admission_kind(writer: &mut Writer, kind: AdmissionKind) {
    writer.u8(match kind {
        AdmissionKind::ForeignBoundaryGuarantee => 1,
        AdmissionKind::ProviderFact => 2,
        AdmissionKind::CheckedAssemblyClaim => 3,
    });
}

fn decode_evidence(
    reader: &mut Reader<'_>,
    format_version: u16,
) -> Result<ObligationEvidence, ProofCodecError> {
    let obligation = reader.id("ObligationId")?;
    let route = match reader.u8()? {
        1 => EvidenceRoute::KernelDerived(decode_primitive(reader)?),
        2 => EvidenceRoute::CertificateDerived(CertificateEnvelope {
            identity: reader.id("EvidenceIdentity")?,
            proof_system_version: ProofSystemVersion::new(reader.u16()?)
                .ok_or(ProofCodecError::ZeroProofSystemVersion)?,
            proof: decode_proof_node(reader, 0, format_version)?,
        }),
        3 => EvidenceRoute::Admitted(AdmissionEvidence {
            site: reader.id("AdmissionSiteId")?,
            kind: decode_admission_kind(reader)?,
            authority_identity: reader.id("EvidenceIdentity")?,
            evidence_identity: reader.id("EvidenceIdentity")?,
            profile_decision: reader.id("ProfileDecisionId")?,
        }),
        tag => return Err(ProofCodecError::InvalidTag("EvidenceRoute", tag)),
    };
    Ok(ObligationEvidence { obligation, route })
}

fn decode_proof_node(
    reader: &mut Reader<'_>,
    depth: usize,
    format_version: u16,
) -> Result<ProofNode, ProofCodecError> {
    if depth > MAX_PROOF_DEPTH {
        return Err(ProofCodecError::ProofNestingTooDeep);
    }
    let conclusion = decode_proposition(reader, 0, format_version)?;
    let rule = match reader.u8()? {
        1 => ProofRule::Primitive(decode_primitive(reader)?),
        2 => ProofRule::SemanticAxiom {
            index: reader.index()?,
        },
        3 => ProofRule::Assumption {
            index: reader.index()?,
        },
        4 => {
            let count = reader.count()?;
            let mut nodes = Vec::new();
            for _ in 0..count {
                nodes.push(decode_proof_node(reader, depth + 1, format_version)?);
            }
            ProofRule::ConjunctionIntroduction(nodes)
        }
        5 => ProofRule::ConjunctionElimination {
            conjunction: Box::new(decode_proof_node(reader, depth + 1, format_version)?),
            conjunct: reader.index()?,
        },
        6 => ProofRule::ImplicationIntroduction {
            body: Box::new(decode_proof_node(reader, depth + 1, format_version)?),
        },
        7 => ProofRule::ImplicationElimination {
            implication: Box::new(decode_proof_node(reader, depth + 1, format_version)?),
            premise: Box::new(decode_proof_node(reader, depth + 1, format_version)?),
        },
        8 => ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(decode_proof_node(reader, depth + 1, format_version)?),
            middle_equals_right: Box::new(decode_proof_node(reader, depth + 1, format_version)?),
        },
        tag => return Err(ProofCodecError::InvalidTag("ProofRule", tag)),
    };
    Ok(ProofNode { conclusion, rule })
}

fn decode_proposition(
    reader: &mut Reader<'_>,
    depth: usize,
    format_version: u16,
) -> Result<Proposition, ProofCodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(ProofCodecError::PropositionNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(
            decode_scalar_term(reader, 0, format_version)?,
            decode_scalar_term(reader, 0, format_version)?,
        ),
        5 => Proposition::LessThan(
            decode_scalar_term(reader, 0, format_version)?,
            decode_scalar_term(reader, 0, format_version)?,
        ),
        6 => Proposition::LessOrEqual(
            decode_scalar_term(reader, 0, format_version)?,
            decode_scalar_term(reader, 0, format_version)?,
        ),
        7 => {
            let count = reader.count()?;
            let mut conjuncts = Vec::new();
            for _ in 0..count {
                conjuncts.push(decode_proposition(reader, depth + 1, format_version)?);
            }
            Proposition::Conjunction(conjuncts)
        }
        8 => Proposition::Implication {
            premise: Box::new(decode_proposition(reader, depth + 1, format_version)?),
            conclusion: Box::new(decode_proposition(reader, depth + 1, format_version)?),
        },
        9 if format_version >= FORMAT_VERSION_V8 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader, 0, format_version)?;
            let right = decode_content_term(reader, 0, format_version)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        tag => return Err(ProofCodecError::InvalidTag("Proposition", tag)),
    })
}

fn decode_content_algebra(reader: &mut Reader<'_>) -> Result<ContentAlgebra, ProofCodecError> {
    let kind = match reader.u8()? {
        1 => ContentAlgebraKind::IntervalSet,
        2 => ContentAlgebraKind::CountedQuantity,
        tag => return Err(ProofCodecError::InvalidTag("ContentAlgebraKind", tag)),
    };
    Ok(ContentAlgebra {
        kind,
        parameter: reader.string("content algebra parameter")?,
    })
}

fn decode_content_term(
    reader: &mut Reader<'_>,
    depth: usize,
    format_version: u16,
) -> Result<ContentTerm, ProofCodecError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(ProofCodecError::ContentTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => {
            let projection = ContentProjectionIdentity {
                domain: reader.id::<ContentDomainId>("ContentDomainId")?,
                projection_fingerprint: reader.u64()?,
            };
            let version = match reader.u8()? {
                1 => ContentPlaceVersion::Entry,
                2 => ContentPlaceVersion::Current,
                tag => return Err(ProofCodecError::InvalidTag("ContentPlaceVersion", tag)),
            };
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut segments = Vec::new();
            for _ in 0..count {
                segments.push(match reader.u8()? {
                    1 => ContentPlaceSegment::Field(reader.string("content field")?),
                    2 => ContentPlaceSegment::FixedIndex(reader.u64()?),
                    3 if format_version >= FORMAT_VERSION_V9 => {
                        ContentPlaceSegment::Case(reader.string("content case")?)
                    }
                    tag => return Err(ProofCodecError::InvalidTag("ContentPlaceSegment", tag)),
                });
            }
            ContentTerm::Projection {
                projection,
                subject: ContentStructuralPlace {
                    version,
                    root,
                    segments,
                },
            }
        }
        2 => {
            let count = reader.count()?;
            let mut terms = Vec::new();
            for _ in 0..count {
                terms.push(decode_content_term(reader, depth + 1, format_version)?);
            }
            ContentTerm::separate(terms).map_err(ProofCodecError::MalformedProposition)?
        }
        tag => return Err(ProofCodecError::InvalidTag("ContentTerm", tag)),
    })
}

fn decode_scalar_term(
    reader: &mut Reader<'_>,
    depth: usize,
    format_version: u16,
) -> Result<ScalarTerm, ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => ScalarTerm::value(reader.id("ValueId")?, decode_scalar_type(reader)?),
        2 => ScalarTerm::boolean(reader.boolean()?),
        3 => {
            let scalar_type = decode_integer_type(reader)?;
            let value = decode_integer_value(reader)?;
            ScalarTerm::integer(scalar_type, value)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        4 if format_version >= FORMAT_VERSION_V2 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        5 if format_version >= FORMAT_VERSION_V3 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::saturating_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        6 if format_version >= FORMAT_VERSION_V4 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        7 if format_version >= FORMAT_VERSION_V5 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::saturating_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        8 if format_version >= FORMAT_VERSION_V6 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        9 if format_version >= FORMAT_VERSION_V7 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::saturating_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        10 if format_version >= FORMAT_VERSION_V10 => {
            ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1, format_version)?)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        11 if format_version >= FORMAT_VERSION_V11 => {
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::boolean_equal(left, right).map_err(ProofCodecError::MalformedProposition)?
        }
        12 if format_version >= FORMAT_VERSION_V12 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_equal(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        13 if format_version >= FORMAT_VERSION_V13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_less_than(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        14 if format_version >= FORMAT_VERSION_V13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_less_or_equal(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        15 if format_version >= FORMAT_VERSION_V14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_bitwise_and(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        16 if format_version >= FORMAT_VERSION_V14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_bitwise_or(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        17 if format_version >= FORMAT_VERSION_V14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_bitwise_xor(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        18 if format_version >= FORMAT_VERSION_V15 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_version)?;
            let count = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        19 if format_version >= FORMAT_VERSION_V15 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_version)?;
            let count = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        20 if format_version >= FORMAT_VERSION_V16 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        21 if format_version >= FORMAT_VERSION_V17 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_widen(source_type, target_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        22 if format_version >= FORMAT_VERSION_V19 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        23 if format_version >= FORMAT_VERSION_V20 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_version)?;
            let count = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        24 if format_version >= FORMAT_VERSION_V21 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_version)?;
            let count = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        25 if format_version >= FORMAT_VERSION_V22 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        26 if format_version >= FORMAT_VERSION_V23 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        27 if format_version >= FORMAT_VERSION_V24 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        28 if format_version >= FORMAT_VERSION_V25 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        29 if format_version >= FORMAT_VERSION_V26 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::exact_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        30 if format_version >= FORMAT_VERSION_V27 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        31 if format_version >= FORMAT_VERSION_V28 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_version)?;
            let right = decode_scalar_term(reader, depth + 1, format_version)?;
            ScalarTerm::wrapping_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        tag => return Err(ProofCodecError::InvalidTag("ScalarTerm", tag)),
    })
}

fn decode_scalar_type(reader: &mut Reader<'_>) -> Result<ScalarType, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => ScalarType::Boolean,
        2 => ScalarType::Integer(decode_integer_type(reader)?),
        tag => return Err(ProofCodecError::InvalidTag("ScalarType", tag)),
    })
}

fn decode_integer_type(reader: &mut Reader<'_>) -> Result<IntegerType, ProofCodecError> {
    let tag = reader.u8()?;
    let bits = reader.u16()?;
    match tag {
        1 => IntegerType::new(IntegerSign::Signed, bits),
        2 => IntegerType::new(IntegerSign::Unsigned, bits),
        3 => IntegerType::address(bits),
        tag => return Err(ProofCodecError::InvalidTag("IntegerSign", tag)),
    }
    .map_err(ProofCodecError::MalformedProposition)
}

fn decode_integer_value(reader: &mut Reader<'_>) -> Result<IntegerValue, ProofCodecError> {
    Ok(match reader.u8()? {
        1 => IntegerValue::Signed(i128::from_le_bytes(reader.array()?)),
        2 => IntegerValue::Unsigned(u128::from_le_bytes(reader.array()?)),
        tag => return Err(ProofCodecError::InvalidTag("IntegerValue", tag)),
    })
}

fn decode_primitive(reader: &mut Reader<'_>) -> Result<PrimitiveJudgment, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(PrimitiveJudgment::Truth),
        2 => Ok(PrimitiveJudgment::ReflexiveEquality),
        3 => Ok(PrimitiveJudgment::ClosedIntegerRelation),
        tag => Err(ProofCodecError::InvalidTag("PrimitiveJudgment", tag)),
    }
}

fn decode_admission_kind(reader: &mut Reader<'_>) -> Result<AdmissionKind, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(AdmissionKind::ForeignBoundaryGuarantee),
        2 => Ok(AdmissionKind::ProviderFact),
        3 => Ok(AdmissionKind::CheckedAssemblyClaim),
        tag => Err(ProofCodecError::InvalidTag("AdmissionKind", tag)),
    }
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn finish(self) -> Vec<u8> {
        self.bytes
    }

    fn bytes(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn id(&mut self, id: impl PsiSemanticId) {
        self.bytes(&id.get().to_le_bytes());
    }

    fn len(&mut self, label: &'static str, len: usize) -> Result<(), ProofCodecError> {
        self.u32(u32::try_from(len).map_err(|_| ProofCodecError::CollectionTooLong(label))?);
        Ok(())
    }

    fn index(&mut self, label: &'static str, index: usize) -> Result<(), ProofCodecError> {
        self.u32(u32::try_from(index).map_err(|_| ProofCodecError::IndexTooLarge(label))?);
        Ok(())
    }

    fn string(&mut self, label: &'static str, value: &str) -> Result<(), ProofCodecError> {
        if value.len() > MAX_CONTENT_IDENTITY_BYTES {
            return Err(ProofCodecError::StringTooLong(label));
        }
        self.len(label, value.len())?;
        self.bytes(value.as_bytes());
        Ok(())
    }
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, len: usize) -> Result<&'bytes [u8], ProofCodecError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(ProofCodecError::UnexpectedEnd)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(ProofCodecError::UnexpectedEnd)?;
        self.offset = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ProofCodecError> {
        self.take(N)?
            .try_into()
            .map_err(|_| ProofCodecError::UnexpectedEnd)
    }

    fn u8(&mut self) -> Result<u8, ProofCodecError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, ProofCodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u32(&mut self) -> Result<u32, ProofCodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, ProofCodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn count(&mut self) -> Result<u32, ProofCodecError> {
        self.u32()
    }

    fn index(&mut self) -> Result<usize, ProofCodecError> {
        usize::try_from(self.u32()?).map_err(|_| ProofCodecError::IndexOutsideHost)
    }

    fn boolean(&mut self) -> Result<bool, ProofCodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(ProofCodecError::InvalidBoolean(value)),
        }
    }

    fn string(&mut self, label: &'static str) -> Result<String, ProofCodecError> {
        let len =
            usize::try_from(self.count()?).map_err(|_| ProofCodecError::StringTooLong(label))?;
        if len > MAX_CONTENT_IDENTITY_BYTES {
            return Err(ProofCodecError::StringTooLong(label));
        }
        std::str::from_utf8(self.take(len)?)
            .map(str::to_owned)
            .map_err(|_| ProofCodecError::InvalidUtf8(label))
    }

    fn id<T: PsiSemanticId>(&mut self, label: &'static str) -> Result<T, ProofCodecError> {
        T::new(self.u64()?).ok_or(ProofCodecError::ZeroIdentity(label))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCodecError {
    InvalidMagic,
    UnsupportedFormatVersion(u16),
    ZeroProofSystemVersion,
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    IndexTooLarge(&'static str),
    IndexOutsideHost,
    NonCanonicalEvidenceOrder,
    NonCanonicalEncoding,
    PropositionNestingTooDeep,
    ScalarTermNestingTooDeep,
    ContentTermNestingTooDeep,
    ProofNestingTooDeep,
    UnsupportedScalarTermForFormat,
    UnsupportedContentPropositionForFormat,
    UnsupportedContentCasePathForFormat,
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedProposition(PropositionError),
}

impl std::fmt::Display for ProofCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProofCodecError {}
