use crate::plan::NativePlan;
use omega_core::diagnostics::Diagnostic;
use omega_image::{
    EmittedImageOutput, FinalImage, FinalImageInput, emitted_direct_executable_output,
};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

pub fn can_emit_target_output(target: NativeTarget) -> bool {
    matches!(
        (target.object_format, target.architecture),
        (ObjectFormat::Elf, Architecture::Aarch64) | (ObjectFormat::MachO, Architecture::Aarch64)
    )
}

pub fn emit_target_output(
    native_plan: &NativePlan,
) -> Option<Result<EmittedImageOutput, Diagnostic>> {
    match (
        native_plan.target.object_format,
        native_plan.target.architecture,
    ) {
        (ObjectFormat::Elf, Architecture::Aarch64) => {
            Some(emit_elf_aarch64_executable(native_plan))
        }
        (ObjectFormat::MachO, Architecture::Aarch64) => {
            Some(emit_macho_aarch64_executable(native_plan))
        }
        _ => None,
    }
}

fn emit_elf_aarch64_executable(native_plan: &NativePlan) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(native_plan);
    let output = omega_image_elf::emit_elf_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn emit_macho_aarch64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(native_plan);
    let output = omega_image_macho::emit_macho_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn build_final_image(native_plan: &NativePlan) -> FinalImage {
    omega_image::build_final_image(FinalImageInput {
        target: native_plan.target,
        object: &native_plan.object,
        relocations: &native_plan.relocations,
        text_bytes: native_plan.machine_code.bytes.storage_slice(),
        data_bytes: native_plan.data.bytes.storage_slice(),
    })
}
