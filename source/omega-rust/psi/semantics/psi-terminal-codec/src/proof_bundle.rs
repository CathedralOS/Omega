mod synopsis;
mod validation;
mod wire;

use psi_core::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, ContentAlgebra,
    ContentAlgebraKind, ContentConservation, ContentDomainId, ContentPlaceSegment,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    IeeeFloatComparisonKind, IeeeFloatFormat, IeeeFloatStructuralField, IntegerCarrier,
    IntegerSign, IntegerType, IntegerValue, Proposition, PropositionError, PropositionId,
    ScalarTerm, ScalarType, StructuralCaseSubject,
};
use psi_proof_admission::{
    AdmissionEvidence, AdmissionKind, CertificateEnvelope, EvidenceRoute, IntegerAffineWitness,
    IntegerCastChainWitness, PrimitiveJudgment, ProofNode, ProofRule, ProofSystemMarker,
};
use psi_terminal_verifier::{
    EvidenceProducerProvenance, EvidenceProducerRealization, EvidenceProducerRowSource,
    ObligationEvidence, ProofBundle,
};
use sha2::{Digest, Sha256};
pub use synopsis::render_verified_proof_synopsis;
use validation::validate_bundle;
use wire::{Reader, Writer};

const MAGIC: &[u8; 8] = b"PSIPRF\0\0";
/// Single current pre-release proof vocabulary marker.
pub(crate) const FORMAT_MARKER: u16 = 19;
const FINGERPRINT_DOMAIN: &[u8] = b"psi-terminal-proof-bundle-fingerprint\0";
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
    encode_raw(bundle, FORMAT_MARKER)
}

