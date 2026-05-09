use crate::plan::NativePlan;
use crate::target_output::emit_target_output;
use omega_core::diagnostics::Diagnostic;

pub use omega_image::{
    EmittedImageOutput as EmittedNativeOutput, ImageOutputKind as NativeOutputKind,
};

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

    Err(Diagnostic::error(format!(
        "cannot emit native executable for {:?}; no direct image writer is registered for this target",
        native_plan.target
    )))
}
