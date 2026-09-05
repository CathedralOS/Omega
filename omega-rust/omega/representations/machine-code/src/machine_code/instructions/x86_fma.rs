use calling_conventions::MachineRegister;
use semantic_vocabulary::{IeeeFloatValue, OperationId, ValueId};
use sha2::{Digest, Sha256};
use target::{Architecture, NativeTarget, X86FeatureRequirement};

const X86_SCALAR_FMA_FRAGMENT_SCHEMA: &[u8] = b"omega.x86-scalar-fma-fragment.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum X86ScalarFmaFormat {
    Binary32,
    Binary64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86ScalarFmaOperandRecord {
    pub defining_operation: OperationId,
    pub source_value: ValueId,
    pub value: IeeeFloatValue,
    pub register: MachineRegister,
    pub code_offset: usize,
    pub byte_count: usize,
}

/// Exact semantic and selected-plan custody joined to one emitted mechanics
/// fragment by identity. The complete selected plan remains in the native
/// realization request/artifact; its SHA-256 digest is the non-substitutable
/// join key, while the compact report identity remains diagnostic only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86ScalarFmaOccurrenceRecord {
    pub terminal_operation: OperationId,
    pub result: ValueId,
    pub format: X86ScalarFmaFormat,
    pub left: X86ScalarFmaOperandRecord,
    pub right: X86ScalarFmaOperandRecord,
    pub addend: X86ScalarFmaOperandRecord,
    pub destination: MachineRegister,
    pub provider_plan_report_identity: u64,
    pub provider_plan_digest: [u8; 32],
    pub slot: target::X86ScalarFmaSlot,
    pub admitted_provider: target::AdmittedX86ScalarFmaProvider,
    pub fragment_identity: [u8; 32],
    pub operation_ordinal: usize,
}

/// Function-level proof that ambient x86 floating controls were saved,
/// Omega's canonical nearest-even/gradual/masked MXCSR was installed, and the
/// complete incoming control state was restored before return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86FloatingControlRecord {
    pub target: NativeTarget,
    pub canonical_mxcsr: u32,
    pub canonical_slot_byte_offset: u32,
    pub saved_slot_byte_offset: u32,
    pub save_offset: usize,
    pub save_byte_count: usize,
    pub canonical_store_offset: usize,
    pub canonical_store_byte_count: usize,
    pub install_offset: usize,
    pub install_byte_count: usize,
    pub restore_offset: usize,
    pub restore_byte_count: usize,
}

/// Per-call proof that one returning x86 foreign boundary preserved the
/// caller's complete MXCSR. The slot may be reused by sequential calls, while
/// each call retains distinct save/restore instruction intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86ForeignCallFloatingControlRecord {
    pub target: NativeTarget,
    pub saved_slot_byte_offset: u32,
    pub save_offset: usize,
    pub save_byte_count: usize,
    pub restore_offset: usize,
    pub restore_byte_count: usize,
}

/// Exact machine-code custody for one feature-requiring scalar FMA3 interval.
///
/// This record does not admit its feature requirement and does not select a
/// source provider. Object construction independently replays the bytes and
/// all fields before retaining it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct X86ScalarFmaFragment {
    pub requirement: X86FeatureRequirement,
    pub target: NativeTarget,
    pub format: X86ScalarFmaFormat,
    pub destination: MachineRegister,
    pub addend: MachineRegister,
    pub multiplicand: MachineRegister,
    pub code_offset: usize,
    pub byte_count: usize,
    pub identity: [u8; 32],
}

impl X86ScalarFmaFragment {
    pub fn recomputed_identity(&self) -> Option<[u8; 32]> {
        x86_scalar_fma_fragment_identity(self)
    }
}

