//! Closed proof projections from landed runtime floats to `FloatMeaning`.
//!
//! These rows are deliberately separate from integer embedding: floats never
//! project into proof `Int` or a bare `Rat`, and NaN payload identity is not
//! retained by the public proof carrier.

use crate::float_semantics::{FloatFormat, FloatMeaning};

/// Toolchain source that owns the closed public float-projection declarations.
/// A declaration with the same path and signature in any other source has no
/// projection semantics.
pub const FLOAT_PROJECTION_CORE_SOURCE: &str = "float_operations.omg";
/// Toolchain source that owns the proof-only projection result carrier.
pub const FLOAT_MEANING_CORE_SOURCE: &str = "float_meaning.omg";

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
