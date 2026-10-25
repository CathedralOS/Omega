//! Application-owned publication of compiler outputs.
//!
//! The compiler stops at an in-memory semantic product. Choosing a path and
//! making bytes visible is command/product policy, not compilation.

use crate::{BuildError, BuildOutcome, NativeDestination, ProjectProduct, TargetBuildOutcome};
use artifacts::compile_timings::{CompileTimings, StageMeta};
use compiler::CompilationOutput;
use std::path::{Path, PathBuf};

/// Preserve every target's outcome. Checks, inspections and retained requests publish
/// no bytes; published products use their own admitted canonical directory.
pub(crate) fn publish_products(
    work: Vec<crate::build_directory::TargetWork>,
    mut reports: Vec<(target::TargetProfile, Result<CompilationOutput, BuildError>)>,
    product: &ProjectProduct,
    timings: &mut CompileTimings,
) -> Vec<TargetBuildOutcome> {
    let mut outcomes = Vec::with_capacity(work.len());
    for target in work {
        let result = match target.directory {
            Err(error) => Err(error),
            Ok(directory) => {
                let report = match reports
                    .iter()
                    .position(|(profile, _)| *profile == target.profile)
                {
                    Some(position) => reports.remove(position).1,
                    None => Err(BuildError::Diagnostics(vec![
                        diagnostics::Diagnostic::error(format!(
                            "build lost the requested product for {}",
                            target.profile.target_name(),
                        )),
                    ])),
                };
                match report {
                    Err(error) => Err(error),
                    Ok(report) => publish_product(report, product, &directory, timings),
                }
            }
        };
        outcomes.push(TargetBuildOutcome {
            target: target.profile,
            result,
        });
    }
    outcomes
}

fn publish_product(
    output: CompilationOutput,
    product: &ProjectProduct,
    directory: &Path,
    timings: &mut CompileTimings,
) -> Result<BuildOutcome, BuildError> {
    let (output, executable_path) = match product {
        ProjectProduct::Check
        | ProjectProduct::Inspection { .. }
        | ProjectProduct::Native {
            destination: NativeDestination::Retained,
            ..
        } => (output, None),
        ProjectProduct::Native {
            destination: NativeDestination::Published,
            ..
        } => {
            let report = output
                .into_report()
                .map_err(|error| BuildError::Diagnostics(error.into_diagnostics()))?;
            let (report, path) = timings
                .record_result(
                    StageMeta::new("publish", "retained product", "published product"),
                    || publish_compilation(report, directory),
                )
                .map_err(BuildError::Publication)?;
            (CompilationOutput::Report(report), path)
        }
    };
    Ok(BuildOutcome {
        output,
        timings: timings.clone(),
        executable_path,
    })
}

pub fn publish_compilation(
    report: compilation_products::CompileReport,
    build_dir: &Path,
) -> Result<(compilation_products::CompileReport, Option<PathBuf>), String> {
    let report = compilation_products::publication::publish_compilation(report, build_dir)?;
    let executable = report
        .checked_native_executable_path()
        .map(Path::to_path_buf);
    Ok((report, executable))
}

pub fn publish_native_artifact(
    report: compilation_products::CompileReport,
    build_dir: &Path,
) -> Result<(compilation_products::CompileReport, PathBuf), String> {
    if report.output_kind() == compilation_products::CompileOutputKind::BuildArtifacts {
        return Err("an artifact-only build has no executable to run".into());
    }
    if report.output_kind() != compilation_products::CompileOutputKind::RetainedNativeArtifact {
        return Err("running requires a retained native executable product".into());
    }
    let (published, path) = publish_compilation(report, build_dir)?;
    let path =
        path.ok_or_else(|| "native publication did not retain executable custody".to_owned())?;
    Ok((published, path))
}
