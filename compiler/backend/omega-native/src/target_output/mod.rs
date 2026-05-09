pub mod elf;
pub mod macho;

use crate::emitter::EmittedNativeOutput;
use crate::emitter::NativeOutputKind;
use crate::plan::NativePlan;
use omega_core::diagnostics::Diagnostic;
use omega_image::ExecutableImageOutput;
use omega_target::{Architecture, NativeTarget, ObjectFormat};

pub fn can_emit_target_output(target: NativeTarget) -> bool {
    matches!(
        (target.object_format, target.architecture),
        (ObjectFormat::Elf, Architecture::Aarch64) | (ObjectFormat::MachO, Architecture::Aarch64)
    )
}

pub fn emit_target_output(
    native_plan: &NativePlan,
) -> Option<Result<EmittedNativeOutput, Diagnostic>> {
    match (
        native_plan.target.object_format,
        native_plan.target.architecture,
    ) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            Some(elf::emit_elf_arm64_executable(native_plan))
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            Some(macho::emit_macho_arm64_executable(native_plan))
        }
        _ => None,
    }
}

fn emitted_direct_executable_output(output: ExecutableImageOutput) -> EmittedNativeOutput {
    EmittedNativeOutput {
        bytes: output.bytes,
        file_name: output.file_name,
        format: output.format,
        kind: NativeOutputKind::DirectExecutable,
        text_bytes: output.text_bytes,
        data_bytes: output.data_bytes,
        bss_bytes: output.bss_bytes,
        symbols: output.symbols,
        relocations: output.relocations,
        final_image_symbols: output.symbols,
        final_image_imports: output.imports,
        final_image_relocations: output.relocations,
    }
}
