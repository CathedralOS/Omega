/*
Publish a finished compiler product to the filesystem.

Compilation itself performs no filesystem installation. `publish_compilation`
selects the retained product from `CompileReport`, stages and installs its
native executable, macOS `.app` package, dynamic ELF file, or `<stem>.psi`
Terminal artifact alongside any independent component companions and required
Windows internet provider library, and then publishes completed build-output
companions.
*/

/*
Repeated publication and check-only reports fail closed before any filesystem
mutation. Flat native process graphs stage all component images, descriptions,
`omega-components.tsv`, and the root executable before atomically installing
the batch via `platform_custody::file_publication`.
*/

use crate::{CompileOutputKind, CompileReport};
use std::path::Path;

pub fn publish_compilation(
    report: CompileReport,
    build_dir: &Path,
) -> Result<CompileReport, String> {
    if report.wrote_output() {
        return Err("publication requires an unpublished product".into());
    }
    let mut report = match report.output_kind() {
        CompileOutputKind::RetainedNativeArtifact => publish_native(report, build_dir)?,
        CompileOutputKind::TerminalArtifact => publish_terminal(report, build_dir)?,
        CompileOutputKind::BuildArtifacts => report,
        _ => return Err("compilation did not retain a publishable product".into()),
    };
    if let Some(outputs) = report.build_outputs_mut() {
        outputs.publish(build_dir)?;
        report.record_build_output_publication()
    } else {
        Ok(report)
    }
}

fn publish_terminal(report: CompileReport, build_dir: &Path) -> Result<CompileReport, String> {
    let artifact = report.retained_terminal_artifact().ok_or_else(|| {
        "terminal publication requires exactly one retained terminal artifact".to_owned()
    })?;
    product_publication::publish_psi(report.root_path(), artifact.artifact(), build_dir)?;
    report.record_terminal_publication()
}

fn publish_native(mut report: CompileReport, build_dir: &Path) -> Result<CompileReport, String> {
    report.validate_component_product_inputs()?;
    let components = report
        .runtime_provisions()
        .map(|runtime| runtime.component_products())
        .unwrap_or_default();
    if let Some(artifact) = report.retained_dynamic_elf_artifact() {
        let (receipt, companions) =
            product_publication::publish_dynamic_elf(artifact, components, build_dir)?;
        if !companions.is_empty() {
            report.record_component_publications(companions)?;
        }
        return report.record_dynamic_elf_publication(receipt);
    }
    let artifact = report.retained_native_artifact().ok_or_else(|| {
        "native publication requires exactly one retained native artifact".to_owned()
    })?;
    let published = product_publication::publish_executable(
        product_publication::ExecutableProduct {
            artifact,
            components,
            intent: report.application_intent(),
            name: report.application_name(),
            identifier: report.application_identifier(),
        },
        build_dir,
    )?;
    if !published.components.is_empty() {
        report.record_component_publications(published.components)?;
    }
    report.record_native_publication(published.receipt, published.package)
}
