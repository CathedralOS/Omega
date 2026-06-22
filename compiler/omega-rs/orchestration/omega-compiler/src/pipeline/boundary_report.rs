mod builder;

use crate::pipeline::compile_options::CompileOptions;
use builder::{append_capability_blast_radius, build_boundary_report};
use omega_artifacts::ArtifactWriter;
use omega_checked_trees::CheckedTrees;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;

pub(super) fn write_boundary_report(
    options: &CompileOptions,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
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
) -> Result<(), Vec<Diagnostic>> {
    let mut report = build_boundary_report(syntax);
    append_capability_blast_radius(&mut report, checked);

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_boundary_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}
