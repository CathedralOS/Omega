/*
We define source-authored bit-fragment and record storage geometry
(`authored_storage`), target-supplied carrier layout (`target_carrier_layout`),
atomic cell types (`atomic_cell`), structural place roots and affine
construction coordinates (`structural_places`), reference origin extents
(`reference_origins`), and content conservation projections (`content`).
*/

pub(crate) mod atomic_cell {
    use crate::{BoundedIntegerType, IntegerType, ScalarType};

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum AtomicCellType {
        Boolean,
        Integer(IntegerType),
        BoundedInteger(BoundedIntegerType),
    }

    impl AtomicCellType {
        pub fn from_scalar(scalar: ScalarType) -> Option<Self> {
            match scalar {
                ScalarType::Boolean => Some(Self::Boolean),
                ScalarType::Integer(integer) => Self::Integer(integer)
                    .is_supported()
                    .then_some(Self::Integer(integer)),
                _ => None,
            }
        }

        pub const fn scalar_type(self) -> ScalarType {
            match self {
                Self::Boolean => ScalarType::Boolean,
                Self::Integer(integer) => ScalarType::Integer(integer),
                Self::BoundedInteger(bounds) => ScalarType::Integer(bounds.integer_type()),
            }
        }

        pub const fn bounds(self) -> Option<BoundedIntegerType> {
            match self {
                Self::BoundedInteger(bounds) => Some(bounds),
                _ => None,
            }
        }

        pub fn is_supported(self) -> bool {
            match self.scalar_type() {
                ScalarType::Boolean => true,
                ScalarType::Integer(integer) => {
                    !integer.is_address() && matches!(integer.bits(), Some(8 | 16 | 32 | 64))
                }
                _ => false,
            }
        }
    }
}

pub(crate) mod authored_storage {
    use crate::ScalarType;
    use arena::{Arena, HandleSpan};
    use std::cmp::Ordering;
    use std::hash::{Hash, Hasher};

    #[derive(
        Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct AuthoredBitFragment {
        pub container_byte_offset: u64,
        pub container_width_bits: u64,
        pub destination_lsb: u64,
        pub source_lsb: u64,
        pub width: u64,
    }

    #[derive(Debug, Clone, compiler_storage::Record)]
    pub struct AuthoredBitFragments {
        storage: Arena<AuthoredBitFragment>,
        fragments: HandleSpan<AuthoredBitFragment>,
    }

    impl AuthoredBitFragments {
        pub fn new(fragments: impl IntoIterator<Item = AuthoredBitFragment>) -> Self {
            let mut rows: Vec<_> = fragments.into_iter().collect();
            rows.sort_unstable();
            let mut storage = Arena::new();
            let fragments = storage.insert_many(rows);
            Self { storage, fragments }
        }

        pub fn fragments(&self) -> &[AuthoredBitFragment] {
            self.storage.span_or_empty(self.fragments)
        }

        pub fn byte_envelope(&self) -> Option<(u64, u64)> {
            let mut start = u64::MAX;
            let mut end = 0;
            for fragment in self.fragments() {
                if fragment.container_width_bits == 0
                    || !fragment.container_width_bits.is_multiple_of(8)
                {
                    return None;
                }
                start = start.min(fragment.container_byte_offset);
                end = end.max(
                    fragment
                        .container_byte_offset
                        .checked_add(fragment.container_width_bits / 8)?,
                );
            }
            Some((start, end.checked_sub(start)?))
        }
    }

    impl PartialEq for AuthoredBitFragments {
        fn eq(&self, other: &Self) -> bool {
            self.fragments() == other.fragments()
        }
    }
    impl Eq for AuthoredBitFragments {}
    impl PartialOrd for AuthoredBitFragments {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for AuthoredBitFragments {
        fn cmp(&self, other: &Self) -> Ordering {
            self.fragments().cmp(other.fragments())
        }
    }
    impl Hash for AuthoredBitFragments {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.fragments().hash(state);
        }
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct AuthoredRecordStorage {
        pub byte_size: u64,
        pub byte_alignment: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum AuthoredFieldStorage {
        Scalar {
            byte_offset: u64,
            stored_scalar: Option<ScalarType>,
        },
        Repeated {
            byte_offset: u64,
            element_stride: u64,
        },
        Direct {
            byte_offset: u64,
        },
        Fragmented(AuthoredBitFragments),
    }

    impl AuthoredFieldStorage {
        pub const fn byte_offset(&self) -> Option<u64> {
            match self {
                Self::Scalar { byte_offset, .. }
                | Self::Repeated { byte_offset, .. }
                | Self::Direct { byte_offset } => Some(*byte_offset),
                Self::Fragmented(_) => None,
            }
        }

        pub const fn stored_scalar(&self) -> Option<ScalarType> {
            match self {
                Self::Scalar { stored_scalar, .. } => *stored_scalar,
                _ => None,
            }
        }

        pub const fn element_stride(&self) -> Option<u64> {
            match self {
                Self::Repeated { element_stride, .. } => Some(*element_stride),
                _ => None,
            }
        }
    }
}

pub(crate) mod content {
    use std::collections::BTreeMap;

