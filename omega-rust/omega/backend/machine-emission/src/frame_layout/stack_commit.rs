//! The frame allocator's stack-commit matrix: one row per supported
//! (architecture, object-format) pair.
//!
//! Lazily backed stacks grow one guard-page granule per touch, so a frame is
//! committed one granule at a time and can never skip past an uncommitted
//! page. The granule is the allocator's target-owned constant: every x86-64
//! host commits in 4 KiB granules, Linux AArch64 commits in 4 KiB granules,
//! and Darwin AArch64 pages are 16 KiB. Probing at a finer granule than the
//! target's remains correct, so the AAPCS64 immediate bound that rejects
//! every frame above 4095 bytes does not make the Linux AArch64 granule a
//! silent encoding assumption.
//!
//! Layout computation and independent replay both resolve the granule here.
//! An undeclared pair answers `None` rather than silently inheriting another
//! pair's granule, so a new supported target extends this matrix deliberately
//! instead of widening a wildcard arm.

use target::{Architecture, NativeTarget, ObjectFormat};

/// Resolve one declared (architecture, object-format) pair to the stack-commit
/// granule its frame allocator guarantees, in bytes.
///
/// Declared rows are x86-64 with ELF or COFF and AArch64 with ELF at 4 KiB,
/// plus (Aarch64, Mach-O) at 16 KiB — the same four pairs the ABI preservation
/// matrix admits. (Aarch64, COFF) and (x86-64, Mach-O) are undeclared and
/// answer `None`.
pub(super) const fn stack_commit_granule_bytes(target: NativeTarget) -> Option<u64> {
    match (target.architecture, target.object_format) {
        (Architecture::X86_64, ObjectFormat::Elf)
        | (Architecture::X86_64, ObjectFormat::Coff)
        | (Architecture::Aarch64, ObjectFormat::Elf) => Some(4_096),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(16_384),
        (Architecture::X86_64, ObjectFormat::MachO)
        | (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::stack_commit_granule_bytes;
    use target::{NativeTarget, ObjectFormat};

    #[test]
    fn stack_commit_matrix_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (NativeTarget::linux_x64(), 4_096),
            (NativeTarget::windows_x64(), 4_096),
            (NativeTarget::uefi_x64(), 4_096),
            (NativeTarget::linux_arm64(), 4_096),
            (NativeTarget::macos_arm64(), 16_384),
        ] {
            assert_eq!(
                stack_commit_granule_bytes(target),
                Some(expected),
                "{target:?}"
            );
        }
    }

    #[test]
    fn stack_commit_matrix_fails_closed_on_undeclared_pairs() {
        for target in [
            NativeTarget {
                object_format: ObjectFormat::MachO,
                ..NativeTarget::linux_x64()
            },
            NativeTarget {
                object_format: ObjectFormat::Coff,
                ..NativeTarget::linux_arm64()
            },
        ] {
            assert_eq!(stack_commit_granule_bytes(target), None, "{target:?}");
        }
    }
}