pub fn decode_proof_bundle(bytes: &[u8]) -> Result<ProofBundle, ProofCodecError> {
    let mut reader = Reader::new(bytes);
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(ProofCodecError::InvalidMagic);
    }
    let format_marker = reader.u16()?;
    if format_marker != FORMAT_MARKER {
        return Err(ProofCodecError::UnsupportedFormatMarker(format_marker));
    }
    let evidence_count = reader.count()?;
    let mut evidence = Vec::new();
    for _ in 0..evidence_count {
        evidence.push(decode_evidence(&mut reader, format_marker)?);
    }
    let producer_count = reader.count()?;
    let mut evidence_producers = Vec::new();
    for _ in 0..producer_count {
        evidence_producers.push(decode_evidence_producer(&mut reader)?);
    }
    if reader.remaining() != 0 {
        return Err(ProofCodecError::TrailingBytes(reader.remaining()));
    }
    let bundle = ProofBundle {
        evidence,
        evidence_producers,
    };
    validate_bundle(&bundle)?;
    if encode_raw(&bundle, format_marker)? != bytes {
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

fn encode_raw(bundle: &ProofBundle, format_marker: u16) -> Result<Vec<u8>, ProofCodecError> {
    let mut writer = Writer::default();
    writer.bytes(MAGIC);
    writer.u16(format_marker);
    writer.len("evidence", bundle.evidence.len())?;
    for evidence in &bundle.evidence {
        encode_evidence(&mut writer, evidence, format_marker)?;
    }
    writer.len("evidence producers", bundle.evidence_producers.len())?;
    for producer in &bundle.evidence_producers {
        encode_evidence_producer(&mut writer, producer)?;
    }
    Ok(writer.finish())
}

fn encode_evidence_producer(
    writer: &mut Writer,
    producer: &EvidenceProducerProvenance,
) -> Result<(), ProofCodecError> {
    writer.id(producer.id);
    writer.id(producer.term);
    writer.string(
        "evidence producer conformance",
        &producer.conformance_identity,
    )?;
    writer.string("evidence producer trait", &producer.evidence_trait_identity)?;
    writer.len("evidence producer rows", producer.rows.len())?;
    for row in &producer.rows {
        writer.string(
            "evidence producer declaring trait",
            &row.declaring_trait_identity,
        )?;
        writer.len(
            "evidence producer declaring trait arguments",
            row.declaring_trait_arguments.len(),
        )?;
        for argument in &row.declaring_trait_arguments {
            writer.string("evidence producer declaring trait argument", argument)?;
        }
        writer.string("evidence producer requirement", &row.requirement_identity)?;
        writer.string(
            "evidence producer machine",
            &row.realization_machine_identity,
        )?;
        writer.string("evidence producer state", &row.realization_state_identity)?;
        writer.u8(match row.source {
            EvidenceProducerRowSource::Inline => 1,
            EvidenceProducerRowSource::Reference => 2,
            EvidenceProducerRowSource::TraitDefault => 3,
        });
    }
    Ok(())
}

fn decode_evidence_producer(
    reader: &mut Reader<'_>,
) -> Result<EvidenceProducerProvenance, ProofCodecError> {
    let id = reader.id("EvidenceIdentity")?;
    let term = reader.id("EvidenceTermId")?;
    let conformance_identity = reader.string("evidence producer conformance")?;
    let evidence_trait_identity = reader.string("evidence producer trait")?;
    let row_count = reader.count()?;
    let mut rows = Vec::new();
    for _ in 0..row_count {
        let declaring_trait_identity = reader.string("evidence producer declaring trait")?;
        let argument_count = reader.count()?;
        let mut declaring_trait_arguments = Vec::new();
        for _ in 0..argument_count {
            declaring_trait_arguments
                .push(reader.string("evidence producer declaring trait argument")?);
        }
        rows.push(EvidenceProducerRealization {
            declaring_trait_identity,
            declaring_trait_arguments,
            requirement_identity: reader.string("evidence producer requirement")?,
            realization_machine_identity: reader.string("evidence producer machine")?,
            realization_state_identity: reader.string("evidence producer state")?,
            source: match reader.u8()? {
                1 => EvidenceProducerRowSource::Inline,
                2 => EvidenceProducerRowSource::Reference,
                3 => EvidenceProducerRowSource::TraitDefault,
                tag => {
                    return Err(ProofCodecError::InvalidTag(
                        "EvidenceProducerRowSource",
                        tag,
                    ));
                }
            },
        });
    }
    Ok(EvidenceProducerProvenance {
        id,
        term,
        conformance_identity,
        evidence_trait_identity,
        rows,
    })
}

fn encode_evidence(
    writer: &mut Writer,
    evidence: &ObligationEvidence,
    format_marker: u16,
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
            writer.u16(certificate.proof_system_marker.get());
            encode_proof_node(writer, &certificate.proof, 0, format_marker)?;
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
    format_marker: u16,
) -> Result<(), ProofCodecError> {
    if depth > MAX_PROOF_DEPTH {
        return Err(ProofCodecError::ProofNestingTooDeep);
    }
    encode_proposition(writer, &node.conclusion, 0, format_marker)?;
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
                encode_proof_node(writer, node, depth + 1, format_marker)?;
            }
        }
        ProofRule::ConjunctionElimination {
            conjunction,
            conjunct,
        } => {
            writer.u8(5);
            encode_proof_node(writer, conjunction, depth + 1, format_marker)?;
            writer.index("conjunct index", *conjunct)?;
        }
        ProofRule::DisjunctionIntroduction { disjunct, index } => {
            writer.u8(9);
            encode_proof_node(writer, disjunct, depth + 1, format_marker)?;
            writer.index("disjunct index", *index)?;
        }
        ProofRule::ImplicationIntroduction { body } => {
            writer.u8(6);
            encode_proof_node(writer, body, depth + 1, format_marker)?;
        }
        ProofRule::ImplicationElimination {
            implication,
            premise,
        } => {
            writer.u8(7);
            encode_proof_node(writer, implication, depth + 1, format_marker)?;
            encode_proof_node(writer, premise, depth + 1, format_marker)?;
        }
        ProofRule::EqualityTransitivity {
            left_equals_middle,
            middle_equals_right,
        } => {
            writer.u8(8);
            encode_proof_node(writer, left_equals_middle, depth + 1, format_marker)?;
            encode_proof_node(writer, middle_equals_right, depth + 1, format_marker)?;
        }
        ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle,
            middle_less_or_equal_right,
        } => {
            writer.u8(10);
            encode_proof_node(writer, left_less_or_equal_middle, depth + 1, format_marker)?;
            encode_proof_node(writer, middle_less_or_equal_right, depth + 1, format_marker)?;
        }
        ProofRule::IntegerLessOrEqualSubstitution {
            relation,
            equality,
            endpoint,
        } => {
            writer.u8(11);
            encode_proof_node(writer, relation, depth + 1, format_marker)?;
            encode_proof_node(writer, equality, depth + 1, format_marker)?;
            writer.index("integer <= substitution endpoint", *endpoint)?;
        }
        ProofRule::IntegerAffineBound {
            root_bound,
            witness,
        } => {
            writer.u8(12);
            encode_proof_node(writer, root_bound, depth + 1, format_marker)?;
            encode_scalar_term(writer, &witness.root, 0, format_marker)?;
            encode_scalar_term(writer, &witness.target, 0, format_marker)?;
            writer.len(
                "integer affine definition axioms",
                witness.definition_axioms.len(),
            )?;
            for &index in &witness.definition_axioms {
                writer.index("integer affine definition axiom", index)?;
            }
            writer.len(
                "integer affine literal axioms",
                witness.literal_axioms.len(),
            )?;
            for &index in &witness.literal_axioms {
                match index {
                    None => writer.u8(0),
                    Some(index) => {
                        writer.u8(1);
                        writer.index("integer affine literal axiom", index)?;
                    }
                }
            }
        }
        ProofRule::IntegerCastBound {
            root_bound,
            witness,
        } => {
            writer.u8(13);
            encode_proof_node(writer, root_bound, depth + 1, format_marker)?;
            encode_scalar_term(writer, &witness.root, 0, format_marker)?;
            encode_scalar_term(writer, &witness.target, 0, format_marker)?;
            writer.len(
                "integer cast definition axioms",
                witness.definition_axioms.len(),
            )?;
            for &index in &witness.definition_axioms {
                writer.index("integer cast definition axiom", index)?;
            }
        }
    }
    Ok(())
}

