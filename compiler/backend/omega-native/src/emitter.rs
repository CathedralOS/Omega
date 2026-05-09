use crate::plan::NativePlan;
use crate::target_output::emit_target_output;
use omega_core::diagnostics::Diagnostic;
use omega_object::{ObjectContainerInput, ObjectContainerOutput, emit_omega_object_container};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmittedNativeOutput {
    pub bytes: Vec<u8>,
    pub file_name: String,
    pub format: String,
    pub kind: NativeOutputKind,
    pub text_bytes: usize,
    pub data_bytes: usize,
    pub bss_bytes: usize,
    pub symbols: usize,
    pub relocations: usize,
    pub final_image_symbols: usize,
    pub final_image_imports: usize,
    pub final_image_relocations: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeOutputKind {
    DirectExecutable,
    LinkableObject,
    NativeContainer,
}

pub fn emit_native_output(native_plan: &NativePlan) -> Result<EmittedNativeOutput, Diagnostic> {
    if native_plan.machine_code.bytes.len() != native_plan.machine_code.byte_count {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            native_plan.target,
            native_plan.machine_code.bytes.len(),
            native_plan.machine_code.byte_count
        )));
    }

    if let Some(emitted_output) = emit_target_output(native_plan) {
        return emitted_output;
    }

    emit_omega_native_container(native_plan)
}

fn emit_omega_native_container(
    native_plan: &NativePlan,
) -> Result<EmittedNativeOutput, Diagnostic> {
    let output = emit_omega_object_container(ObjectContainerInput {
        target: native_plan.target,
        object: &native_plan.object,
        relocations: &native_plan.relocations,
        text_bytes: native_plan.machine_code.bytes.storage_slice(),
        data_bytes: native_plan.data.bytes.storage_slice(),
    });

    Ok(emitted_native_container_output(output))
}

fn emitted_native_container_output(output: ObjectContainerOutput) -> EmittedNativeOutput {
    EmittedNativeOutput {
        bytes: output.bytes,
        file_name: output.file_name,
        format: output.format,
        kind: NativeOutputKind::NativeContainer,
        text_bytes: output.text_bytes,
        data_bytes: output.data_bytes,
        bss_bytes: output.bss_bytes,
        symbols: output.symbols,
        relocations: output.relocations,
        final_image_symbols: 0,
        final_image_imports: 0,
        final_image_relocations: 0,
    }
}