    use crate::{ContentDomainId, PlaceId, PropositionError, StructuralPlaceKind};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct ContentAlgebra {
        pub kind: ContentAlgebraKind,
        pub parameter: String,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum ContentAlgebraKind {
        IntervalSet,
        CountedQuantity,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct ContentProjectionIdentity {
        pub domain: ContentDomainId,
        pub projection_report_fingerprint: u64,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum ContentProjectionScalar {
        SubjectField(Vec<String>),
        RuntimeScalarEmbedding(Vec<String>),
        Natural(String),
        Successor(Box<Self>),
        Add(Box<Self>, Box<Self>),
        Subtract(Box<Self>, Box<Self>),
        Multiply(Box<Self>, Box<Self>),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum ContentProjectionExpression {
        IntervalSet(Vec<(ContentProjectionScalar, ContentProjectionScalar)>),
        CountedQuantity(ContentProjectionScalar),
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum ContentPlaceVersion {
        Entry,
        Current,
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub enum ContentPlaceSegment {
        Case(String),
        Field(String),
        FixedIndex(u64),
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct ContentStructuralPlace {
        pub version: ContentPlaceVersion,
        pub root: PlaceId,
        pub segments: Vec<ContentPlaceSegment>,
    }

    impl ContentStructuralPlace {
        pub fn is_case_alternative(&self, other: &Self) -> bool {
            self.root == other.root
                && self.version == other.version
                && self
                    .segments
                    .iter()
                    .zip(&other.segments)
                    .find(|(left, right)| left != right)
                    .is_some_and(|(left, right)| {
                        matches!(
                            (left, right),
                            (ContentPlaceSegment::Case(_), ContentPlaceSegment::Case(_))
                        )
                    })
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
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
            validate_term(&self.left)?;
            validate_term(&self.right)
        }
    }

    pub fn content_conservation_report_fingerprint(
        conservation: &ContentConservation,
        structural_places: &BTreeMap<PlaceId, StructuralPlaceKind>,
    ) -> Option<u64> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"content-conservation-v1");
        encode_fingerprint_algebra(conservation.algebra(), &mut bytes);
        encode_fingerprint_term(conservation.left(), structural_places, &mut bytes)?;
        encode_fingerprint_term(conservation.right(), structural_places, &mut bytes)?;
        let mut hash = crate::fnv1a::Fnv1a::new();
        hash.bytes(&bytes);
        Some(hash.finish())
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
                output
                    .extend_from_slice(&u32::try_from(projection.domain.get()).ok()?.to_le_bytes());
                crate::codec_bytes::push_encoded(
                    output,
                    projection.projection_report_fingerprint.to_le_bytes(),
                );
                output.push(match subject.version {
                    ContentPlaceVersion::Entry => 1,
                    ContentPlaceVersion::Current => 2,
                });
                match structural_places.get(&subject.root)? {
                    StructuralPlaceKind::Parameter { position } => {
                        output.push(1);
                        crate::codec_bytes::push_encoded(output, position.to_le_bytes());
                    }
                    StructuralPlaceKind::Result => output.push(3),
                    StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type,
                    } => {
                        output.push(4);
                        crate::codec_bytes::push_encoded(output, producer.get().to_le_bytes());
                        crate::codec_bytes::push_encoded(
                            output,
                            structural_type.get().to_le_bytes(),
                        );
                    }
                    StructuralPlaceKind::ByteSequenceLiteral { .. }
                    | StructuralPlaceKind::BlockParameter { .. }
                    | StructuralPlaceKind::ProviderAttachment { .. }
                    | StructuralPlaceKind::TrivialAffineLocal { .. } => return None,
                }
                crate::codec_bytes::push_encoded(
                    output,
                    (subject.segments.len() as u64).to_le_bytes(),
                );
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
                            crate::codec_bytes::push_encoded(output, index.to_le_bytes());
                        }
                    }
                }
            }
            ContentTerm::Separate(terms) => {
                output.push(2);
                crate::codec_bytes::push_encoded(output, (terms.len() as u64).to_le_bytes());
                for term in terms {
                    encode_fingerprint_term(term, structural_places, output)?;
                }
            }
        }
        Some(())
    }

