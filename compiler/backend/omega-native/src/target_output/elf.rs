use crate::emitter::EmittedNativeOutput;
use crate::plan::NativePlan;
use crate::target_output::{build_final_image, emitted_direct_executable_output};
use omega_core::diagnostics::Diagnostic;

pub fn emit_elf_arm64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let image = build_final_image(native_plan);
    let output = omega_image_elf::emit_elf_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}
