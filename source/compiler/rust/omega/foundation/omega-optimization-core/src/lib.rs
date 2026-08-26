//! Stable, target-independent identities for Omega optimization selection.
//!
//! This crate deliberately owns no optimizer registry, analysis manager, cost
//! model, or executable rewrite. An empty [`OptimizationSelections`] value is
//! therefore sufficient to keep the ordinary compiler path from constructing
//! optimizer machinery while explicit build selection is being brought up.

use sha2::{Digest, Sha256};
use std::fmt;

const SELECTION_ENCODING_MAGIC: &[u8; 8] = b"OMGOPT\0\0";
const SELECTION_ENCODING_VERSION: u32 = 1;
const SELECTION_IDENTITY_DOMAIN: &[u8] = b"omega.optimization-selections.v1\0";

/// One source-visible, semantics-preserving optimization family.
///
/// Declaration order is the canonical order used by selection encodings. It
/// is not a promise about pass scheduling; the pass manager will derive that
/// schedule from the exact selected set and declared dependencies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Optimization {
    ControlFlowCleanup = 1,
    SparseConditionalConstantPropagation = 2,
    CopyPropagation = 3,
    GlobalValueNumbering = 4,
    DeadPureScalarElimination = 5,
    ProofCheckElision = 6,
}

impl Optimization {
    pub const ALL: [Self; 6] = [
        Self::ControlFlowCleanup,
        Self::SparseConditionalConstantPropagation,
        Self::CopyPropagation,
        Self::GlobalValueNumbering,
        Self::DeadPureScalarElimination,
        Self::ProofCheckElision,
    ];

    pub const fn build_case_name(self) -> &'static str {
        match self {
            Self::ControlFlowCleanup => "ControlFlowCleanup",
            Self::SparseConditionalConstantPropagation => "SparseConditionalConstantPropagation",
            Self::CopyPropagation => "CopyPropagation",
            Self::GlobalValueNumbering => "GlobalValueNumbering",
            Self::DeadPureScalarElimination => "DeadPureScalarElimination",
            Self::ProofCheckElision => "ProofCheckElision",
        }
    }

    pub const fn build_counter_field(self) -> &'static str {
        match self {
            Self::ControlFlowCleanup => "control_flow_cleanup",
            Self::SparseConditionalConstantPropagation => "sparse_conditional_constant_propagation",
            Self::CopyPropagation => "copy_propagation",
            Self::GlobalValueNumbering => "global_value_numbering",
            Self::DeadPureScalarElimination => "dead_pure_scalar_elimination",
            Self::ProofCheckElision => "proof_check_elision",
        }
    }

    fn from_tag(tag: u8) -> Result<Self, SelectionDecodeError> {
        Self::ALL
            .into_iter()
            .find(|optimization| *optimization as u8 == tag)
            .ok_or(SelectionDecodeError::UnknownOptimizationTag(tag))
    }
}

/// A canonical, duplicate-free set of explicitly selected optimizations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationSelections {
    selected: Vec<Optimization>,
}

