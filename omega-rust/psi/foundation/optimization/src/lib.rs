/*
We define the target-neutral Psi optimization selection vocabulary, canonical wire
codec, and shared bounded-cursor and SHA-256 domain-digest primitives consumed across
Psi representation and identity crates.

Our `PsiOptimization` enum declares the closed 9-member pre- and post-Terminal Psi
optimization vocabulary (`ControlFlowCleanup` through `LoopInvariantScalarMotion`) via
`psi_optimization_vocabulary!`, generating canonical ascending-tag order, wire tag
resolution, and diagnostic names from a single declaration table. `PsiOptimizationSelections`
maintains a sorted, duplicate-free selection set that encodes into the 16-byte `PSIOPT\0\0`
v1 wire header plus 1-byte tags and hashes under `"psi.optimization-selections.v1\0"`
via `DomainDigestEncoder` to produce `PsiOptimizationSelectionIdentity`. Alongside the
selection vocabulary, `codec_bytes`, `cursor`, `digest_encoder`, and `variant_accessors`
provide the shared wire-encoding helper (`push_encoded`), bounded decode cursor (`Cursor`,
`Truncated`, `TakeError`, `truncated_from_adapter!`), domain-tagged SHA-256 encoder
(`DomainDigestEncoder`), and structural result-variant accessor macro
(`structural_variant_accessor!`).
*/

/*
Psi owns only the target-neutral optimization passes that can be selected on portable
Psi artifacts; target, instruction, register-allocation, machine, and layout selections
belong to Omega. Keeping `PsiOptimization` and `PsiOptimizationSelections` in a dedicated
foundation crate allows build coordinators to project source-visible optimization policies
into Psi's closed set without Psi ever importing the larger target-aware catalog. Hosting
`Cursor` and `DomainDigestEncoder` here also eliminates crate-local copies of the byte
cursor and SHA-256 framing protocol across Psi identity and artifact codecs while
keeping every previously computed digest byte-for-byte identical.
*/

/*
We consolidated `codec_bytes.rs`, `cursor.rs`, `digest_encoder.rs`,
`optimization_selections.rs`, and `variant_accessors.rs` into `src/lib.rs`, preserving
the public submodule paths (`codec_bytes`, `cursor`, `digest_encoder`,
`optimization_selections`, and `variant_accessors`) and macro exports while eliminating
five micro-files.
*/

#![forbid(unsafe_code)]

pub mod codec_bytes {
    pub fn push_encoded<const N: usize>(buffer: &mut Vec<u8>, encoded: [u8; N]) {
        buffer.extend_from_slice(&encoded);
    }
}

pub mod cursor {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Truncated;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TakeError {
        Truncated,      // Cursor exhausted before the requested read completed.
        LengthOverflow, // Decoded length exceeded the bounded target range.
    }

    pub struct Cursor<'a> {
        encoded: &'a [u8], // Borrowed wire payload slice.
        pub offset: usize, // Next unread byte position within `encoded`.
    }

    impl<'a> Cursor<'a> {
        pub const fn new(encoded: &'a [u8]) -> Self {
            Self { encoded, offset: 0 }
        }

        pub fn remaining(&self) -> usize {
            self.encoded.len().saturating_sub(self.offset)
        }

        pub fn take(&mut self, count: usize) -> Result<&'a [u8], Truncated> {
            let end = self.offset.checked_add(count).ok_or(Truncated)?;
            let value = self.encoded.get(self.offset..end).ok_or(Truncated)?;
            self.offset = end;
            Ok(value)
        }

        pub fn array<const N: usize>(&mut self) -> Result<[u8; N], Truncated> {
            self.take(N)?.try_into().map_err(|_| Truncated)
        }

        pub fn byte(&mut self) -> Result<u8, Truncated> {
            Ok(self.array::<1>()?[0])
        }

        pub fn u16(&mut self) -> Result<u16, Truncated> {
            Ok(u16::from_le_bytes(self.array()?))
        }

        pub fn u64(&mut self) -> Result<u64, Truncated> {
            Ok(u64::from_le_bytes(self.array()?))
        }
    }

    #[macro_export]
    macro_rules! truncated_from_adapter {
        ($error:ty) => {
            impl From<$crate::cursor::Truncated> for $error {
                fn from(_: $crate::cursor::Truncated) -> Self {
                    Self::Truncated
                }
            }
        };
        ($error:ty => $variant:expr) => {
            impl From<$crate::cursor::Truncated> for $error {
                fn from(_: $crate::cursor::Truncated) -> Self {
                    $variant
                }
            }
        };
    }
}

pub mod digest_encoder {
    use sha2::{Digest, Sha256};

    pub struct DomainDigestEncoder(Sha256);

    impl DomainDigestEncoder {
        pub fn new(domain: &[u8]) -> Self {
            let mut digest = Sha256::new();
            digest.update(domain);
            Self(digest)
        }

        pub fn finish(self) -> [u8; 32] {
            self.0.finalize().into()
        }

        pub fn byte(&mut self, value: u8) {
            self.0.update([value]);
        }

        pub fn len(&mut self, value: usize) {
            self.0.update((value as u64).to_le_bytes());
        }

        pub fn u16(&mut self, value: u16) {
            self.0.update(value.to_le_bytes());
        }

        pub fn u64(&mut self, value: u64) {
            self.0.update(value.to_le_bytes());
        }

        pub fn raw(&mut self, bytes: &[u8]) {
            self.0.update(bytes);
        }

        pub fn bytes(&mut self, bytes: &[u8]) {
            self.len(bytes.len());
            self.0.update(bytes);
        }
    }
}

pub mod variant_accessors {
    #[macro_export]
    macro_rules! structural_variant_accessor {
        ($result:ty; $($rest:tt)*) => {
            pub const fn structural(&self) -> Option<&$result> {
                match self {
                    Self::Structural(result) => Some(result),
                    $($rest)* => None,
                }
            }
        };
    }
}

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
            crate::codec_bytes::push_encoded(&mut encoded, ENCODING_VERSION.to_le_bytes());
            crate::codec_bytes::push_encoded(&mut encoded, count.to_le_bytes());
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
            let mut digest = crate::digest_encoder::DomainDigestEncoder::new(IDENTITY_DOMAIN);
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
