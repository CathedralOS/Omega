//! The object target matrix: one policy row per supported
//! (architecture, object-format) pair.
//!
//! The object layer derives a process-entry spelling, a section spelling per
//! `SectionKind`, and the canonical text-section alignment from the target.
//! Those lookups used to re-derive their answers from `object_format` (or
//! `architecture`) alone through match arms with wildcards, so an undeclared
//! pair such as (Aarch64, COFF) or (x86-64, Mach-O) silently inherited
//! whichever format arm came first. The four admitted pairs are declared here
//! once; every derivation resolves through [`object_target_policy`], and an
//! undeclared pair answers `None` instead of a borrowed spelling.

use target::{Architecture, NativeTarget, ObjectFormat};

/// The object policy for one declared (architecture, object-format) pair.
///
/// `text_section_alignment` is the alignment
/// `validate_relocation_free_object` requires of the pair's text section,
/// not a suggestion the writer may relax.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectTargetPolicy {
    pub entry_symbol_name: &'static str,
    pub text_section_name: &'static str,
    pub data_section_name: &'static str,
    pub bss_section_name: &'static str,
    pub text_section_alignment: u64,
}

/// Resolve one declared (architecture, object-format) pair to its object
/// policy.
///
/// Declared rows are Aarch64 with ELF or Mach-O and x86-64 with ELF or COFF —
/// the same four pairs the relocation-free validator admits. (Aarch64, COFF)
/// and (x86-64, Mach-O) are undeclared and return `None`; adding a supported
/// pair extends this matrix deliberately rather than widening a wildcard.
pub fn object_target_policy(target: NativeTarget) -> Option<ObjectTargetPolicy> {
    match (target.architecture, target.object_format) {
        (Architecture::Aarch64, ObjectFormat::Elf) => Some(ObjectTargetPolicy {
            entry_symbol_name: "main",
            text_section_name: ".text",
            data_section_name: ".data",
            bss_section_name: ".bss",
            text_section_alignment: 4,
        }),
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(ObjectTargetPolicy {
            entry_symbol_name: "_main",
            text_section_name: "__TEXT,__text",
            data_section_name: "__DATA,__data",
            bss_section_name: "__DATA,__bss",
            text_section_alignment: 4,
        }),
        (Architecture::X86_64, ObjectFormat::Elf) => Some(ObjectTargetPolicy {
            entry_symbol_name: "main",
            text_section_name: ".text",
            data_section_name: ".data",
            bss_section_name: ".bss",
            text_section_alignment: 1,
        }),
        (Architecture::X86_64, ObjectFormat::Coff) => Some(ObjectTargetPolicy {
            entry_symbol_name: "main",
            text_section_name: ".text",
            data_section_name: ".data",
            bss_section_name: ".bss",
            text_section_alignment: 1,
        }),
        (Architecture::Aarch64, ObjectFormat::Coff)
        | (Architecture::X86_64, ObjectFormat::MachO) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{ObjectTargetPolicy, object_target_policy};
    use target::{Architecture, NativeTarget, ObjectFormat};

    #[test]
    fn object_target_policy_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (
                NativeTarget::linux_arm64(),
                ObjectTargetPolicy {
                    entry_symbol_name: "main",
                    text_section_name: ".text",
                    data_section_name: ".data",
                    bss_section_name: ".bss",
                    text_section_alignment: 4,
                },
            ),
            (
                NativeTarget::macos_arm64(),
                ObjectTargetPolicy {
                    entry_symbol_name: "_main",
                    text_section_name: "__TEXT,__text",
                    data_section_name: "__DATA,__data",
                    bss_section_name: "__DATA,__bss",
                    text_section_alignment: 4,
                },
            ),
            (
                NativeTarget::linux_x64(),
                ObjectTargetPolicy {
                    entry_symbol_name: "main",
                    text_section_name: ".text",
                    data_section_name: ".data",
                    bss_section_name: ".bss",
                    text_section_alignment: 1,
                },
            ),
            (
                NativeTarget::windows_x64(),
                ObjectTargetPolicy {
                    entry_symbol_name: "main",
                    text_section_name: ".text",
                    data_section_name: ".data",
                    bss_section_name: ".bss",
                    text_section_alignment: 1,
                },
            ),
        ] {
            assert_eq!(object_target_policy(target), Some(expected));
        }
        // Windows and UEFI share the (x86-64, COFF) row: their `NativeTarget`
        // contracts are indistinguishable at this layer.
        assert_eq!(
            object_target_policy(NativeTarget::uefi_x64()),
            object_target_policy(NativeTarget::windows_x64())
        );
    }

    #[test]
    fn object_target_policy_fails_closed_on_undeclared_pairs() {
        for target in [
            NativeTarget {
                architecture: Architecture::Aarch64,
                object_format: ObjectFormat::Coff,
                pointer_size: 8,
                pointer_alignment: 8,
            },
            NativeTarget {
                architecture: Architecture::X86_64,
                object_format: ObjectFormat::MachO,
                pointer_size: 8,
                pointer_alignment: 8,
            },
        ] {
            assert_eq!(object_target_policy(target), None);
        }
    }
}
