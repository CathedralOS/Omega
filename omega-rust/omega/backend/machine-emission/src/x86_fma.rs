//! Source-free feature-custody seam for canonical scalar x86 FMA3.
//!
//! This module intentionally does not accept a Terminal/source operation and
//! does not select a generic Float provider. It emits one exact instruction
//! fragment while retaining the AVX+FMA3 requirement that a later admitted
//! provider must discharge.

use calling_conventions::MachineRegister;
use diagnostics::Diagnostic;
use machine_code::{X86ScalarFmaFormat, X86ScalarFmaFragment};
use target::{NativeTarget, X86FeatureRequirement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmittedX86ScalarFmaFragment {
    pub bytes: [u8; 5],
    pub custody: X86ScalarFmaFragment,
}

pub fn emit_feature_required_x86_scalar_fma(
    requirement: X86FeatureRequirement,
    target: NativeTarget,
    format: X86ScalarFmaFormat,
    destination: MachineRegister,
    addend: MachineRegister,
    multiplicand: MachineRegister,
    code_offset: usize,
) -> Result<EmittedX86ScalarFmaFragment, Diagnostic> {
    if !requirement.has_canonical_identity() {
        return Err(Diagnostic::error(
            "x86 scalar FMA requirement has a noncanonical identity",
        ));
    }
    if requirement.profile().native_target() != target {
        return Err(Diagnostic::error(
            "x86 scalar FMA requirement profile does not own the selected native target",
        ));
    }
    let bytes = match format {
        X86ScalarFmaFormat::Binary32 => {
            isa_x86_64::encode_vfmadd132ss(destination, addend, multiplicand)?
        }
        X86ScalarFmaFormat::Binary64 => {
            isa_x86_64::encode_vfmadd132sd(destination, addend, multiplicand)?
        }
    };
    let mut custody = X86ScalarFmaFragment {
        requirement,
        target,
        format,
        destination,
        addend,
        multiplicand,
        code_offset,
        byte_count: bytes.len(),
        identity: [0; 32],
    };
    custody.identity = custody
        .recomputed_identity()
        .expect("validated x86/XMM scalar FMA fragment has an identity");
    Ok(EmittedX86ScalarFmaFragment { bytes, custody })
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::TargetProfile;

    #[test]
    fn emits_both_formats_with_exact_requirement_and_interval_custody() {
        let requirement = X86FeatureRequirement::scalar_fma(TargetProfile::LinuxX64).unwrap();
        for (format, expected) in [
            (X86ScalarFmaFormat::Binary32, [0xc4, 0xe2, 0x71, 0x99, 0xc2]),
            (X86ScalarFmaFormat::Binary64, [0xc4, 0xe2, 0xf1, 0x99, 0xc2]),
        ] {
            let emitted = emit_feature_required_x86_scalar_fma(
                requirement,
                NativeTarget::linux_x64(),
                format,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
                MachineRegister::X86Xmm(2),
                11,
            )
            .unwrap();
            assert_eq!(emitted.bytes, expected);
            assert_eq!(emitted.custody.requirement, requirement);
            assert_eq!(emitted.custody.target, NativeTarget::linux_x64());
            assert_eq!(emitted.custody.code_offset, 11);
            assert_eq!(emitted.custody.byte_count, 5);
            assert_eq!(
                emitted.custody.recomputed_identity(),
                Some(emitted.custody.identity)
            );
        }
    }

    #[test]
    fn emits_extended_registers_without_inferencing_host_features() {
        let requirement = X86FeatureRequirement::scalar_fma(TargetProfile::WindowsX64).unwrap();
        let emitted = emit_feature_required_x86_scalar_fma(
            requirement,
            NativeTarget::windows_x64(),
            X86ScalarFmaFormat::Binary64,
            MachineRegister::X86Xmm(9),
            MachineRegister::X86Xmm(10),
            MachineRegister::X86Xmm(15),
            0,
        )
        .unwrap();
        assert_eq!(emitted.bytes, [0xc4, 0x42, 0xa9, 0x99, 0xcf]);
    }

    #[test]
    fn rejects_cross_profile_target_and_invalid_registers() {
        let linux = X86FeatureRequirement::scalar_fma(TargetProfile::LinuxX64).unwrap();
        assert!(
            emit_feature_required_x86_scalar_fma(
                linux,
                NativeTarget::windows_x64(),
                X86ScalarFmaFormat::Binary32,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
                MachineRegister::X86Xmm(2),
                0,
            )
            .is_err()
        );
        assert!(
            emit_feature_required_x86_scalar_fma(
                linux,
                NativeTarget::linux_x64(),
                X86ScalarFmaFormat::Binary32,
                MachineRegister::X86Rax,
                MachineRegister::X86Xmm(1),
                MachineRegister::X86Xmm(2),
                0,
            )
            .is_err()
        );
    }
}