fn encode_ieee_float_format(writer: &mut Writer, format: IeeeFloatFormat) {
    writer.u8(match format {
        IeeeFloatFormat::Binary32 => 1,
        IeeeFloatFormat::Binary64 => 2,
    });
}

fn encode_ieee_float_comparison_kind(writer: &mut Writer, kind: IeeeFloatComparisonKind) {
    writer.u8(match kind {
        IeeeFloatComparisonKind::Equal => 1,
        IeeeFloatComparisonKind::NotEqual => 2,
    });
}

fn decode_ieee_float_comparison_kind(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatComparisonKind, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatComparisonKind::Equal),
        2 => Ok(IeeeFloatComparisonKind::NotEqual),
        tag => Err(ProofCodecError::InvalidTag("IeeeFloatComparisonKind", tag)),
    }
}

fn decode_ieee_float_format(reader: &mut Reader<'_>) -> Result<IeeeFloatFormat, ProofCodecError> {
    match reader.u8()? {
        1 => Ok(IeeeFloatFormat::Binary32),
        2 => Ok(IeeeFloatFormat::Binary64),
        tag => Err(ProofCodecError::InvalidTag("IeeeFloatFormat", tag)),
    }
}

fn encode_ieee_float_field(
    writer: &mut Writer,
    field: &IeeeFloatStructuralField,
) -> Result<(), ProofCodecError> {
    encode_canonical_structural_field(writer, field.root(), field.path(), "IEEE float field path")
}

fn decode_ieee_float_field(
    reader: &mut Reader<'_>,
) -> Result<IeeeFloatStructuralField, ProofCodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    IeeeFloatStructuralField::new(root, path).map_err(ProofCodecError::MalformedProposition)
}

fn encode_byte_sequence_field(
    writer: &mut Writer,
    field: &ByteSequenceStructuralField,
) -> Result<(), ProofCodecError> {
    encode_canonical_structural_field(
        writer,
        field.root(),
        field.path(),
        "byte-sequence field path",
    )
}

fn decode_byte_sequence_field(
    reader: &mut Reader<'_>,
) -> Result<ByteSequenceStructuralField, ProofCodecError> {
    let (root, path) = decode_canonical_structural_field(reader)?;
    ByteSequenceStructuralField::new(root, path).map_err(ProofCodecError::MalformedProposition)
}

fn encode_canonical_structural_field(
    writer: &mut Writer,
    root: psi_core::PlaceId,
    path: &[CanonicalStructuralPathSegment],
    length_label: &'static str,
) -> Result<(), ProofCodecError> {
    writer.id(root);
    writer.len(length_label, path.len())?;
    for segment in path {
        match segment {
            CanonicalStructuralPathSegment::Field(field) => {
                writer.u8(1);
                writer.id(*field);
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                writer.u8(2);
                writer.u64(*index);
            }
            CanonicalStructuralPathSegment::Case(case) => {
                writer.u8(3);
                writer.id(*case);
            }
        }
    }
    Ok(())
}

