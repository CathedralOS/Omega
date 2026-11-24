/*
We define the closed object target policy matrix (`object_target_policy`),
mapping each `(architecture, object_format)` pair to its canonical
process-entry spelling, section names, and text-section alignment. Every pair
has a row; `(Aarch64, Coff)` serves `windows_arm64`. The match is exhaustive,
so a new architecture or object format must declare its own row.
*/

use target::{Architecture, NativeTarget, ObjectFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectTargetPolicy {
    pub entry_symbol_name: &'static str,
    pub text_section_name: &'static str,
    pub data_section_name: &'static str,
    pub bss_section_name: &'static str,
    pub text_section_alignment: u64,
}

const fn row(
    entry_symbol_name: &'static str,
    text_section_name: &'static str,
    data_section_name: &'static str,
    bss_section_name: &'static str,
    text_section_alignment: u64,
) -> ObjectTargetPolicy {
    ObjectTargetPolicy {
        entry_symbol_name,
        text_section_name,
        data_section_name,
        bss_section_name,
        text_section_alignment,
    }
}

pub(crate) fn object_target_policy(target: NativeTarget) -> Option<ObjectTargetPolicy> {
    match (target.architecture, target.object_format) {
        (Architecture::Aarch64, ObjectFormat::Elf | ObjectFormat::Coff) => {
            Some(row("main", ".text", ".data", ".bss", 4))
        }
        (Architecture::Aarch64, ObjectFormat::MachO) => Some(row(
            "_main",
            "__TEXT,__text",
            "__DATA,__data",
            "__DATA,__bss",
            4,
        )),
        (Architecture::X86_64, ObjectFormat::Elf | ObjectFormat::Coff) => {
            Some(row("main", ".text", ".data", ".bss", 1))
        }
        (Architecture::X86_64, ObjectFormat::MachO) => Some(row(
            "_main",
            "__TEXT,__text",
            "__DATA,__data",
            "__DATA,__bss",
            1,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{object_target_policy, row};
    use target::NativeTarget;

    #[test]
    fn object_target_policy_declares_one_row_per_supported_pair() {
        for (target, expected) in [
            (
                NativeTarget::linux_arm64(),
                row("main", ".text", ".data", ".bss", 4),
            ),
            (
                NativeTarget::macos_arm64(),
                row("_main", "__TEXT,__text", "__DATA,__data", "__DATA,__bss", 4),
            ),
            (
                NativeTarget::linux_x64(),
                row("main", ".text", ".data", ".bss", 1),
            ),
            (
                NativeTarget::macos_x64(),
                row("_main", "__TEXT,__text", "__DATA,__data", "__DATA,__bss", 1),
            ),
            (
                NativeTarget::windows_x64(),
                row("main", ".text", ".data", ".bss", 1),
            ),
        ] {
            assert_eq!(object_target_policy(target), Some(expected));
        }
        assert_eq!(
            object_target_policy(NativeTarget::uefi_x64()),
            object_target_policy(NativeTarget::windows_x64())
        );
    }

    #[test]
    fn windows_arm64_objects_use_the_coff_names_and_aarch64_alignment() {
        assert_eq!(
            object_target_policy(NativeTarget::windows_arm64()),
            Some(row("main", ".text", ".data", ".bss", 4))
        );
    }
}
