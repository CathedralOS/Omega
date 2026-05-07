pub mod macho;

use crate::diagnostics::Diagnostic;
use crate::native::emitter::EmittedNativeObject;
use crate::native::plan::NativePlan;
use crate::native::target::{Architecture, NativeTarget, ObjectFormat};

pub fn can_emit_target_object(target: NativeTarget) -> bool {
    matches!(
        (target.object_format, target.architecture),
        (ObjectFormat::MachO, Architecture::Aarch64)
    )
}

pub fn emit_target_object(
    native_plan: &NativePlan,
) -> Option<Result<EmittedNativeObject, Diagnostic>> {
    if native_plan.target.object_format == ObjectFormat::MachO
        && native_plan.target.architecture == Architecture::Aarch64
    {
        Some(macho::emit_macho_arm64_object(native_plan))
    } else {
        None
    }
}