fn decode_canonical_structural_field(
    reader: &mut Reader<'_>,
) -> Result<(psi_core::PlaceId, Vec<CanonicalStructuralPathSegment>), ProofCodecError> {
    let root = reader.id("PlaceId")?;
    let count = reader.count()?;
    let mut path = Vec::with_capacity(count as usize);
    for _ in 0..count {
        path.push(match reader.u8()? {
            1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
            2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
            3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
            tag => {
                return Err(ProofCodecError::InvalidTag(
                    "CanonicalStructuralPathSegment",
                    tag,
                ));
            }
        });
    }
    Ok((root, path))
}

fn encode_proposition(
    writer: &mut Writer,
    proposition: &Proposition,
    depth: usize,
    format_marker: u16,
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
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
        }
        Proposition::LessThan(left, right) => {
            writer.u8(5);
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
        }
        Proposition::LessOrEqual(left, right) => {
            writer.u8(6);
            encode_scalar_term(writer, left, 0, format_marker)?;
            encode_scalar_term(writer, right, 0, format_marker)?;
        }
        Proposition::Conjunction(conjuncts) => {
            writer.u8(7);
            writer.len("proof proposition conjuncts", conjuncts.len())?;
            for conjunct in conjuncts {
                encode_proposition(writer, conjunct, depth + 1, format_marker)?;
            }
        }
        Proposition::Implication {
            premise,
            conclusion,
        } => {
            writer.u8(8);
            encode_proposition(writer, premise, depth + 1, format_marker)?;
            encode_proposition(writer, conclusion, depth + 1, format_marker)?;
        }
        Proposition::ContentConservation(conservation) => {
            writer.u8(9);
            encode_content_algebra(writer, conservation.algebra())?;
            encode_content_term(writer, conservation.left(), 0, format_marker)?;
            encode_content_term(writer, conservation.right(), 0, format_marker)?;
        }
        Proposition::Disjunction(disjuncts) => {
            writer.u8(10);
            writer.len("proof proposition disjuncts", disjuncts.len())?;
            for disjunct in disjuncts {
                encode_proposition(writer, disjunct, depth + 1, format_marker)?;
            }
        }
        Proposition::IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => {
            writer.u8(11);
            encode_ieee_float_comparison_kind(writer, *kind);
            encode_ieee_float_format(writer, *format);
            encode_ieee_float_field(writer, left)?;
            encode_ieee_float_field(writer, right)?;
        }
        Proposition::ByteSequenceEqual { left, right } => {
            writer.u8(12);
            encode_byte_sequence_field(writer, left)?;
            encode_byte_sequence_field(writer, right)?;
        }
        Proposition::StructuralCaseMembership { subject, case } => {
            writer.u8(13);
            encode_canonical_structural_field(
                writer,
                subject.root(),
                subject.path(),
                "structural case subject path",
            )?;
            writer.id(*case);
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
    format_marker: u16,
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
                encode_content_term(writer, term, depth + 1, format_marker)?;
            }
        }
    }
    Ok(())
}

