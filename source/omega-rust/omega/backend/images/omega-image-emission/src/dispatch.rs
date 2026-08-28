use crate::input::ExecutableImageInput;
use omega_image::{
    EmittedImageOutput, FinalImage, FinalImageInput, emitted_direct_executable_output,
};
use omega_target::{Architecture, ObjectFormat};
use psi_diagnostics::Diagnostic;

pub fn emit_executable_image(
    input: ExecutableImageInput<'_>,
) -> Option<Result<EmittedImageOutput, Diagnostic>> {
    match (input.target.object_format, input.target.architecture) {
        (ObjectFormat::Elf, Architecture::Aarch64) => Some(emit_elf_aarch64_executable(input)),
        (ObjectFormat::Elf, Architecture::X86_64) => Some(emit_elf_x86_64_executable(input)),
        (ObjectFormat::MachO, Architecture::Aarch64) => Some(emit_macho_aarch64_executable(input)),
        (ObjectFormat::Coff, Architecture::X86_64) => Some(emit_pe_x86_64_executable(input)),
        _ => None,
    }
}

fn emit_elf_aarch64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(input)?;
    let output = omega_image_elf::emit_elf_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn emit_elf_x86_64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(input)?;
    let output = omega_image_elf::emit_elf_x86_64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn emit_macho_aarch64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let image = build_final_image(input)?;
    let output = omega_image_macho::emit_macho_aarch64_executable(image)?;
    Ok(emitted_direct_executable_output(output))
}

fn emit_pe_x86_64_executable(
    input: ExecutableImageInput<'_>,
) -> Result<EmittedImageOutput, Diagnostic> {
    let subsystem = input.subsystem;
    let image = build_final_image(input)?;
    let output = omega_image_pe::emit_pe_x86_64_executable(image, subsystem)?;
    Ok(emitted_direct_executable_output(output))
}

fn build_final_image(input: ExecutableImageInput<'_>) -> Result<FinalImage, Diagnostic> {
    let image = omega_image::build_final_image(FinalImageInput {
        target: input.target,
        object: input.object,
        relocations: input.relocations,
        text_bytes: input.text_bytes,
        data_bytes: input.data_bytes,
    });
    omega_image::validate_final_image_function_linkage(&image, input.object)?;
    Ok(image)
}
