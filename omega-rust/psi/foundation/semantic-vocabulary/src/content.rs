use std::collections::BTreeMap;

use crate::{
    BoundaryMachineId, ContentDomainId, PlaceId, PropositionError, StructuralFieldId,
    StructuralTypeId,
};

/// One compiler-owned closed content algebra and its normalized parameter
/// identity. The parameter is the canonical coordinate-space or unit type
/// identity emitted by the Psi frontend.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentAlgebra {
    pub kind: ContentAlgebraKind,
    pub parameter: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentAlgebraKind {
    IntervalSet,
    CountedQuantity,
}

/// Owner-local content-projection coordinate selected by one qualification.
///
/// The compact report fingerprint is non-authoritative: the owning structural
/// domain retains the exact algebra and expression, and every use rejoins and
/// replays that definition. Arena-local machine symbols are deliberately not
/// part of terminal Psi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentProjectionIdentity {
    pub domain: ContentDomainId,
    pub projection_report_fingerprint: u64,
}

/// Source-handle-free scalar expression defining one installed root's
/// per-occurrence capacity. Field segments are stable structural identities,
/// never frontend arena handles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentProjectionScalar {
    SubjectField(Vec<String>),
    RuntimeScalarEmbedding(Vec<String>),
    Natural(String),
    Successor(Box<Self>),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
}

/// Exact finite content expression selected by the qualification owner's
/// `Content<A>` projection. This is semantic schema, not a trusted manifest
/// total and not an authority occurrence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentProjectionExpression {
    IntervalSet(Vec<(ContentProjectionScalar, ContentProjectionScalar)>),
    CountedQuantity(ContentProjectionScalar),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentPlaceVersion {
    Entry,
    Current,
}

/// Role of a module-declared structural-place root.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StructuralPlaceKind {
    Parameter {
        position: u32,
        is_self: bool,
    },
    Result,
    /// One structural operation result, established only after the exact
    /// producer succeeds. The producer identity prevents a result place from
    /// being treated as live before or independently of that operation.
    OperationResult {
        producer: crate::OperationId,
        structural_type: StructuralTypeId,
    },
    /// One immutable borrowed byte-sequence literal established by an exact
    /// terminal operation. The declaration ordinal makes source-order
    /// identity explicit without retaining a source-tree handle.
    ByteSequenceLiteral {
        declaration_ordinal: u32,
        structural_type: StructuralTypeId,
    },
    /// One authored provider-backed attachment field specialized out of the
    /// runtime record into an exact bodyless boundary requirement. Installation
    /// must bind the named boundary; this root is semantic evidence and has no
    /// target layout of its own.
    ProviderAttachment {
        attachment: StructuralTypeId,
        field: StructuralFieldId,
        boundary: BoundaryMachineId,
    },
    /// One whole, claim-free affine local established by an explicit terminal
    /// operation. The exact concrete type and source declaration coordinate
    /// make trivial disposal independently checkable without retaining a
    /// source-tree handle.
    TrivialAffineLocal {
        declaration_ordinal: u32,
        structural_type: StructuralTypeId,
        /// Present only for one statically ordered element of an abandoned
        /// fixed-array construction. The root type and literal index make the
        /// prefix schedule independently replayable without turning the
        /// uninitialized aggregate into an ABI input.
        construction: Option<AffineConstructionElement>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AffineConstructionElement {
    pub root_structural_type: StructuralTypeId,
    pub index: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentPlaceSegment {
    /// Stable sum-case spelling within the statically typed parent path.
    Case(String),
    /// Stable field spelling within the statically typed parent path.
    Field(String),
    FixedIndex(u64),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentStructuralPlace {
    pub version: ContentPlaceVersion,
    pub root: PlaceId,
    pub segments: Vec<ContentPlaceSegment>,
}

/// Closed symbolic content term. `Separate` children are flat and strictly
/// ordered in canonical terminal modules.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ContentTerm {
    Projection {
        projection: ContentProjectionIdentity,
        subject: ContentStructuralPlace,
    },
    Separate(Vec<ContentTerm>),
}

impl ContentTerm {
    pub fn separate(terms: impl IntoIterator<Item = Self>) -> Result<Self, PropositionError> {
        let mut flattened = Vec::new();
        for term in terms {
            match term {
                Self::Separate(children) => flattened.extend(children),
                other => flattened.push(other),
            }
        }
        flattened.sort();
        if flattened.len() < 2 {
            return Err(PropositionError::NonCanonicalContentSeparationArity(
                flattened.len(),
            ));
        }
        if flattened.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PropositionError::NonCanonicalContentSeparationOrder);
        }
        Ok(Self::Separate(flattened))
    }
}

