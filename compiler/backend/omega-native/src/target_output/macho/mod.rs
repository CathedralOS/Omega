use crate::plan::NativePlan;
use crate::target_output::{build_final_image, emitted_direct_executable_output};
use omega_core::diagnostics::Diagnostic;
use omega_image::EmittedImageOutput;

pub fn emit_macho_arm64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(native_plan);
    let output = omega_image_macho::emit_macho_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}
