//! Per-instruction-set facts behind the two Mach-O ABIs.
//!
//! The emitters and validators share one file plan, command stream, bind/rebase
//! language, and signature layout; the ISA choice supplies the header CPU
//! fields, the loader/code-directory page granule, the import-thunk encoding,
//! the entry-alignment contract, and the relocation applicator. Anything not
//! listed here is genuinely format-level and stays shared. The thunk bytes
//! themselves stay beside their dyld-linking callers; `isa-x86_64` owns the
//! closed x86-64 form's decoder and claimed footprint.

use calling_conventions::StateFootprintEvidence;
use diagnostics::Diagnostic;
use image::{FinalImage, FinalImageLayout};

/// The two instruction sets the supported Mach-O writer emits for.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MachoIsa {
    Aarch64,
    X86_64,
}

impl MachoIsa {
    /// The ISA a `(format, architecture)` pair implies, or `None` off the
    /// supported Mach-O diagonals. The emitter refuses any image whose target
    /// disagrees, and validator dispatch uses the same test so a forged
    /// object/target pairing cannot substitute the other ISA's constants.
    pub(crate) fn from_format(
        object_format: target::ObjectFormat,
        architecture: target::Architecture,
    ) -> Option<Self> {
        match (object_format, architecture) {
            (target::ObjectFormat::MachO, target::Architecture::Aarch64) => Some(Self::Aarch64),
            (target::ObjectFormat::MachO, target::Architecture::X86_64) => Some(Self::X86_64),
            _ => None,
        }
    }

    pub(crate) fn native_target(self) -> target::NativeTarget {
        match self {
            Self::Aarch64 => target::NativeTarget::macos_arm64(),
            Self::X86_64 => target::NativeTarget::macos_x64(),
        }
    }

    /// The hosted-application profile whose normalized dylib locators this
    /// ISA's bind plan admits.
    pub(crate) fn target_profile(self) -> target::TargetProfile {
        match self {
            Self::Aarch64 => target::TargetProfile::MacosArm64,
            Self::X86_64 => target::TargetProfile::MacosX64,
        }
    }

    /// `cputype`: CPU_TYPE_ARM64 / CPU_TYPE_X86_64 (ABI64 flag included).
    pub(crate) fn cpu_type(self) -> u32 {
        match self {
            Self::Aarch64 => 0x0100_000c,
            Self::X86_64 => 0x0100_0007,
        }
    }

    /// `cpusubtype`: ARM64's ALL / x86-64's ALL|LIB64.
    pub(crate) fn cpu_subtype(self) -> u32 {
        match self {
            Self::Aarch64 => 0,
            Self::X86_64 => 0x8000_0003,
        }
    }

    /// The OS page granule: 16 KiB on Apple silicon, 4 KiB on Intel.
    /// Governs segment VM/file alignment and the `__bss` file-tail rule.
    pub(crate) fn page_size(self) -> u64 {
        match self {
            Self::Aarch64 => 0x4000,
            Self::X86_64 => 0x1000,
        }
    }

    /// The CodeDirectory `pageSize` field's log2.
    pub(crate) fn code_signature_page_power(self) -> u8 {
        match self {
            Self::Aarch64 => 14,
            Self::X86_64 => 12,
        }
    }

    /// Bytes per import thunk: ADRP+LDR+BR is three instructions; the x86-64
    /// stub is the closed `jmp qword ptr [rip + disp32]` form.
    pub(crate) fn import_thunk_size(self) -> usize {
        match self {
            Self::Aarch64 => 12,
            Self::X86_64 => isa_x86_64::X86_64_IMPORT_THUNK_BYTE_COUNT,
        }
    }

    /// Entry-point alignment the mapping validator enforces: fixed-width
    /// AArch64 instructions are 4-aligned; x86-64 accepts any byte.
    pub(crate) fn entry_alignment(self) -> u64 {
        match self {
            Self::Aarch64 => 4,
            Self::X86_64 => 1,
        }
    }

    pub(crate) fn format_name(self) -> &'static str {
        match self {
            Self::Aarch64 => "mach-o-arm64-executable",
            Self::X86_64 => "mach-o-x86_64-executable",
        }
    }

    /// Apply this ISA's object relocations to the image's section bytes.
    pub(crate) fn apply_relocations(
        self,
        image: &mut FinalImage,
        layout: &FinalImageLayout,
        output_name: &str,
    ) -> Result<(), Diagnostic> {
        match self {
            Self::Aarch64 => image::apply_aarch64_relocations(image, layout, output_name),
            Self::X86_64 => image::apply_x86_64_relocations(image, layout, output_name),
        }
    }

    /// The architectural state each thunk declares it writes before control
    /// leaves it: AArch64 clobbers X16 on the way to IP; the x86-64 stub's
    /// decoder owns that claim in `isa-x86_64`.
    pub(crate) fn thunk_footprint(self) -> StateFootprintEvidence {
        match self {
            Self::Aarch64 => StateFootprintEvidence::new(
                calling_conventions::RegisterSet::new([
                    calling_conventions::MachineRegister::Aarch64X(16),
                ]),
                calling_conventions::MachineStateSet::new([
                    calling_conventions::MachineState::InstructionPointer,
                ]),
            ),
            Self::X86_64 => isa_x86_64::x86_64_import_thunk_footprint(),
        }
    }
}
