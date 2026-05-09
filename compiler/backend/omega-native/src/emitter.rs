use crate::plan::NativePlan;
use omega_core::diagnostics::Diagnostic;
use omega_image::EmittedImageOutput;
use omega_image_emission::{ExecutableImageInput, emit_executable_image};

pub fn emit_native_output(native_plan: &NativePlan) -> Result<EmittedImageOutput, Diagnostic> {
    if native_plan.machine_code.bytes.len() != native_plan.machine_code.byte_count {
        return Err(Diagnostic::error(format!(
            "cannot emit native output for {:?}: encoded {} machine byte(s), planned {} byte(s)",
            native_plan.target,
            native_plan.machine_code.bytes.len(),
            native_plan.machine_code.byte_count
        )));
    }

    if let Some(emitted_output) = emit_executable_image(ExecutableImageInput {
        target: native_plan.target,
        object: &native_plan.object,
        relocations: &native_plan.relocations,
        text_bytes: native_plan.machine_code.bytes.storage_slice(),
        data_bytes: native_plan.data.bytes.storage_slice(),
    }) {
        return emitted_output;
    }

    Err(Diagnostic::error(format!(
        "cannot emit native executable for {:?}; no direct image writer is registered for this target",
        native_plan.target
    )))
}
