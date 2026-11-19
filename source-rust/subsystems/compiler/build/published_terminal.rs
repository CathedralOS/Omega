// Inspect an explicitly retained completed `NativeArtifact` to construct the
// interpreter's `TerminalNativeCodeLayoutEnvironment`. Private object machine
// numbers are local to their own semantic artifacts, so we filter to layouts
// matching this root's `TerminalPsiIdentity` without private function indirection.

use terminal_interpreter::{
    TerminalNativeCodeLayout, TerminalNativeCodeLayoutEnvironment, TerminalNativeCodeRegion,
};
use terminal_runtime::embedding::{HostedTerminalProgram, execute_with_image_installation};
use terminal_runtime::{TerminalRuntimeError, TerminalRuntimeObservation, TerminalRuntimeRequest};

use product_publication::RetainedNativeArtifact;

/// Run the portable program with metadata from the optional completed product.
/// Observations of addresses or regions cannot be used as a native product.
///
/// ```compile_fail
/// fn run_with_host_addresses(request: terminal_runtime::TerminalRuntimeRequest<'_>,
///                            addresses: Vec<(u64, u64)>) {
///     project_build::published_terminal::execute_published_terminal(request, Some(&addresses)).unwrap();
/// }
/// ```
pub fn execute_published_terminal(
    request: TerminalRuntimeRequest<'_>,
    image: Option<&RetainedNativeArtifact>,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    execute_published_terminal_sliced(request, image, std::num::NonZeroU64::MAX)
}

pub fn execute_published_terminal_sliced(
    request: TerminalRuntimeRequest<'_>,
    image: Option<&RetainedNativeArtifact>,
    quantum: std::num::NonZeroU64,
) -> Result<TerminalRuntimeObservation, TerminalRuntimeError> {
    let Some(image) = image else {
        return terminal_runtime::execute_sliced(request, quantum);
    };
    let host = HostedTerminalProgram::prepare(request)?;
    let layouts = inspect(image, host.request())?;
    let installation = host
        .program()
        .admit_runtime_supplies(terminal_interpreter::TerminalRuntimeSupplyRequest {
            native_code_layouts: Some(&layouts),
            ..host.supplies()
        })
        .map_err(TerminalRuntimeError::Requirements)?;
    execute_with_image_installation(host, &installation, quantum)
}

fn inspect(
    image: &RetainedNativeArtifact,
    request: &TerminalRuntimeRequest<'_>,
) -> Result<TerminalNativeCodeLayoutEnvironment, TerminalRuntimeError> {
    let artifact = request.artifact;
    let execution_target = request.target.or_else(|| {
        request
            .provisions
            .map(|provisions| provisions.target_slot().owner.target_name())
    });
    let Some(target) = execution_target else {
        return Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection requires the completed image's target",
        ));
    };
    let selected_target =
        require_valid_inspection(target::NativeTarget::from_omega_target_name(Some(target)))?;
    if image.psi_artifact().manifest().identity() != artifact.manifest().identity()
        || image.psi_artifact().view().identity() != artifact.view().identity()
        || image.target() != selected_target
    {
        return Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target",
        ));
    }
    let descriptions = require_valid_inspection(image.inspect_root_code_layouts())?;
    let layouts = descriptions
        .into_iter()
        .map(|layout| TerminalNativeCodeLayout {
            machine: layout.machine,
            entry: layout.entry,
            regions: layout
                .regions
                .into_iter()
                .map(|(base, length)| TerminalNativeCodeRegion { base, length })
                .collect(),
        })
        .collect();
    require_valid_inspection(TerminalNativeCodeLayoutEnvironment::linked_image(
        artifact.view().identity(),
        target,
        *image.identity().as_bytes(),
        layouts,
    ))
}

// A supplied image that fails inspection is invalid, not an absent host capability.
// Keep that distinction for every inspection step, including environment assembly.
fn require_valid_inspection<T>(
    result: Result<T, impl std::fmt::Display>,
) -> Result<T, TerminalRuntimeError> {
    result.map_err(|reason| TerminalRuntimeError::NativeArtifactInspection(reason.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{TerminalNativeCodeLayoutEnvironment, require_valid_inspection};

    #[test]
    fn invalid_native_inspection_is_not_comparison_unavailability() {
        let invalid_target = target::NativeTarget::from_omega_target_name(Some("invalid-target"));
        let expected_reason = invalid_target
            .as_ref()
            .expect_err("invalid target")
            .to_string();
        let error = require_valid_inspection(invalid_target).expect_err("refuse invalid target");
        assert!(!error.comparison_unavailable());
        assert!(!error.unavailable_before_guest_effects());
        assert!(error.to_string().contains(&expected_reason));
    }

    #[test]
    fn malformed_native_layout_environment_is_a_failure() {
        let semantic = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([1; 32]),
        };
        let invalid = TerminalNativeCodeLayoutEnvironment::linked_image(
            semantic,
            "windows_x86_64",
            [0; 32],
            Vec::new(),
        );
        let error = require_valid_inspection(invalid).expect_err("refuse missing image identity");
        assert!(!error.comparison_unavailable());
        assert!(!error.unavailable_before_guest_effects());
        assert!(
            error
                .to_string()
                .contains("invalid completed-image metadata")
        );
    }
}
