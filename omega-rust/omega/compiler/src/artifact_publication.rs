//! Publish a finished compiler product. Compilation itself performs no installation.
//!
//! One entrypoint selects the retained product, prepares and installs its exact
//! artifact/proof pairs, then publishes completed build companions. Report methods
//! only record the resulting custody transitions. Backend owners retain image
//! evidence and platform installation mechanics.
mod build_outputs;
mod companions;
mod native;
mod terminal;

use crate::{CompileOutputKind, CompileReport};
use std::path::Path;

/// Publish one retained product and its completed build outputs.
/// Repeated publication and check-only reports reject before filesystem mutation.
/// Requested proof pairs are checked before installation; no build or compiler
/// stage is re-entered here. Failures return no successful publication report.
pub fn publish_compilation(
    report: CompileReport,
    build_dir: &Path,
) -> Result<CompileReport, String> {
    if report.wrote_output() || !report.has_consistent_executable_publication_custody() {
        return Err("publication requires an unpublished product with consistent custody".into());
    }
    let report = match report.output_kind() {
        CompileOutputKind::RetainedNativeArtifact => native::publish(report, build_dir)?,
        CompileOutputKind::TerminalArtifact => terminal::publish(report, build_dir)?,
        CompileOutputKind::BuildArtifacts => report,
        _ => return Err("compilation did not retain a publishable product".into()),
    };
    if let Some(outputs) = report.build_outputs() {
        let directory = build_outputs::publish(outputs, build_dir)?;
        report.record_build_output_publication(directory)
    } else {
        Ok(report)
    }
}
