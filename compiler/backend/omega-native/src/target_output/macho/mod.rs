use crate::emitter::{EmittedNativeOutput, NativeOutputKind};
use crate::final_image::build_final_image;
use crate::plan::NativePlan;
use omega_core::diagnostics::Diagnostic;
use omega_image::ExecutableImageOutput;

pub fn emit_macho_arm64_executable(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let image = build_final_image(native_plan);
    let output = omega_image_macho::emit_macho_aarch64_executable(image)?;
    Ok(emitted_native_output(output))
}

fn emitted_native_output(output: ExecutableImageOutput) -> EmittedNativeOutput {
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
