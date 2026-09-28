//! The realization request and settlement vocabulary: boundary, provider,
//! builtin, and callback-thunk settlements, the request itself, and the
//! requested or settled artifact results.

pub use native_artifact::RequestedNativeArtifact;
pub(crate) use terminal_psi_to_abstract_operations::VerifiedNativeArtifactInput as NativeRealizationInput;
pub use terminal_psi_to_abstract_operations::provider_admission::{
    NativeBoundaryRealization, NativeCompilerBuiltinSettlement, NativeProviderSettlement,
};

pub use backend_plan::NativeCallbackThunkSettlement;

/// Native admission plus the product's physical selections and image destination.
/// Admission vocabulary belongs to stage 00; the compiler forwards that request
/// unchanged instead of maintaining a second flattened copy of its fields.
pub struct NativeRealizationRequest<'request> {
    pub admission: terminal_psi_to_abstract_operations::NativeAdmissionRequest<'request>,
    pub image_request: image_emission::ExecutableImageEmissionRequest,
    pub optimization_selections: &'request optimization_core::PostTerminalOptimizationSelections,
    /// Isolated bodies paired one-to-one with the admitted callback arguments.
    /// Each body has its own Terminal artifact namespace.
    pub callback_thunks: &'request [NativeCallbackThunkSettlement<'request>],
}

/// Failed requested realization with the complete image input recoverable.
#[derive(Debug)]
#[must_use = "requested native realization failure retains the exact image request"]
pub struct RequestedNativeArtifactError {
    pub(crate) image_request: image_emission::ExecutableImageEmissionRequest,
    pub(crate) diagnostics: Vec<diagnostics::Diagnostic>,
}

impl RequestedNativeArtifactError {
    pub fn into_parts(
        self,
    ) -> (
        image_emission::ExecutableImageEmissionRequest,
        Vec<diagnostics::Diagnostic>,
    ) {
        (self.image_request, self.diagnostics)
    }

    pub fn diagnostics(&self) -> &[diagnostics::Diagnostic] {
        &self.diagnostics
    }
}
