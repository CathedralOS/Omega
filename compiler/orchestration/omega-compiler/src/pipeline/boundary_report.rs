mod builder;

use crate::pipeline::compile_options::CompileOptions;
use builder::build_boundary_report;
use omega_artifacts::ArtifactWriter;
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
