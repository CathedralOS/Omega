/*
Each hosted target declares how it realizes every canonical `f32`/`f64`
operation, so float arithmetic no longer depends on a Build program choosing a
provider for 104 operator slots on every compile. The table is the target's
declaration under decision `target-realized-float-arithmetic`
(wiki/spec/language/numeric_values.md#target-realization-and-control-state).

An entry selects a catalog intrinsic or checked machine under its ISA-feature
and floating-control obligations, or names an unsupported target constraint.
Checked entries retain their actual callable and costs through ordinary
provider plans. Contracts never vary: the
operation keeps its `FloatSemantics` contract on every target, and a target
that cannot meet it declares the operation unsupported rather than
substituting another algorithm.

The table classifies only canonical operation names. The provider chooser
consults it for candidates whose provider is the authenticated sealed core
catalog map; a lookalike name in another package is an ordinary candidate.
*/

use crate::TargetProfile;

/// The ISA admission a catalog realization requires before native emission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatIsaAdmission {
    /// Part of the target's baseline floating-point unit.
    Baseline,
    /// x86-64 scalar FMA: the deployment must admit AVX and FMA3.
    X86AvxFma3,
}

impl FloatIsaAdmission {
    pub const fn report_name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::X86AvxFma3 => "avx+fma3",
        }
    }
}

/// The floating-control state a realization relies on or must restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FloatControlObligation {
    /// Runs under the canonical controls: nearest-even, gradual underflow,
    /// masked exceptions.
    CanonicalControls,
    /// Changes the rounding mode internally and restores canonical controls
    /// before the operation completes.
    DirectedRoundingRestored,
}

impl FloatControlObligation {
    pub const fn report_name(self) -> &'static str {
        match self {
            Self::CanonicalControls => "canonical-controls",
            Self::DirectedRoundingRestored => "directed-rounding-restored",
        }
    }
}

/// One target's declared realization of one canonical float operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalFloatRealization {
    CatalogIntrinsic {
        isa: FloatIsaAdmission,
        controls: FloatControlObligation,
    },
    /// The authenticated catalog member is an ordinary checked Omega body.
    /// Its exact callable and costs remain in the selected adapter plan.
    CheckedMachine {
        isa: FloatIsaAdmission,
        controls: FloatControlObligation,
    },
    Unsupported {
        constraint: &'static str,
    },
}

/// The canonical operation names, as the core catalog spells their public
/// requirements. Spelled operators (`Float::add`) are families whose `f32`
/// and `f64` coordinates share one entry.
pub const CANONICAL_FLOAT_OPERATIONS: [&str; 104] = [
    "Float::add",
    "Float::subtract",
    "Float::multiply",
    "Float::divide",
    "Float::equal",
    "Float::not_equal",
    "Float::less",
    "Float::less_or_equal",
    "Float::greater",
    "Float::greater_or_equal",
    "F32::negate",
    "F64::negate",
    "F32::multiply_then_add",
    "F64::multiply_then_add",
    "F32::fused_multiply_add",
    "F64::fused_multiply_add",
    "F32::square_root",
    "F64::square_root",
    "F32::minimum",
    "F64::minimum",
    "F32::maximum",
    "F64::maximum",
    "F32::from_f64",
    "F64::from_f32",
    "F32::from_i8",
    "F32::from_i16",
    "F32::from_i32",
    "F32::from_i64",
    "F32::from_u8",
    "F32::from_u16",
    "F32::from_u32",
    "F32::from_u64",
    "F64::from_i8",
    "F64::from_i16",
    "F64::from_i32",
    "F64::from_i64",
    "F64::from_u8",
    "F64::from_u16",
    "F64::from_u32",
    "F64::from_u64",
    "I8::from_f32",
    "I16::from_f32",
    "I32::from_f32",
    "I64::from_f32",
    "U8::from_f32",
    "U16::from_f32",
    "U32::from_f32",
    "U64::from_f32",
    "I8::from_f64",
    "I16::from_f64",
    "I32::from_f64",
    "I64::from_f64",
    "U8::from_f64",
    "U16::from_f64",
    "U32::from_f64",
    "U64::from_f64",
    "F32::classify",
    "F64::classify",
    "F32::is_nan",
    "F64::is_nan",
    "F32::is_infinite",
    "F64::is_infinite",
    "F32::is_finite",
    "F64::is_finite",
    "F32::is_normal",
    "F64::is_normal",
    "F32::is_subnormal",
    "F64::is_subnormal",
    "F32::add_toward_zero",
    "F64::add_toward_zero",
    "F32::add_toward_positive",
    "F64::add_toward_positive",
    "F32::add_toward_negative",
    "F64::add_toward_negative",
    "F32::subtract_toward_zero",
    "F64::subtract_toward_zero",
    "F32::subtract_toward_positive",
    "F64::subtract_toward_positive",
    "F32::subtract_toward_negative",
    "F64::subtract_toward_negative",
    "F32::multiply_toward_zero",
    "F64::multiply_toward_zero",
    "F32::multiply_toward_positive",
    "F64::multiply_toward_positive",
    "F32::multiply_toward_negative",
    "F64::multiply_toward_negative",
    "F32::divide_toward_zero",
    "F64::divide_toward_zero",
    "F32::divide_toward_positive",
    "F64::divide_toward_positive",
    "F32::divide_toward_negative",
    "F64::divide_toward_negative",
    "F32::square_root_toward_zero",
    "F64::square_root_toward_zero",
    "F32::square_root_toward_positive",
    "F64::square_root_toward_positive",
    "F32::square_root_toward_negative",
    "F64::square_root_toward_negative",
    "F32::fused_multiply_add_toward_zero",
    "F64::fused_multiply_add_toward_zero",
    "F32::fused_multiply_add_toward_positive",
    "F64::fused_multiply_add_toward_positive",
    "F32::fused_multiply_add_toward_negative",
    "F64::fused_multiply_add_toward_negative",
];

