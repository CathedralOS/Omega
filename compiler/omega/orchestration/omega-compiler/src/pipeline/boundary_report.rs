mod builder;

use crate::pipeline::compile_options::CompileOptions;
use builder::{append_capability_blast_radius, build_boundary_report};
use omega_artifacts::ArtifactWriter;
use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_syntax_trees::SyntaxTrees;

pub(super) fn write_boundary_report(
    options: &CompileOptions,
    syntax: &SyntaxTrees,
    emit_auxiliary_artifacts: bool,
) -> Result<(), Vec<Diagnostic>> {
    if !emit_auxiliary_artifacts {
        return Ok(());
    }
    let report = build_boundary_report(syntax);

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_boundary_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}

/// Rewrites the boundary report once checked facts are available, adding the
/// capability blast-radius surface (authority ceiling and authority-flow verbs).
pub(super) fn write_boundary_report_with_capabilities(
    options: &CompileOptions,
    syntax: &SyntaxTrees,
    checked: &CheckedTrees,
    emit_auxiliary_artifacts: bool,
) -> Result<(), Vec<Diagnostic>> {
    let mut report = build_boundary_report(syntax);
    append_capability_blast_radius(&mut report, checked)?;

    if !emit_auxiliary_artifacts {
        return Ok(());
    }

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_boundary_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}
