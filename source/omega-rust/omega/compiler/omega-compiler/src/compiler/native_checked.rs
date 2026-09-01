use super::{CompileOutputKind, CompileReport};
use crate::pipeline::CheckedCompilation;

/// One native production report paired with the exact checked compilation
/// consumed by that same compiler invocation.
///
/// The carrier is deliberately non-clonable. Its private constructor prevents
/// callers from pairing independently produced checked and native products.
#[derive(Debug)]
#[must_use = "a native compilation receipt retains the checked/native invocation join"]
pub(super) struct NativeCompilationWithCheckedReceipt {
    checked: CheckedCompilation,
    report: CompileReport,
}

impl NativeCompilationWithCheckedReceipt {
    pub(super) fn new(
        checked: CheckedCompilation,
        report: CompileReport,
    ) -> Result<Self, &'static str> {
        if report.output_kind() != CompileOutputKind::RetainedNativeArtifact
            || report.wrote_output()
            || !report.has_consistent_executable_publication_custody()
        {
            return Err("checked native receipt requires one retained native artifact report");
        }
        if report.source_file_count != checked.source_file_count() {
            return Err("checked native receipt source count disagrees with its report");
        }
        let profile = checked
            .selected_target_profile()
            .ok_or("checked native receipt requires one selected target profile")?;
        let native_target = checked
            .selected_native_target()
            .ok_or("checked native receipt requires one selected native target")?;
        if profile.native_target() != native_target {
            return Err("checked native receipt target profile disagrees with its native target");
        }
        let artifact = report
            .retained_native_artifact()
            .ok_or("checked native receipt requires one retained native artifact")?;
        if artifact.target() != native_target {
            return Err("checked native receipt retained artifact target disagrees with checking");
        }
        if let Some(manifest) = report.production_manifest() {
            if manifest.subject().target_profile() != profile
                || manifest.subject().native_target() != native_target
                || !manifest.matches_native_artifact(artifact)
            {
                return Err("checked native receipt production manifest disagrees with checking");
            }
        }
        Ok(Self { checked, report })
    }

    /// Consume the checked/native pairing and return the legacy report.
    pub(super) fn into_report(self) -> CompileReport {
        let Self { checked, report } = self;
        drop(checked);
        report
    }
}
