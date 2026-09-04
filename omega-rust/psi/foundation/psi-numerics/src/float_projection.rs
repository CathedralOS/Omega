//! Closed proof projections from landed runtime floats to `FloatMeaning`.
//!
//! These rows are deliberately separate from integer embedding: floats never
//! project into proof `Int` or a bare `Rat`, and NaN payload identity is not
//! retained by the public proof carrier.

use crate::float_semantics::{FloatFormat, FloatMeaning};
use sha2::{Digest, Sha256};

/// Toolchain source that owns the closed public float-projection declarations.
/// A declaration with the same path and signature in any other source has no
/// projection semantics.
pub const FLOAT_PROJECTION_CORE_SOURCE: &str = "float_operations.omg";
/// Toolchain source that owns the proof-only projection result carrier.
pub const FLOAT_MEANING_CORE_SOURCE: &str = "float_meaning.omg";
/// Immutable version of the closed numeric laws consumed by D40 projection
/// reconstruction. This is semantic identity, not a display or codec version.
pub const FLOAT_PROJECTION_CATALOG_VERSION: u16 = 1;
pub const FLOAT_MEANING_RESULT_IDENTITY: &str = "toolchain::FloatMeaning";
const FLOAT_PROJECTION_CONTRACT_DOMAIN: &[u8] = b"psi-float-projection-contract\0";