    pub fn encode_fingerprint_string(value: &str, output: &mut Vec<u8>) {
        crate::codec_bytes::push_encoded(output, (value.len() as u64).to_le_bytes());
        output.extend_from_slice(value.as_bytes());
    }

    fn validate_term(term: &ContentTerm) -> Result<(), PropositionError> {
        let mut pending = vec![term];
        while let Some(term) = pending.pop() {
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
                    pending.extend(terms.iter().rev());
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::{
            BTreeMap, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
            ContentPlaceSegment, ContentPlaceVersion, ContentProjectionIdentity,
            ContentStructuralPlace, ContentTerm, PlaceId, PropositionError, StructuralPlaceKind,
            content_conservation_report_fingerprint,
        };
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
                    (parameter, StructuralPlaceKind::Parameter { position: 0 }),
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

        #[test]
        fn block_parameters_cannot_supply_entry_or_current_content_authority() {
            let place = PlaceId::new(1).expect("place");
            let places = BTreeMap::from([(
                place,
                StructuralPlaceKind::BlockParameter {
                    block: crate::BlockId::new(2).expect("block"),
                    position: 0,
                },
            )]);
            let context = PropositionContext::from_value_types_and_places(
                [],
                places.iter().map(|(place, kind)| (*place, *kind)),
            )
            .expect("block place context");
            for version in [ContentPlaceVersion::Entry, ContentPlaceVersion::Current] {
                let term = projection(place, version, "bytes");
                let conservation = ContentConservation::new(
                    ContentAlgebra {
                        kind: ContentAlgebraKind::IntervalSet,
                        parameter: "Address".into(),
                    },
                    term.clone(),
                    term,
                );
                assert_eq!(
                    context.validate(&Proposition::ContentConservation(conservation.clone())),
                    Err(PropositionError::UnsupportedContentLocalStructuralPlace(
                        place
                    ))
                );
                assert_eq!(
                    content_conservation_report_fingerprint(&conservation, &places),
                    None
                );
            }
        }
    }
}

pub(crate) mod reference_origins {
    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum ReferenceOriginExtent {
        Projected,
        WithinRegion,
    }
}

pub(crate) mod structural_places {
    use crate::{BoundaryMachineId, StructuralFieldId, StructuralTypeId};

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub enum StructuralPlaceKind {
        Parameter {
            position: u32,
        },
        BlockParameter {
            block: crate::BlockId,
            position: u32,
        },
        Result,
        OperationResult {
            producer: crate::OperationId,
            structural_type: StructuralTypeId,
        },
        ByteSequenceLiteral {
            declaration_ordinal: u32,
            structural_type: StructuralTypeId,
        },
        ProviderAttachment {
            attachment: StructuralTypeId,
            field: StructuralFieldId,
            boundary: BoundaryMachineId,
        },
        TrivialAffineLocal {
            declaration_ordinal: u32,
            structural_type: StructuralTypeId,
            construction: Option<AffineConstructionElement>,
        },
    }

    impl StructuralPlaceKind {
        pub fn produced_structural_type(&self) -> Option<StructuralTypeId> {
            match self {
                Self::OperationResult {
                    structural_type, ..
                }
                | Self::ByteSequenceLiteral {
                    structural_type, ..
                }
                | Self::TrivialAffineLocal {
                    structural_type, ..
                } => Some(*structural_type),
                _ => None,
            }
        }

        pub fn as_operation_result(&self) -> Option<(crate::OperationId, StructuralTypeId)> {
            match self {
                Self::OperationResult {
                    producer,
                    structural_type,
                } => Some((*producer, *structural_type)),
                _ => None,
            }
        }
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct AffineConstructionElement {
        pub root_structural_type: StructuralTypeId,
        pub index: u64,
    }
}

pub(crate) mod target_carrier_layout {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
    pub struct TargetCarrierLayout {
        pub pointer_byte_size: u64,
        pub pointer_byte_alignment: u64,
        pub descriptor_byte_size: u64,
        pub descriptor_byte_alignment: u64,
        pub case_tag_byte_size: u64,
        pub case_tag_byte_alignment: u64,
    }
}
