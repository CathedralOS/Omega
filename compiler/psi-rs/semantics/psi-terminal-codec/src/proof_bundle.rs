use psi_core::{
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionError, PropositionId,
    PsiSemanticId, ScalarTerm, ScalarType,
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
const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-proof-bundle-fingerprint-v1\0";
const MAX_PROPOSITION_DEPTH: usize = 256;
const MAX_SCALAR_TERM_DEPTH: usize = 256;
const MAX_PROOF_DEPTH: usize = 256;

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
        FORMAT_VERSION_V1 | FORMAT_VERSION_V2 | FORMAT_VERSION_V3 | FORMAT_VERSION_V4
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
    }
    proposition
        .validate()
        .map_err(ProofCodecError::MalformedProposition)
}

fn validate_scalar_term_depth(term: &ScalarTerm, depth: usize) -> Result<(), ProofCodecError> {
    if depth > MAX_SCALAR_TERM_DEPTH {
        return Err(ProofCodecError::ScalarTermNestingTooDeep);
    }
    match term {
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
        | ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
            validate_scalar_term_depth(left, depth + 1)?;
            validate_scalar_term_depth(right, depth + 1)?;
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
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => false,
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
        ScalarTerm::WrappingIntegerSubtract { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. }
        | ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
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
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => false,
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
        ScalarTerm::SaturatingIntegerAdd { .. } => true,
        ScalarTerm::WrappingIntegerAdd { left, right, .. } => {
            scalar_term_uses_v3(left) || scalar_term_uses_v3(right)
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
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
        Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => false,
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
        ScalarTerm::WrappingIntegerAdd { .. } => true,
        ScalarTerm::SaturatingIntegerAdd { left, right, .. } => {
            scalar_term_uses_v2(left) || scalar_term_uses_v2(right)
        }
        ScalarTerm::WrappingIntegerSubtract { left, right, .. } => {
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
    writer.u8(match integer_type.sign() {
        IntegerSign::Signed => 1,
        IntegerSign::Unsigned => 2,
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
        tag => return Err(ProofCodecError::InvalidTag("Proposition", tag)),
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
    let sign = match reader.u8()? {
        1 => IntegerSign::Signed,
        2 => IntegerSign::Unsigned,
        tag => return Err(ProofCodecError::InvalidTag("IntegerSign", tag)),
    };
    IntegerType::new(sign, reader.u16()?).map_err(ProofCodecError::MalformedProposition)
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
    ProofNestingTooDeep,
    UnsupportedScalarTermForFormat,
    MalformedProposition(PropositionError),
}

impl std::fmt::Display for ProofCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProofCodecError {}