/// One normalized equality in one closed content algebra.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentConservation {
    algebra: ContentAlgebra,
    left: ContentTerm,
    right: ContentTerm,
}

impl ContentConservation {
    pub fn new(algebra: ContentAlgebra, left: ContentTerm, right: ContentTerm) -> Self {
        if left <= right {
            Self {
                algebra,
                left,
                right,
            }
        } else {
            Self {
                algebra,
                left: right,
                right: left,
            }
        }
    }

    pub const fn algebra(&self) -> &ContentAlgebra {
        &self.algebra
    }

    pub const fn left(&self) -> &ContentTerm {
        &self.left
    }

    pub const fn right(&self) -> &ContentTerm {
        &self.right
    }

    pub(crate) fn validate(&self) -> Result<(), PropositionError> {
        if self.algebra.parameter.is_empty() {
            return Err(PropositionError::EmptyContentAlgebraParameter);
        }
        if self.left > self.right {
            return Err(PropositionError::NonCanonicalContentEquationOrder);
        }
        validate_term(&self.left, 0)?;
        validate_term(&self.right, 0)
    }
}

/// Reconstruct the checked-language report fingerprint of one terminal
/// content equation. This compact value is non-authoritative: structural root
/// ids are machine-local representation, while theorem admission replays the
/// exact retained equation and substitution against the producer call.
///
/// `None` means the terminal row cannot have originated in the checked
/// fingerprint vocabulary (currently only a content-domain id wider than the
/// checked `u32` identity space, or an undeclared structural root).
pub fn content_conservation_report_fingerprint(
    conservation: &ContentConservation,
    structural_places: &BTreeMap<PlaceId, StructuralPlaceKind>,
) -> Option<u64> {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"content-conservation-v1");
    encode_fingerprint_algebra(conservation.algebra(), &mut bytes);
    encode_fingerprint_term(conservation.left(), structural_places, &mut bytes)?;
    encode_fingerprint_term(conservation.right(), structural_places, &mut bytes)?;
    Some(bytes.into_iter().fold(OFFSET, |mut hash, byte| {
        hash ^= u64::from(byte);
        hash.wrapping_mul(PRIME)
    }))
}

fn encode_fingerprint_algebra(algebra: &ContentAlgebra, output: &mut Vec<u8>) {
    output.push(match algebra.kind {
        ContentAlgebraKind::IntervalSet => 3,
        ContentAlgebraKind::CountedQuantity => 2,
    });
    encode_fingerprint_string(&algebra.parameter, output);
}

fn encode_fingerprint_term(
    term: &ContentTerm,
    structural_places: &BTreeMap<PlaceId, StructuralPlaceKind>,
    output: &mut Vec<u8>,
) -> Option<()> {
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            output.push(1);
            output.extend_from_slice(&u32::try_from(projection.domain.get()).ok()?.to_le_bytes());
            output.extend_from_slice(&projection.projection_report_fingerprint.to_le_bytes());
            output.push(match subject.version {
                ContentPlaceVersion::Entry => 1,
                ContentPlaceVersion::Current => 2,
            });
            match structural_places.get(&subject.root)? {
                StructuralPlaceKind::Parameter { position, is_self } => {
                    output.push(if *is_self { 2 } else { 1 });
                    output.extend_from_slice(&position.to_le_bytes());
                }
                StructuralPlaceKind::Result => output.push(3),
                StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type,
                } => {
                    output.push(4);
                    output.extend_from_slice(&producer.get().to_le_bytes());
                    output.extend_from_slice(&structural_type.get().to_le_bytes());
                }
                // Literal and trivial affine locals carry no claims or content
                // qualifications in the accepted slice, so they cannot become
                // content-proposition roots by merely being declared.
                StructuralPlaceKind::ByteSequenceLiteral { .. }
                | StructuralPlaceKind::ProviderAttachment { .. }
                | StructuralPlaceKind::TrivialAffineLocal { .. } => return None,
            }
            output.extend_from_slice(&(subject.segments.len() as u64).to_le_bytes());
            for segment in &subject.segments {
                match segment {
                    ContentPlaceSegment::Case(name) => {
                        output.push(3);
                        encode_fingerprint_string(name, output);
                    }
                    ContentPlaceSegment::Field(name) => {
                        output.push(1);
                        encode_fingerprint_string(name, output);
                    }
                    ContentPlaceSegment::FixedIndex(index) => {
                        output.push(2);
                        output.extend_from_slice(&index.to_le_bytes());
                    }
                }
            }
        }
        ContentTerm::Separate(terms) => {
            output.push(2);
            output.extend_from_slice(&(terms.len() as u64).to_le_bytes());
            for term in terms {
                encode_fingerprint_term(term, structural_places, output)?;
            }
        }
    }
    Some(())
}