impl OptimizationSelections {
    pub fn new(
        selected: impl IntoIterator<Item = Optimization>,
    ) -> Result<Self, DuplicateOptimization> {
        let mut selected = selected.into_iter().collect::<Vec<_>>();
        selected.sort_unstable();
        for pair in selected.windows(2) {
            if pair[0] == pair[1] {
                return Err(DuplicateOptimization(pair[0]));
            }
        }
        Ok(Self { selected })
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn as_slice(&self) -> &[Optimization] {
        &self.selected
    }

    pub fn contains(&self, optimization: Optimization) -> bool {
        self.selected.binary_search(&optimization).is_ok()
    }

    /// Canonical standalone encoding for cache, artifact, and replay inputs.
    pub fn encode(&self) -> Vec<u8> {
        let count = u32::try_from(self.selected.len())
            .expect("the closed optimization vocabulary fits in u32");
        let mut encoded = Vec::with_capacity(16 + self.selected.len());
        encoded.extend_from_slice(SELECTION_ENCODING_MAGIC);
        encoded.extend_from_slice(&SELECTION_ENCODING_VERSION.to_le_bytes());
        encoded.extend_from_slice(&count.to_le_bytes());
        encoded.extend(self.selected.iter().map(|optimization| *optimization as u8));
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, SelectionDecodeError> {
        if encoded.len() < 16 {
            return Err(SelectionDecodeError::Truncated);
        }
        if &encoded[..8] != SELECTION_ENCODING_MAGIC {
            return Err(SelectionDecodeError::WrongMagic);
        }
        let version = u32::from_le_bytes(encoded[8..12].try_into().expect("fixed version width"));
        if version != SELECTION_ENCODING_VERSION {
            return Err(SelectionDecodeError::UnsupportedVersion(version));
        }
        let count = u32::from_le_bytes(encoded[12..16].try_into().expect("fixed count width"));
        let count = usize::try_from(count).map_err(|_| SelectionDecodeError::Truncated)?;
        let expected = 16usize
            .checked_add(count)
            .ok_or(SelectionDecodeError::Truncated)?;
        if encoded.len() < expected {
            return Err(SelectionDecodeError::Truncated);
        }
        if encoded.len() > expected {
            return Err(SelectionDecodeError::TrailingBytes);
        }
        let mut previous = None;
        let mut selected = Vec::with_capacity(count);
        for tag in &encoded[16..] {
            let optimization = Optimization::from_tag(*tag)?;
            if let Some(previous) = previous {
                if optimization == previous {
                    return Err(SelectionDecodeError::Duplicate(optimization));
                }
                if optimization < previous {
                    return Err(SelectionDecodeError::NonCanonicalOrder);
                }
            }
            previous = Some(optimization);
            selected.push(optimization);
        }
        Ok(Self { selected })
    }

    /// Domain-separated digest of the exact canonical selected set.
    pub fn identity(&self) -> OptimizationSelectionIdentity {
        let encoded = self.encode();
        let mut digest = Sha256::new();
        digest.update(SELECTION_IDENTITY_DOMAIN);
        digest.update(
            u64::try_from(encoded.len())
                .expect("selection encoding length fits u64")
                .to_le_bytes(),
        );
        digest.update(encoded);
        OptimizationSelectionIdentity(digest.finalize().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptimizationSelectionIdentity([u8; 32]);

impl OptimizationSelectionIdentity {
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateOptimization(pub Optimization);

impl fmt::Display for DuplicateOptimization {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "optimization `{}` is selected more than once",
            self.0.build_case_name()
        )
    }
}

impl std::error::Error for DuplicateOptimization {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionDecodeError {
    Truncated,
    WrongMagic,
    UnsupportedVersion(u32),
    UnknownOptimizationTag(u8),
    Duplicate(Optimization),
    NonCanonicalOrder,
    TrailingBytes,
}

impl fmt::Display for SelectionDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => formatter.write_str("optimization selection encoding is truncated"),
            Self::WrongMagic => {
                formatter.write_str("optimization selection encoding has wrong magic")
            }
            Self::UnsupportedVersion(version) => write!(
                formatter,
                "optimization selection encoding version {version} is unsupported"
            ),
            Self::UnknownOptimizationTag(tag) => {
                write!(formatter, "optimization selection has unknown tag {tag}")
            }
            Self::Duplicate(optimization) => write!(
                formatter,
                "optimization selection repeats `{}`",
                optimization.build_case_name()
            ),
            Self::NonCanonicalOrder => {
                formatter.write_str("optimization selection is not in canonical order")
            }
            Self::TrailingBytes => {
                formatter.write_str("optimization selection encoding has trailing bytes")
            }
        }
    }
}

impl std::error::Error for SelectionDecodeError {}

#[cfg(test)]
mod tests {
    use super::{Optimization, OptimizationSelections, SelectionDecodeError};

    #[test]
    fn selections_are_sorted_and_round_trip_canonically() {
        let selections = OptimizationSelections::new([
            Optimization::ProofCheckElision,
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("unique selections");
        assert_eq!(
            selections.as_slice(),
            &[
                Optimization::ControlFlowCleanup,
                Optimization::CopyPropagation,
                Optimization::ProofCheckElision,
            ]
        );
        let encoded = selections.encode();
        assert_eq!(
            OptimizationSelections::decode(&encoded).expect("canonical decode"),
            selections
        );
        assert_eq!(
            OptimizationSelections::decode(&encoded)
                .expect("repeat decode")
                .encode(),
            encoded
        );
    }

    #[test]
    fn duplicates_reject_before_identity() {
        let error = OptimizationSelections::new([
            Optimization::GlobalValueNumbering,
            Optimization::GlobalValueNumbering,
        ])
        .expect_err("duplicate selection must reject");
        assert_eq!(error.0, Optimization::GlobalValueNumbering);
    }

    #[test]
    fn decoder_rejects_noncanonical_and_trailing_encodings() {
        let selections = OptimizationSelections::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("unique selections");
        let mut reversed = selections.encode();
        reversed[16..].reverse();
        assert_eq!(
            OptimizationSelections::decode(&reversed),
            Err(SelectionDecodeError::NonCanonicalOrder)
        );

        let mut trailing = selections.encode();
        trailing.push(0);
        assert_eq!(
            OptimizationSelections::decode(&trailing),
            Err(SelectionDecodeError::TrailingBytes)
        );
    }

    #[test]
    fn identity_is_domain_stable_and_selection_sensitive() {
        let empty = OptimizationSelections::default().identity();
        let selected = OptimizationSelections::new([Optimization::ControlFlowCleanup])
            .expect("unique selection")
            .identity();
        assert_ne!(empty, selected);
        assert_eq!(empty, OptimizationSelections::default().identity());
    }
}
