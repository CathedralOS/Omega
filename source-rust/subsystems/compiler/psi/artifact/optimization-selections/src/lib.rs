//! Canonical target-neutral optimization request records, tags and identity encoding.
#![forbid(unsafe_code)]

pub mod optimization_selections {
    use std::fmt;

    const ENCODING_MAGIC: &[u8; 8] = b"PSIOPT\0\0";
    const ENCODING_VERSION: u32 = 1;
    const IDENTITY_DOMAIN: &[u8] = b"psi.optimization-selections.v1\0";

    macro_rules! psi_optimization_vocabulary {
        ($count:literal; $( $(#[$attribute:meta])* $variant:ident = $tag:literal ),+ $(,)?) => {
            #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            #[repr(u8)]
            pub enum PsiOptimization {
                $( $(#[$attribute])* $variant = $tag ),+
            }

            impl PsiOptimization {
                pub const ALL: [Self; $count] = [$(Self::$variant),+];

                pub const fn name(self) -> &'static str {
                    match self {
                        $(Self::$variant => stringify!($variant)),+
                    }
                }

                fn from_tag(tag: u8) -> Result<Self, PsiOptimizationSelectionDecodeError> {
                    Self::ALL
                        .into_iter()
                        .find(|optimization| *optimization as u8 == tag)
                        .ok_or(PsiOptimizationSelectionDecodeError::UnknownTag(tag))
                }
            }
        };
    }

    psi_optimization_vocabulary! {
        9;
        ControlFlowCleanup = 1,
        SparseConditionalConstantPropagation = 2,
        CopyPropagation = 3,
        GlobalValueNumbering = 4,
        DeadPureScalarElimination = 5,
        ProofCheckElision = 6,
        StateSpecialization = 7,
        RepresentationSpecialization = 8,
        LoopInvariantScalarMotion = 9,
    }

    #[derive(Debug, Clone, Default, PartialEq, Eq)]
    pub struct PsiOptimizationSelections {
        selected: Vec<PsiOptimization>, // Canonical ascending-tag duplicate-free selections.
    }

    impl PsiOptimizationSelections {
        pub fn new(
            selected: impl IntoIterator<Item = PsiOptimization>,
        ) -> Result<Self, DuplicatePsiOptimization> {
            let mut selected = selected.into_iter().collect::<Vec<_>>();
            selected.sort_unstable();
            for pair in selected.windows(2) {
                if pair[0] == pair[1] {
                    return Err(DuplicatePsiOptimization(pair[0]));
                }
            }
            Ok(Self { selected })
        }

        pub fn as_slice(&self) -> &[PsiOptimization] {
            &self.selected
        }

        pub fn is_empty(&self) -> bool {
            self.selected.is_empty()
        }

        pub fn contains(&self, optimization: PsiOptimization) -> bool {
            self.selected.binary_search(&optimization).is_ok()
        }

        pub fn encode(&self) -> Vec<u8> {
            let count = u32::try_from(self.selected.len())
                .expect("the closed Psi optimization vocabulary fits in u32");
            let mut encoded = Vec::with_capacity(16 + self.selected.len());
            encoded.extend_from_slice(ENCODING_MAGIC);
            ::wire::codec_bytes::push_encoded(&mut encoded, ENCODING_VERSION.to_le_bytes());
            ::wire::codec_bytes::push_encoded(&mut encoded, count.to_le_bytes());
            encoded.extend(self.selected.iter().map(|optimization| *optimization as u8));
            encoded
        }

        pub fn decode(encoded: &[u8]) -> Result<Self, PsiOptimizationSelectionDecodeError> {
            if encoded.len() < 16 {
                return Err(PsiOptimizationSelectionDecodeError::Truncated);
            }
            if &encoded[..8] != ENCODING_MAGIC {
                return Err(PsiOptimizationSelectionDecodeError::WrongMagic);
            }
            let version =
                u32::from_le_bytes(encoded[8..12].try_into().expect("fixed version width"));
            if version != ENCODING_VERSION {
                return Err(PsiOptimizationSelectionDecodeError::UnsupportedVersion(
                    version,
                ));
            }
            let count = u32::from_le_bytes(encoded[12..16].try_into().expect("fixed count width"));
            let count = usize::try_from(count)
                .map_err(|_| PsiOptimizationSelectionDecodeError::Truncated)?;
            let expected = 16usize
                .checked_add(count)
                .ok_or(PsiOptimizationSelectionDecodeError::Truncated)?;
            if encoded.len() < expected {
                return Err(PsiOptimizationSelectionDecodeError::Truncated);
            }
            if encoded.len() > expected {
                return Err(PsiOptimizationSelectionDecodeError::TrailingBytes);
            }

            let mut previous = None;
            let mut selected = Vec::with_capacity(count);
            for tag in &encoded[16..] {
                let optimization = PsiOptimization::from_tag(*tag)?;
                if let Some(previous) = previous {
                    if optimization == previous {
                        return Err(PsiOptimizationSelectionDecodeError::Duplicate(optimization));
                    }
                    if optimization < previous {
                        return Err(PsiOptimizationSelectionDecodeError::NonCanonicalOrder);
                    }
                }
                previous = Some(optimization);
                selected.push(optimization);
            }
            Ok(Self { selected })
        }

        pub fn identity(&self) -> PsiOptimizationSelectionIdentity {
            let encoded = self.encode();
            let mut digest = ::wire::digest_encoder::DomainDigestEncoder::new(IDENTITY_DOMAIN);
            digest.bytes(&encoded);
            PsiOptimizationSelectionIdentity(digest.finish())
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PsiOptimizationSelectionIdentity([u8; 32]);

    impl PsiOptimizationSelectionIdentity {
        pub const fn from_bytes(bytes: [u8; 32]) -> Self {
            Self(bytes)
        }

        pub const fn bytes(self) -> [u8; 32] {
            self.0
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DuplicatePsiOptimization(pub PsiOptimization);

    impl fmt::Display for DuplicatePsiOptimization {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "duplicate Psi optimization `{}`", self.0.name())
        }
    }

    impl std::error::Error for DuplicatePsiOptimization {}

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum PsiOptimizationSelectionDecodeError {
        Truncated,
        WrongMagic,
        UnsupportedVersion(u32),
        UnknownTag(u8),
        Duplicate(PsiOptimization),
        NonCanonicalOrder,
        TrailingBytes,
    }

    impl fmt::Display for PsiOptimizationSelectionDecodeError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(formatter, "invalid Psi optimization selection: {self:?}")
        }
    }

    impl std::error::Error for PsiOptimizationSelectionDecodeError {}

    #[cfg(test)]
    mod tests {
        use super::{
            DuplicatePsiOptimization, PsiOptimization, PsiOptimizationSelectionDecodeError,
            PsiOptimizationSelections,
        };

        #[test]
        fn vocabulary_declares_stable_contiguous_tags() {
            for (index, optimization) in PsiOptimization::ALL.into_iter().enumerate() {
                assert_eq!(usize::from(optimization as u8), index + 1);
            }
        }

        #[test]
        fn selections_round_trip_and_empty_is_canonical() {
            for selections in [
                PsiOptimizationSelections::default(),
                PsiOptimizationSelections::new([
                    PsiOptimization::ProofCheckElision,
                    PsiOptimization::ControlFlowCleanup,
                ])
                .unwrap(),
            ] {
                let encoded = selections.encode();
                assert_eq!(
                    PsiOptimizationSelections::decode(&encoded).unwrap(),
                    selections
                );
                assert_eq!(
                    PsiOptimizationSelections::decode(&encoded)
                        .unwrap()
                        .identity(),
                    selections.identity()
                );
            }
        }

        #[test]
        fn duplicate_noncanonical_unknown_and_trailing_inputs_reject() {
            assert_eq!(
                PsiOptimizationSelections::new([
                    PsiOptimization::CopyPropagation,
                    PsiOptimization::CopyPropagation,
                ]),
                Err(DuplicatePsiOptimization(PsiOptimization::CopyPropagation))
            );

            let mut reversed = PsiOptimizationSelections::new([
                PsiOptimization::ControlFlowCleanup,
                PsiOptimization::CopyPropagation,
            ])
            .unwrap()
            .encode();
            reversed[16..].reverse();
            assert_eq!(
                PsiOptimizationSelections::decode(&reversed),
                Err(PsiOptimizationSelectionDecodeError::NonCanonicalOrder)
            );

            let mut unknown = PsiOptimizationSelections::default().encode();
            unknown[12..16].copy_from_slice(&1_u32.to_le_bytes());
            unknown.push(255);
            assert_eq!(
                PsiOptimizationSelections::decode(&unknown),
                Err(PsiOptimizationSelectionDecodeError::UnknownTag(255))
            );

            let mut trailing = PsiOptimizationSelections::default().encode();
            trailing.push(0);
            assert_eq!(
                PsiOptimizationSelections::decode(&trailing),
                Err(PsiOptimizationSelectionDecodeError::TrailingBytes)
            );
        }
    }
}

pub use optimization_selections::{
    DuplicatePsiOptimization, PsiOptimization, PsiOptimizationSelectionDecodeError,
    PsiOptimizationSelectionIdentity, PsiOptimizationSelections,
};
