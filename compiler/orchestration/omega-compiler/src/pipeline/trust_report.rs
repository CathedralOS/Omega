mod builder;
mod validation;

use crate::pipeline::compile_options::CompileOptions;
use builder::build_trust_report;
use omega_artifacts::ArtifactWriter;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::SyntaxTrees;
use validation::validate_trust_report;

pub(super) fn write_trust_report(
    options: &CompileOptions,
    syntax: &SyntaxTrees,
) -> Result<(), Vec<Diagnostic>> {
    let report = build_trust_report(syntax);
    validate_trust_report(&report)?;

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_trust_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}
