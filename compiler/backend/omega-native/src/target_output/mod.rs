pub mod elf;
pub mod macho;

use crate::emitter::EmittedNativeOutput;
use crate::plan::NativePlan;
use crate::target::{Architecture, NativeTarget, ObjectFormat};
use omega_core::diagnostics::Diagnostic;

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