fn encode_fingerprint_string(value: &str, output: &mut Vec<u8>) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

const MAX_CONTENT_TERM_DEPTH: usize = 256;

fn validate_term(term: &ContentTerm, depth: usize) -> Result<(), PropositionError> {
    if depth > MAX_CONTENT_TERM_DEPTH {
        return Err(PropositionError::ContentTermNestingTooDeep);
    }
    match term {
        ContentTerm::Projection {
            projection,
            subject,
        } => {
            if projection.projection_report_fingerprint == 0 {
                return Err(PropositionError::ZeroContentProjectionFingerprint);
            }
            if subject.segments.iter().any(
                |segment| matches!(segment, ContentPlaceSegment::Case(name) if name.is_empty()),
            ) {
                return Err(PropositionError::EmptyContentCaseName);
            }
            if subject.segments.iter().any(
                |segment| matches!(segment, ContentPlaceSegment::Field(name) if name.is_empty()),
            ) {
                return Err(PropositionError::EmptyContentFieldName);
            }
            Ok(())
        }
        ContentTerm::Separate(terms) => {
            if terms.len() < 2 {
                return Err(PropositionError::NonCanonicalContentSeparationArity(
                    terms.len(),
                ));
            }
            if terms
                .iter()
                .any(|term| matches!(term, ContentTerm::Separate(_)))
            {
                return Err(PropositionError::NestedContentSeparation);
            }
            if terms.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(PropositionError::NonCanonicalContentSeparationOrder);
            }
            for term in terms {
                validate_term(term, depth + 1)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Proposition, PropositionContext};

    fn projection(root: PlaceId, version: ContentPlaceVersion, field: &str) -> ContentTerm {
        ContentTerm::Projection {
            projection: ContentProjectionIdentity {
                domain: ContentDomainId::new(1).expect("domain"),
                projection_report_fingerprint: 2,
            },
            subject: ContentStructuralPlace {
                version,
                root,
                segments: vec![ContentPlaceSegment::Field(field.to_owned())],
            },
        }
    }

    #[test]
    fn separate_flattens_and_sorts_but_rejects_duplicate_authority() {
        let root = PlaceId::new(1).expect("place");
        let a = projection(root, ContentPlaceVersion::Current, "a");
        let b = projection(root, ContentPlaceVersion::Current, "b");
        let c = projection(root, ContentPlaceVersion::Current, "c");
        let nested = ContentTerm::Separate(vec![c.clone(), a.clone()]);

        assert_eq!(
            ContentTerm::separate([b.clone(), nested]),
            Ok(ContentTerm::Separate(vec![a.clone(), b.clone(), c]))
        );
        assert_eq!(
            ContentTerm::separate([a.clone(), a]),
            Err(PropositionError::NonCanonicalContentSeparationOrder)
        );
    }

    #[test]
    fn structural_place_context_rejects_unknown_roots_and_entry_results() {
        let parameter = PlaceId::new(1).expect("parameter");
        let result = PlaceId::new(2).expect("result");
        let algebra = ContentAlgebra {
            kind: ContentAlgebraKind::IntervalSet,
            parameter: "Address".to_owned(),
        };
        let proposition = |term: ContentTerm| {
            Proposition::ContentConservation(ContentConservation::new(
                algebra.clone(),
                term.clone(),
                term,
            ))
        };
        let context = PropositionContext::from_value_types_and_places(
            [],
            [
                (
                    parameter,
                    StructuralPlaceKind::Parameter {
                        position: 0,
                        is_self: false,
                    },
                ),
                (result, StructuralPlaceKind::Result),
            ],
        )
        .expect("context");

        assert_eq!(
            context.validate(&proposition(projection(
                PlaceId::new(3).expect("unknown"),
                ContentPlaceVersion::Current,
                "bytes",
            ))),
            Err(PropositionError::UnknownStructuralPlace(
                PlaceId::new(3).expect("unknown")
            ))
        );
        assert_eq!(
            context.validate(&proposition(projection(
                result,
                ContentPlaceVersion::Entry,
                "bytes",
            ))),
            Err(PropositionError::EntryResultStructuralPlace(result))
        );
    }
}
