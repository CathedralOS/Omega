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
        (ObjectFormat::Elf, Architecture::Aarch64)
            | (ObjectFormat::MachO, Architecture::Aarch64)
            | (ObjectFormat::Coff, Architecture::X86_64)
    )
}

pub fn emit_executable_image(
    input: ExecutableImageInput<'_>,
) -> Option<Result<EmittedImageOutput, Diagnostic>> {
    match (input.target.object_format, input.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => Some(emit_elf_aarch64_executable(input)),
        (ObjectFormat::MachO, Architecture::Aarch64) => Some(emit_macho_aarch64_executable(input)),
        (ObjectFormat::Coff, Architecture::X86_64) => Some(emit_pe_x86_64_executable(input)),
        _ => None,
    }
}

pub fn emit_checked_executable_image(
    input: ExecutableImageInput<'_>,
    planned_text_bytes: usize,
) -> Result<EmittedImageOutput, Diagnostic> {
    if input.text_bytes.len() != planned_text_bytes {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            input.target,
            input.text_bytes.len(),
            planned_text_bytes
        )));
    }

    if let Some(emitted_output) = emit_executable_image(input) {
        return emitted_output;
    }

    Err(Diagnostic::error(
        "cannot emit native executable; no direct image writer is registered for this target",
    ))
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

fn emit_pe_x86_64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(input);
    let output = omega_image_pe::emit_pe_x86_64_executable(image)?;
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