pub fn x86_scalar_fma_fragment_identity(fragment: &X86ScalarFmaFragment) -> Option<[u8; 32]> {
    if fragment.target.architecture != Architecture::X86_64 {
        return None;
    }
    let destination = xmm_index(fragment.destination)?;
    let addend = xmm_index(fragment.addend)?;
    let multiplicand = xmm_index(fragment.multiplicand)?;
    let mut hasher = Sha256::new();
    hasher.update(X86_SCALAR_FMA_FRAGMENT_SCHEMA);
    hasher.update(fragment.requirement.identity());
    hasher.update(
        fragment
            .requirement
            .profile()
            .identity()
            .as_str()
            .as_bytes(),
    );
    hasher.update([match fragment.target.architecture {
        Architecture::Aarch64 => 0,
        Architecture::X86_64 => 1,
    }]);
    hasher.update([match fragment.target.object_format {
        target::ObjectFormat::Elf => 0,
        target::ObjectFormat::MachO => 1,
        target::ObjectFormat::Coff => 2,
    }]);
    hasher.update((fragment.target.pointer_size as u64).to_le_bytes());
    hasher.update((fragment.target.pointer_alignment as u64).to_le_bytes());
    hasher.update([match fragment.format {
        X86ScalarFmaFormat::Binary32 => 1,
        X86ScalarFmaFormat::Binary64 => 2,
    }]);
    hasher.update([destination, addend, multiplicand]);
    hasher.update((fragment.code_offset as u64).to_le_bytes());
    hasher.update((fragment.byte_count as u64).to_le_bytes());
    Some(hasher.finalize().into())
}

fn xmm_index(register: MachineRegister) -> Option<u8> {
    match register {
        MachineRegister::X86Xmm(index @ 0..=15) => Some(index),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::TargetProfile;

    fn fragment() -> X86ScalarFmaFragment {
        let requirement = X86FeatureRequirement::scalar_fma(TargetProfile::LinuxX64).unwrap();
        let mut fragment = X86ScalarFmaFragment {
            requirement,
            target: NativeTarget::linux_x64(),
            format: X86ScalarFmaFormat::Binary32,
            destination: MachineRegister::X86Xmm(0),
            addend: MachineRegister::X86Xmm(1),
            multiplicand: MachineRegister::X86Xmm(2),
            code_offset: 7,
            byte_count: 5,
            identity: [0; 32],
        };
        fragment.identity = fragment.recomputed_identity().unwrap();
        fragment
    }

    #[test]
    fn identity_binds_target_format_registers_interval_and_requirement() {
        let baseline = fragment();
        assert_eq!(baseline.recomputed_identity(), Some(baseline.identity));
        let mut candidates = Vec::new();
        let mut changed = baseline;
        changed.requirement = X86FeatureRequirement::scalar_fma(TargetProfile::WindowsX64).unwrap();
        candidates.push(changed);
        let mut changed = baseline;
        changed.target = NativeTarget::windows_x64();
        candidates.push(changed);
        let mut changed = baseline;
        changed.format = X86ScalarFmaFormat::Binary64;
        candidates.push(changed);
        let mut changed = baseline;
        changed.destination = MachineRegister::X86Xmm(9);
        candidates.push(changed);
        let mut changed = baseline;
        changed.addend = MachineRegister::X86Xmm(10);
        candidates.push(changed);
        let mut changed = baseline;
        changed.multiplicand = MachineRegister::X86Xmm(15);
        candidates.push(changed);
        let mut changed = baseline;
        changed.code_offset += 1;
        candidates.push(changed);
        let mut changed = baseline;
        changed.byte_count += 1;
        candidates.push(changed);
        for candidate in candidates {
            assert_ne!(candidate.recomputed_identity(), Some(baseline.identity));
        }
    }

    #[test]
    fn identity_refuses_non_x86_or_non_xmm_fragments() {
        let mut invalid = fragment();
        invalid.target = NativeTarget::linux_arm64();
        assert_eq!(invalid.recomputed_identity(), None);
        invalid = fragment();
        invalid.destination = MachineRegister::X86Rax;
        assert_eq!(invalid.recomputed_identity(), None);
    }
}