fn encode_scalar_term(
    writer: &mut Writer,
    term: &ScalarTerm,
    depth: usize,
    format_marker: u16,
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
        ScalarTerm::BooleanField { root, path } => {
            writer.u8(34);
            writer.id(*root);
            writer.len("Boolean field path", path.len())?;
            for segment in path {
                match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                }
            }
        }
        ScalarTerm::IntegerField {
            root,
            path,
            scalar_type,
        } => {
            writer.u8(35);
            writer.id(*root);
            writer.len("Integer field path", path.len())?;
            for segment in path {
                match segment {
                    CanonicalStructuralPathSegment::Field(field) => {
                        writer.u8(1);
                        writer.id(*field);
                    }
                    CanonicalStructuralPathSegment::FixedIndex(index) => {
                        writer.u8(2);
                        writer.u64(*index);
                    }
                    CanonicalStructuralPathSegment::Case(case) => {
                        writer.u8(3);
                        writer.id(*case);
                    }
                }
            }
            encode_integer_type(writer, *scalar_type);
        }
        ScalarTerm::Boolean(value) => {
            writer.u8(2);
            writer.u8(u8::from(*value));
        }
        ScalarTerm::BooleanNot { operand } => {
            writer.u8(10);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(25);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(26);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(27);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(28);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(29);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(30);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(31);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerDivide {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(32);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerRemainder {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(33);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(24);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_marker)?;
            encode_scalar_term(writer, count, depth + 1, format_marker)?;
        }
        ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value,
            count,
        } => {
            writer.u8(23);
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_marker)?;
            encode_scalar_term(writer, count, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerExactCast {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(22);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
        }
        ScalarTerm::BooleanEqual { left, right } => {
            writer.u8(11);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(12);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerLessThan {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(13);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerLessOrEqual {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(14);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
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
            writer.u8(match term {
                ScalarTerm::IntegerBitwiseAnd { .. } => 15,
                ScalarTerm::IntegerBitwiseOr { .. } => 16,
                ScalarTerm::IntegerBitwiseXor { .. } => 17,
                _ => unreachable!(),
            });
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
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
            writer.u8(match term {
                ScalarTerm::WrappingIntegerShiftLeft { .. } => 18,
                ScalarTerm::WrappingIntegerShiftRight { .. } => 19,
                _ => unreachable!(),
            });
            encode_integer_type(writer, *value_type);
            encode_integer_type(writer, *count_type);
            encode_scalar_term(writer, value, depth + 1, format_marker)?;
            encode_scalar_term(writer, count, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerBitwiseNot {
            scalar_type,
            operand,
        } => {
            writer.u8(20);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
        }
        ScalarTerm::IntegerWiden {
            source_type,
            target_type,
            operand,
        } => {
            writer.u8(21);
            encode_integer_type(writer, *source_type);
            encode_integer_type(writer, *target_type);
            encode_scalar_term(writer, operand, depth + 1, format_marker)?;
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
            writer.u8(4);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerAdd {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(5);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(6);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerSubtract {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(7);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::WrappingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(8);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
        }
        ScalarTerm::SaturatingIntegerMultiply {
            scalar_type,
            left,
            right,
        } => {
            writer.u8(9);
            encode_integer_type(writer, *scalar_type);
            encode_scalar_term(writer, left, depth + 1, format_marker)?;
            encode_scalar_term(writer, right, depth + 1, format_marker)?;
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
    format_marker: u16,
) -> Result<ObligationEvidence, ProofCodecError> {
    let obligation = reader.id("ObligationId")?;
    let route = match reader.u8()? {
        1 => EvidenceRoute::KernelDerived(decode_primitive(reader)?),
        2 => {
            let identity = reader.id("EvidenceIdentity")?;
            let raw_marker = reader.u16()?;
            EvidenceRoute::CertificateDerived(CertificateEnvelope {
                identity,
                proof_system_marker: ProofSystemMarker::new(raw_marker)
                    .ok_or(ProofCodecError::UnsupportedProofSystemMarker(raw_marker))?,
                proof: decode_proof_node(reader, 0, format_marker)?,
            })
        }
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
    format_marker: u16,
) -> Result<ProofNode, ProofCodecError> {
    if depth > MAX_PROOF_DEPTH {
        return Err(ProofCodecError::ProofNestingTooDeep);
    }
    let conclusion = decode_proposition(reader, 0, format_marker)?;
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
                nodes.push(decode_proof_node(reader, depth + 1, format_marker)?);
            }
            ProofRule::ConjunctionIntroduction(nodes)
        }
        5 => ProofRule::ConjunctionElimination {
            conjunction: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
            conjunct: reader.index()?,
        },
        6 => ProofRule::ImplicationIntroduction {
            body: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
        },
        7 => ProofRule::ImplicationElimination {
            implication: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
            premise: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
        },
        8 => ProofRule::EqualityTransitivity {
            left_equals_middle: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
            middle_equals_right: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
        },
        9 => ProofRule::DisjunctionIntroduction {
            disjunct: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
            index: reader.index()?,
        },
        10 => ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(decode_proof_node(
                reader,
                depth + 1,
                format_marker,
            )?),
            middle_less_or_equal_right: Box::new(decode_proof_node(
                reader,
                depth + 1,
                format_marker,
            )?),
        },
        11 => ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
            equality: Box::new(decode_proof_node(reader, depth + 1, format_marker)?),
            endpoint: reader.index()?,
        },
        12 => {
            let root_bound = Box::new(decode_proof_node(reader, depth + 1, format_marker)?);
            let root = decode_scalar_term(reader, 0, format_marker)?;
            let target = decode_scalar_term(reader, 0, format_marker)?;
            let definition_count = reader.count()?;
            let mut definition_axioms = Vec::new();
            for _ in 0..definition_count {
                definition_axioms.push(reader.index()?);
            }
            let literal_count = reader.count()?;
            let mut literal_axioms = Vec::new();
            for _ in 0..literal_count {
                literal_axioms.push(match reader.u8()? {
                    0 => None,
                    1 => Some(reader.index()?),
                    tag => return Err(ProofCodecError::UnknownIntegerAffineLiteralTag(tag)),
                });
            }
            ProofRule::IntegerAffineBound {
                root_bound,
                witness: IntegerAffineWitness {
                    root,
                    target,
                    definition_axioms,
                    literal_axioms,
                },
            }
        }
        13 => {
            let root_bound = Box::new(decode_proof_node(reader, depth + 1, format_marker)?);
            let root = decode_scalar_term(reader, 0, format_marker)?;
            let target = decode_scalar_term(reader, 0, format_marker)?;
            let definition_count = reader.count()?;
            let mut definition_axioms = Vec::new();
            for _ in 0..definition_count {
                definition_axioms.push(reader.index()?);
            }
            ProofRule::IntegerCastBound {
                root_bound,
                witness: IntegerCastChainWitness {
                    root,
                    target,
                    definition_axioms,
                },
            }
        }
        tag => return Err(ProofCodecError::InvalidTag("ProofRule", tag)),
    };
    Ok(ProofNode { conclusion, rule })
}

fn decode_proposition(
    reader: &mut Reader<'_>,
    depth: usize,
    format_marker: u16,
) -> Result<Proposition, ProofCodecError> {
    if depth > MAX_PROPOSITION_DEPTH {
        return Err(ProofCodecError::PropositionNestingTooDeep);
    }
    Ok(match reader.u8()? {
        1 => Proposition::Truth,
        2 => Proposition::Falsehood,
        3 => Proposition::Atom(reader.id::<PropositionId>("PropositionId")?),
        4 => Proposition::Equal(
            decode_scalar_term(reader, 0, format_marker)?,
            decode_scalar_term(reader, 0, format_marker)?,
        ),
        5 => Proposition::LessThan(
            decode_scalar_term(reader, 0, format_marker)?,
            decode_scalar_term(reader, 0, format_marker)?,
        ),
        6 => Proposition::LessOrEqual(
            decode_scalar_term(reader, 0, format_marker)?,
            decode_scalar_term(reader, 0, format_marker)?,
        ),
        7 => {
            let count = reader.count()?;
            let mut conjuncts = Vec::new();
            for _ in 0..count {
                conjuncts.push(decode_proposition(reader, depth + 1, format_marker)?);
            }
            Proposition::Conjunction(conjuncts)
        }
        8 => Proposition::Implication {
            premise: Box::new(decode_proposition(reader, depth + 1, format_marker)?),
            conclusion: Box::new(decode_proposition(reader, depth + 1, format_marker)?),
        },
        9 => {
            let algebra = decode_content_algebra(reader)?;
            let left = decode_content_term(reader, 0, format_marker)?;
            let right = decode_content_term(reader, 0, format_marker)?;
            Proposition::ContentConservation(ContentConservation::new(algebra, left, right))
        }
        10 => {
            let count = reader.count()?;
            let mut disjuncts = Vec::new();
            for _ in 0..count {
                disjuncts.push(decode_proposition(reader, depth + 1, format_marker)?);
            }
            Proposition::Disjunction(disjuncts)
        }
        11 => Proposition::IeeeFloatComparison {
            kind: decode_ieee_float_comparison_kind(reader)?,
            format: decode_ieee_float_format(reader)?,
            left: decode_ieee_float_field(reader)?,
            right: decode_ieee_float_field(reader)?,
        },
        12 => Proposition::ByteSequenceEqual {
            left: decode_byte_sequence_field(reader)?,
            right: decode_byte_sequence_field(reader)?,
        },
        13 => {
            let (root, path) = decode_canonical_structural_field(reader)?;
            Proposition::StructuralCaseMembership {
                subject: StructuralCaseSubject::new(root, path),
                case: reader.id("StructuralCaseId")?,
            }
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
    format_marker: u16,
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
                    3 => ContentPlaceSegment::Case(reader.string("content case")?),
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
                terms.push(decode_content_term(reader, depth + 1, format_marker)?);
            }
            ContentTerm::separate(terms).map_err(ProofCodecError::MalformedProposition)?
        }
        tag => return Err(ProofCodecError::InvalidTag("ContentTerm", tag)),
    })
}

fn decode_scalar_term(
    reader: &mut Reader<'_>,
    depth: usize,
    format_marker: u16,
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
        4 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        5 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        6 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        7 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        8 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        9 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        10 => ScalarTerm::boolean_not(decode_scalar_term(reader, depth + 1, format_marker)?)
            .map_err(ProofCodecError::MalformedProposition)?,
        11 => {
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::boolean_equal(left, right).map_err(ProofCodecError::MalformedProposition)?
        }
        12 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_equal(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        13 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_less_than(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        14 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_less_or_equal(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        15 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_and(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        16 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_or(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        17 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_xor(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        18 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_shift_left(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        19 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_shift_right(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        20 => {
            let scalar_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_bitwise_not(scalar_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        21 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_widen(source_type, target_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        22 => {
            let source_type = decode_integer_type(reader)?;
            let target_type = decode_integer_type(reader)?;
            let operand = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::integer_exact_cast(source_type, target_type, operand)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        23 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_shift_right(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        24 => {
            let value_type = decode_integer_type(reader)?;
            let count_type = decode_integer_type(reader)?;
            let value = decode_scalar_term(reader, depth + 1, format_marker)?;
            let count = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_shift_left(value_type, count_type, value, count)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        25 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_add(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        26 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_subtract(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        27 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_multiply(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        28 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        29 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::exact_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        30 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        31 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::wrapping_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        32 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_divide(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        33 => {
            let scalar_type = decode_integer_type(reader)?;
            let left = decode_scalar_term(reader, depth + 1, format_marker)?;
            let right = decode_scalar_term(reader, depth + 1, format_marker)?;
            ScalarTerm::saturating_integer_remainder(scalar_type, left, right)
                .map_err(ProofCodecError::MalformedProposition)?
        }
        34 => {
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut path = Vec::new();
            for _ in 0..count {
                path.push(match reader.u8()? {
                    1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    tag => {
                        return Err(ProofCodecError::InvalidTag(
                            "CanonicalStructuralPathSegment",
                            tag,
                        ));
                    }
                });
            }
            ScalarTerm::boolean_field_path(root, path)
        }
        35 => {
            let root = reader.id("PlaceId")?;
            let count = reader.count()?;
            let mut path = Vec::new();
            for _ in 0..count {
                path.push(match reader.u8()? {
                    1 => CanonicalStructuralPathSegment::Field(reader.id("StructuralFieldId")?),
                    2 => CanonicalStructuralPathSegment::FixedIndex(reader.u64()?),
                    3 => CanonicalStructuralPathSegment::Case(reader.id("StructuralCaseId")?),
                    tag => {
                        return Err(ProofCodecError::InvalidTag(
                            "CanonicalStructuralPathSegment",
                            tag,
                        ));
                    }
                });
            }
            ScalarTerm::integer_field_path(root, path, decode_integer_type(reader)?)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofCodecError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    UnsupportedProofSystemMarker(u16),
    UnknownIntegerAffineLiteralTag(u8),
    UnexpectedEnd,
    TrailingBytes(usize),
    InvalidBoolean(u8),
    InvalidTag(&'static str, u8),
    ZeroIdentity(&'static str),
    CollectionTooLong(&'static str),
    IndexTooLarge(&'static str),
    IndexOutsideHost,
    NonCanonicalEvidenceOrder,
    NonCanonicalEvidenceProducerOrder,
    NonCanonicalEvidenceProducerRows,
    InvalidEvidenceProducer,
    NonCanonicalEncoding,
    PropositionNestingTooDeep,
    ScalarTermNestingTooDeep,
    ContentTermNestingTooDeep,
    ProofNestingTooDeep,
    StringTooLong(&'static str),
    InvalidUtf8(&'static str),
    MalformedProposition(PropositionError),
    TrustGraph(crate::TrustGraphError),
}

impl std::fmt::Display for ProofCodecError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for ProofCodecError {}