/// Source-free identity of one sealed toolchain projection declaration and
/// the numeric catalog against which its complete signature was checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloatProjectionContractIdentity {
    pub format: u16,
    pub operation: u8,
    /// Rooted-checker declaration tag for the complete recognized operation,
    /// toolchain ownership, parameter shape, and FloatMeaning result contract.
    pub declaration: u8,
    pub catalog_version: u16,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatProjectionOperation {
    Meaning32,
    Meaning64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatProjectionRule {
    pub source_format: FloatFormat,
    pub finite_nonzero_is_exact_rational: bool,
    pub preserves_signed_zero: bool,
    pub preserves_signed_infinity: bool,
    pub erases_nan_payload: bool,
}

impl FloatProjectionOperation {
    pub fn contract_identity(self) -> FloatProjectionContractIdentity {
        let (format, operation, declaration, source_carrier) = match self {
            Self::Meaning32 => (32_u16, 1_u8, 1_u8, "f32"),
            Self::Meaning64 => (64_u16, 2_u8, 2_u8, "f64"),
        };
        let operator_identity = format!(
            "toolchain::{}::{}",
            self.source_namespace(),
            self.source_name(),
        );
        let mut hasher = Sha256::new();
        hasher.update(FLOAT_PROJECTION_CONTRACT_DOMAIN);
        hasher.update(format.to_le_bytes());
        hasher.update([operation]);
        hasher.update([declaration]);
        hasher.update(FLOAT_PROJECTION_CATALOG_VERSION.to_le_bytes());
        for owner in [
            FLOAT_PROJECTION_CORE_SOURCE,
            FLOAT_MEANING_CORE_SOURCE,
            operator_identity.as_str(),
            "private-contract-free-ordinary-tokenless-one-parameter",
            source_carrier,
            FLOAT_MEANING_RESULT_IDENTITY,
        ] {
            hasher.update((owner.len() as u64).to_le_bytes());
            hasher.update(owner.as_bytes());
        }
        FloatProjectionContractIdentity {
            format,
            operation,
            declaration,
            catalog_version: FLOAT_PROJECTION_CATALOG_VERSION,
            commitment: hasher.finalize().into(),
        }
    }

    pub const fn source_namespace(self) -> &'static str {
        "Float"
    }

    pub const fn source_name(self) -> &'static str {
        match self {
            Self::Meaning32 => "meaning32",
            Self::Meaning64 => "meaning64",
        }
    }

    /// Select only one exact source-visible projection identity. Leaf spelling
    /// alone never selects a proof projection.
    pub fn from_source_identity(namespace: &str, name: &str) -> Option<Self> {
        match (namespace, name) {
            ("Float", "meaning32") => Some(Self::Meaning32),
            ("Float", "meaning64") => Some(Self::Meaning64),
            _ => None,
        }
    }

    pub const fn rule(self) -> FloatProjectionRule {
        FloatProjectionRule {
            source_format: match self {
                Self::Meaning32 => FloatFormat::BINARY32,
                Self::Meaning64 => FloatFormat::BINARY64,
            },
            finite_nonzero_is_exact_rational: true,
            preserves_signed_zero: true,
            preserves_signed_infinity: true,
            erases_nan_payload: true,
        }
    }

    /// Project one f32 only through the exact `Float::meaning32` row.
    pub fn project_f32(self, value: f32) -> Option<FloatMeaning> {
        (self == Self::Meaning32).then(|| FloatMeaning::from_f32(value))
    }

    /// Project one f64 only through the exact `Float::meaning64` row.
    pub fn project_f64(self, value: f64) -> Option<FloatMeaning> {
        (self == Self::Meaning64).then(|| FloatMeaning::from_f64(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_rows_are_format_exact() {
        assert_eq!(
            FloatProjectionOperation::Meaning32.rule().source_format,
            FloatFormat::BINARY32
        );
        assert_eq!(
            FloatProjectionOperation::Meaning64.rule().source_format,
            FloatFormat::BINARY64
        );
        assert!(
            FloatProjectionOperation::Meaning32
                .project_f64(1.0)
                .is_none()
        );
        assert!(
            FloatProjectionOperation::Meaning64
                .project_f32(1.0)
                .is_none()
        );
    }

    #[test]
    fn source_identities_are_exact_and_closed() {
        for operation in [
            FloatProjectionOperation::Meaning32,
            FloatProjectionOperation::Meaning64,
        ] {
            assert_eq!(
                FloatProjectionOperation::from_source_identity(
                    operation.source_namespace(),
                    operation.source_name(),
                ),
                Some(operation),
            );
        }
        assert_eq!(
            FloatProjectionOperation::from_source_identity("Other", "meaning32"),
            None,
        );
        assert_eq!(
            FloatProjectionOperation::from_source_identity("Float", "meaning"),
            None,
        );
        assert_ne!(
            FloatProjectionOperation::Meaning32.contract_identity(),
            FloatProjectionOperation::Meaning64.contract_identity(),
        );
        assert_eq!(
            FloatProjectionOperation::Meaning32
                .contract_identity()
                .catalog_version,
            FLOAT_PROJECTION_CATALOG_VERSION,
        );
        let binary32 = FloatProjectionOperation::Meaning32.contract_identity();
        assert_eq!(
            (binary32.format, binary32.operation, binary32.declaration),
            (32, 1, 1),
        );
        let binary64 = FloatProjectionOperation::Meaning64.contract_identity();
        assert_eq!(
            (binary64.format, binary64.operation, binary64.declaration),
            (64, 2, 2),
        );
    }

    #[test]
    fn projections_preserve_signs_but_erase_nan_payloads() {
        let projection = FloatProjectionOperation::Meaning32;
        assert_ne!(projection.project_f32(0.0), projection.project_f32(-0.0));
        assert_ne!(
            projection.project_f32(f32::INFINITY),
            projection.project_f32(f32::NEG_INFINITY)
        );
        assert_eq!(
            projection.project_f32(f32::from_bits(0x7fc0_0001)),
            projection.project_f32(f32::from_bits(0x7fff_ffff))
        );
    }

    #[test]
    fn finite_nonzero_projection_is_exact_and_format_specific() {
        let binary32 = FloatProjectionOperation::Meaning32
            .project_f32(0.1)
            .expect("binary32 projection");
        let binary64 = FloatProjectionOperation::Meaning64
            .project_f64(0.1)
            .expect("binary64 projection");
        assert!(matches!(binary32, FloatMeaning::FiniteNonZero(_)));
        assert!(matches!(binary64, FloatMeaning::FiniteNonZero(_)));
        assert_ne!(binary32, binary64, "each landed format projects exactly");
    }
}
