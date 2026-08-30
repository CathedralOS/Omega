use super::{NativeRealizationRequest, realize_native_artifact};
use omega_native_artifact::NativeArtifact;
use psi_diagnostics::Diagnostic;

/// A native artifact coupled to caller-owned callback-use custody.
///
/// Native realization does not interpret the opaque sidecar. This carrier
/// only proves that the exact value supplied by the caller crossed the
/// realization boundary beside the resulting source-free artifact. It grants
/// no thunk, relocation, registration, invocation, address, or lifetime
/// authority.
#[derive(Debug)]
#[must_use = "native realization must preserve callback-use custody"]
pub struct RealizedNativeArtifactWithCallbackCustody<C> {
    artifact: NativeArtifact,
    callback_custody: C,
}

impl<C> RealizedNativeArtifactWithCallbackCustody<C> {
    pub const fn artifact(&self) -> &NativeArtifact {
        &self.artifact
    }

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
    }

    pub fn into_parts(self) -> (NativeArtifact, C) {
        (self.artifact, self.callback_custody)
    }
}

/// Diagnostic rejection from callback-custody-aware native realization.
///
/// The existing native realization consumes its canonical Terminal artifact.
/// This adapter separately returns the only additional owned input, the
/// opaque callback sidecar, exactly for diagnosis or a later owner.
#[derive(Debug)]
#[must_use = "native realization rejection returns callback-use custody"]
pub struct CallbackCustodyNativeRealizationError<C> {
    diagnostics: Vec<Diagnostic>,
    callback_custody: C,
}

impl<C> CallbackCustodyNativeRealizationError<C> {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub const fn callback_custody(&self) -> &C {
        &self.callback_custody
    }

    pub fn into_parts(self) -> (Vec<Diagnostic>, C) {
        (self.diagnostics, self.callback_custody)
    }
}

/// Realize a canonical source-free artifact without losing the caller's
/// opaque callback-use sidecar.
///
/// Success and rejection both return the sidecar by value in its original
/// order. This adapter deliberately does not admit, lower, or fingerprint its
/// contents.
pub fn realize_native_artifact_with_callback_custody<C>(
    artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    request: NativeRealizationRequest<'_>,
    callback_custody: C,
) -> Result<RealizedNativeArtifactWithCallbackCustody<C>, CallbackCustodyNativeRealizationError<C>>
{
    match realize_native_artifact(artifact, request) {
        Ok(artifact) => Ok(RealizedNativeArtifactWithCallbackCustody {
            artifact,
            callback_custody,
        }),
        Err(diagnostics) => Err(CallbackCustodyNativeRealizationError {
            diagnostics,
            callback_custody,
        }),
    }
}
