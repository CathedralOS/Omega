use omega_core::diagnostics::Diagnostic;
use omega_image::{
    EmittedImageOutput, FinalImage, FinalImageInput, emitted_direct_executable_output,
};
use omega_object::{ObjectPlan, RelocationPlan};
use omega_target::{Architecture, NativeTarget, ObjectFormat};

pub struct ExecutableImageInput<'a> {
    pub target: NativeTarget,
    pub object: &'a ObjectPlan,
    pub relocations: &'a RelocationPlan,
    pub text_bytes: &'a [u8],
    pub data_bytes: &'a [u8],
}

pub fn can_emit_executable_image(target: NativeTarget) -> bool {
    matches!(
        (target.object_format, target.architecture),
        (ObjectFormat::Elf, Architecture::Aarch64) | (ObjectFormat::MachO, Architecture::Aarch64)
    )
}

pub fn emit_executable_image(
    input: ExecutableImageInput<'_>,
) -> Option<Result<EmittedImageOutput, Diagnostic>> {
    match (input.target.object_format, input.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => Some(emit_elf_aarch64_executable(input)),
        (ObjectFormat::MachO, Architecture::Aarch64) => Some(emit_macho_aarch64_executable(input)),
        _ => None,
    }
}

fn emit_elf_aarch64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(input);
    let output = omega_image_elf::emit_elf_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn emit_macho_aarch64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(input);
    let output = omega_image_macho::emit_macho_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn build_final_image(input: ExecutableImageInput<'_>) -> FinalImage {
    omega_image::build_final_image(FinalImageInput {
        target: input.target,
        object: input.object,
        relocations: input.relocations,
        text_bytes: input.text_bytes,
        data_bytes: input.data_bytes,
    })
}
