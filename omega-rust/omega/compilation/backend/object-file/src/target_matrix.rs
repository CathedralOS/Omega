/*
We define the closed object target policy matrix (`object_target_policy`),
mapping each of the five supported `(architecture, object_format)` pairs to its
canonical process-entry spelling, section names, and text-section alignment.
Undeclared pairs such as `(Aarch64, Coff)` return `None` rather than matching a
format wildcard.
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
        (Architecture::Aarch64, ObjectFormat::Elf) => {
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
        (Architecture::Aarch64, ObjectFormat::Coff) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{object_target_policy, row};
    use target::{Architecture, NativeTarget, ObjectFormat};

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
    fn object_target_policy_fails_closed_on_undeclared_pairs() {
        let target = NativeTarget {
            architecture: Architecture::Aarch64,
            object_format: ObjectFormat::Coff,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        assert_eq!(object_target_policy(target), None);
    }
}
