//! Target-neutral Psi optimization selection vocabulary.
//!
//! This crate owns only exact passes that can execute before Terminal Psi is
//! sealed. Target, instruction, allocation, machine, and layout selections
//! remain Omega-owned. A build coordinator may project a larger source-visible
//! selection into this closed set, but Psi never imports that larger catalog.

use sha2::{Digest, Sha256};
use std::fmt;

pub mod catalog;
pub use catalog::PRETERMINAL_PSI_PASS_CATALOG;

const ENCODING_MAGIC: &[u8; 8] = b"PSIOPT\0\0";
const ENCODING_VERSION: u32 = 1;
const IDENTITY_DOMAIN: &[u8] = b"psi.optimization-selections.v1\0";

/// One exact target-neutral optimization that executes before Terminal Psi is
/// published.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum PsiOptimization {
    ControlFlowCleanup = 1,
    SparseConditionalConstantPropagation = 2,
    CopyPropagation = 3,
    GlobalValueNumbering = 4,
    DeadPureScalarElimination = 5,
    ProofCheckElision = 6,
}

impl PsiOptimization {
    pub const ALL: [Self; 6] = crate::PRETERMINAL_PSI_PASS_CATALOG;

    pub const fn name(self) -> &'static str {
        match self {
            Self::ControlFlowCleanup => "ControlFlowCleanup",
            Self::SparseConditionalConstantPropagation => "SparseConditionalConstantPropagation",
            Self::CopyPropagation => "CopyPropagation",
            Self::GlobalValueNumbering => "GlobalValueNumbering",
            Self::DeadPureScalarElimination => "DeadPureScalarElimination",
            Self::ProofCheckElision => "ProofCheckElision",
        }
    }

    fn from_tag(tag: u8) -> Result<Self, PsiOptimizationSelectionDecodeError> {
        Self::ALL
            .into_iter()
            .find(|optimization| *optimization as u8 == tag)
            .ok_or(PsiOptimizationSelectionDecodeError::UnknownTag(tag))
    }
}

/// Canonical duplicate-free target-neutral Psi optimization selection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PsiOptimizationSelections {
    selected: Vec<PsiOptimization>,
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
        encoded.extend_from_slice(&ENCODING_VERSION.to_le_bytes());
        encoded.extend_from_slice(&count.to_le_bytes());
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
        let version = u32::from_le_bytes(encoded[8..12].try_into().expect("fixed version width"));
        if version != ENCODING_VERSION {
            return Err(PsiOptimizationSelectionDecodeError::UnsupportedVersion(
                version,
            ));
        }
        let count = u32::from_le_bytes(encoded[12..16].try_into().expect("fixed count width"));
        let count =
            usize::try_from(count).map_err(|_| PsiOptimizationSelectionDecodeError::Truncated)?;
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
        let mut digest = Sha256::new();
        digest.update(IDENTITY_DOMAIN);
        digest.update(
            u64::try_from(encoded.len())
                .expect("selection encoding length fits u64")
                .to_le_bytes(),
        );
        digest.update(encoded);
        PsiOptimizationSelectionIdentity(digest.finalize().into())
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
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn vocabulary_has_stable_contiguous_tags_and_unique_names() {
        let mut names = BTreeSet::new();
        for (index, optimization) in PsiOptimization::ALL.into_iter().enumerate() {
            assert_eq!(usize::from(optimization as u8), index + 1);
            assert!(names.insert(optimization.name()));
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
