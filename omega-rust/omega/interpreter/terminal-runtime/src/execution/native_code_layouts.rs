// Inspect an explicitly retained completed `NativeArtifact` to construct the
// interpreter's `TerminalNativeCodeLayoutEnvironment`. Private object machine
// numbers are local to their own semantic artifacts, so we filter to layouts
// matching this root's `TerminalPsiIdentity` without private function indirection.

use super::TerminalRuntimeError;
use published_runtime_input::RuntimeImageInspection;
use terminal_interpreter::{
    TerminalNativeCodeLayout, TerminalNativeCodeLayoutEnvironment, TerminalNativeCodeRegion,
};

pub(super) fn inspect(
    image: Option<RuntimeImageInspection>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    execution_target: Option<&str>,
) -> Result<Option<TerminalNativeCodeLayoutEnvironment>, TerminalRuntimeError> {
    let Some(image) = image else {
        return Ok(None);
    };
    let Some(target) = execution_target else {
        return Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection requires the completed image's target",
        ));
    };
    let selected_target =
        require_valid_inspection(target::NativeTarget::from_omega_target_name(Some(target)))?;
    if !image.matches(artifact, selected_target) {
        return Err(TerminalRuntimeError::InvalidProvision(
            "native layout inspection names another artifact or target",
        ));
    }
    let (identity, descriptions) = require_valid_inspection(image.into_layouts())?;
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
        identity,
        layouts,
    ))
    .map(Some)
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