/// Finds the canonical `Owner::operation` spelling inside a requirement
/// identity. Identities arrive as `operator::F32::negate(named(name(f32)))..`
/// for named operations and as escaped `named-callable(path(nominal\\(..\\,
/// path\\(Float::less\\)..` for spelled operator families, so the name is
/// matched as a whole path token: the longest canonical name whose
/// neighbouring characters are not identifier characters.
pub fn canonical_float_operation_name(requirement: &str) -> Option<&'static str> {
    let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
    CANONICAL_FLOAT_OPERATIONS
        .iter()
        .copied()
        .filter(|canonical| {
            requirement.match_indices(canonical).any(|(start, _)| {
                let before = requirement[..start].chars().next_back();
                let after = requirement[start + canonical.len()..].chars().next();
                before.is_none_or(|c| !identifier(c)) && after.is_none_or(|c| !identifier(c))
            })
        })
        .max_by_key(|canonical| canonical.len())
}

impl TargetProfile {
    /// The realization this target declares for a canonical float operation,
    /// or `None` when `requirement` names no canonical operation.
    pub fn canonical_float_realization(
        self,
        requirement: &str,
    ) -> Option<CanonicalFloatRealization> {
        let operation = canonical_float_operation_name(requirement)?;
        let (_, name) = operation.split_once("::")?;
        let directed = name.contains("_toward_");
        let fused = name.starts_with("fused_multiply_add");
        let controls = if directed {
            FloatControlObligation::DirectedRoundingRestored
        } else {
            FloatControlObligation::CanonicalControls
        };
        let x86 = matches!(self, Self::LinuxX64 | Self::MacosX64 | Self::WindowsX64);
        let arm = matches!(self, Self::LinuxArm64 | Self::MacosArm64);
        Some(if !(x86 || arm) {
            CanonicalFloatRealization::Unsupported {
                constraint: "the target declares no floating-point realization table",
            }
        } else if x86 && fused && directed {
            CanonicalFloatRealization::Unsupported {
                constraint: "x86-64 has no directed-rounding fused multiply-add realization",
            }
        } else if fused && directed {
            // Core's former Build program selected these for AArch64, but the
            // instruction catalog never realized them natively; declaring them
            // unsupported rejects at checking instead of at native emission.
            CanonicalFloatRealization::Unsupported {
                constraint: "the AArch64 instruction catalog has no directed-rounding fused multiply-add realization",
            }
        } else if name == "multiply_then_add" {
            CanonicalFloatRealization::CheckedMachine {
                isa: FloatIsaAdmission::Baseline,
                controls,
            }
        } else {
            CanonicalFloatRealization::CatalogIntrinsic {
                isa: if x86 && fused {
                    FloatIsaAdmission::X86AvxFma3
                } else {
                    FloatIsaAdmission::Baseline
                },
                controls,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hosted_target_classifies_every_canonical_operation() {
        for profile in [
            TargetProfile::LinuxArm64,
            TargetProfile::LinuxX64,
            TargetProfile::MacosArm64,
            TargetProfile::MacosX64,
            TargetProfile::WindowsX64,
        ] {
            for operation in CANONICAL_FLOAT_OPERATIONS {
                assert!(profile.canonical_float_realization(operation).is_some());
            }
        }
    }

    #[test]
    fn directed_fused_multiply_add_has_no_native_realization() {
        let directed = "F64::fused_multiply_add_toward_zero(named(name(f64)))";
        for profile in [TargetProfile::LinuxX64, TargetProfile::LinuxArm64] {
            assert!(matches!(
                profile.canonical_float_realization(directed),
                Some(CanonicalFloatRealization::Unsupported { .. })
            ));
        }
        assert!(matches!(
            TargetProfile::LinuxArm64.canonical_float_realization("F64::divide_toward_zero"),
            Some(CanonicalFloatRealization::CatalogIntrinsic {
                controls: FloatControlObligation::DirectedRoundingRestored,
                ..
            })
        ));
    }

    #[test]
    fn x86_fused_multiply_add_names_its_isa_admission() {
        assert_eq!(
            TargetProfile::WindowsX64.canonical_float_realization("F32::fused_multiply_add"),
            Some(CanonicalFloatRealization::CatalogIntrinsic {
                isa: FloatIsaAdmission::X86AvxFma3,
                controls: FloatControlObligation::CanonicalControls,
            })
        );
        assert_eq!(
            TargetProfile::LinuxX64.canonical_float_realization("FloatNativeOperations"),
            None
        );
        assert_eq!(
            canonical_float_operation_name(
                "named-callable(path(nominal\\(toolchain-owner\\,path\\(Float::less\\)\\)),parameters())"
            ),
            Some("Float::less")
        );
        assert_eq!(
            canonical_float_operation_name("operator::F32::negate_twice(named(name(f32)))"),
            None
        );
        assert_eq!(
            TargetProfile::LinuxX64.canonical_float_realization("toolchain::Float::add"),
            Some(CanonicalFloatRealization::CatalogIntrinsic {
                isa: FloatIsaAdmission::Baseline,
                controls: FloatControlObligation::CanonicalControls,
            })
        );
    }
}
