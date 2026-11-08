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
use native_artifact::{
    PreparedDynamicElfPublication, PreparedExecutablePublication, publish_macos_application_package,
};
use platform_custody::file_publication::{install_staged_files, stage_exact_bytes};
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
    let stem = report
        .root_path()
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .ok_or_else(|| "terminal publication requires a named root file".to_owned())?;
    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;
    let psi_path = build_dir.join(format!("{stem}.psi"));
    let psi_bytes = artifact.artifact().to_bytes();
    let staged_products = vec![stage_exact_bytes(&psi_path, &psi_bytes, false)?];
    install_staged_files(&staged_products)?;
    report.record_terminal_publication()
}

fn publish_native(mut report: CompileReport, build_dir: &Path) -> Result<CompileReport, String> {
    report.validate_component_product_inputs()?;
    if report.retained_dynamic_elf_artifact().is_some() {
        return publish_dynamic_elf(report, build_dir);
    }
    let artifact = report.retained_native_artifact().ok_or_else(|| {
        "native publication requires exactly one retained native artifact".to_owned()
    })?;
    let publication = PreparedExecutablePublication::new(artifact)?;
    let output = artifact.image().output();

    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;

    let package = artifact.target().object_format == target::ObjectFormat::MachO
        && matches!(
            report.application_intent(),
            Some(target::HostedApplicationIntent::Gui)
        );
    let container_digest = publication.container_digest();
    let components = report
        .runtime_provisions()
        .map(|runtime| runtime.component_products().to_vec())
        .unwrap_or_default();
    if package && !components.is_empty() {
        return Err("macOS application v1 publication admits only its exact executable/plist tree; component companion placement requires its separate package contract".into());
    }
    let mut component_receipts = Vec::new();
    let mut package_publication = None;
    let output_path = if package {
        let application_name = report
            .application_name()
            .map(str::to_owned)
            .ok_or_else(|| {
                "macOS GUI package publication requires the retained authored application name"
                    .to_owned()
            })?;
        let application_identifier = report.application_identifier().cloned().ok_or_else(|| {
            "macOS GUI package publication requires the retained authored application identifier"
                .to_owned()
        })?;
        let receipt = publish_macos_application_package(
            build_dir,
            &application_name,
            &application_identifier,
            &output.bytes,
            container_digest,
        )?;
        let output_path = receipt.inner_executable_path().ok_or_else(|| {
            "macOS package publication produced no inner executable component".to_owned()
        })?;
        package_publication = Some(receipt);
        output_path
    } else {
        let output_path = build_dir.join(&output.file_name);
        component_receipts =
            publish_flat_native_files(&output_path, &output.bytes, artifact.target(), components)?;
        output_path
    };

    let receipt = publication.into_receipt(output_path)?;
    if !component_receipts.is_empty() {
        report.record_component_publications(component_receipts)?;
    }

    report.record_native_publication(receipt, package_publication)
}

fn publish_dynamic_elf(
    mut report: CompileReport,
    build_dir: &Path,
) -> Result<CompileReport, String> {
    let artifact = report.retained_dynamic_elf_artifact().ok_or_else(|| {
        "dynamic ELF publication requires exactly one retained dynamic ELF artifact".to_owned()
    })?;
    let publication = PreparedDynamicElfPublication::new(artifact)?;
    let output = artifact.image().output();
    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;
    let output_path = build_dir.join(&output.file_name);
    let components = report
        .runtime_provisions()
        .map(|runtime| runtime.component_products().to_vec())
        .unwrap_or_default();
    let receipts =
        publish_flat_native_files(&output_path, &output.bytes, artifact.target(), components)?;
    let receipt = publication.into_receipt(output_path);
    if !receipts.is_empty() {
        report.record_component_publications(receipts)?;
    }
    report.record_dynamic_elf_publication(receipt)
}

fn publish_flat_native_files(
    output_path: &Path,
    output: &[u8],
    target: target::NativeTarget,
    components: Vec<std::sync::Arc<native_artifact::NativeComponentProduct>>,
) -> Result<Vec<native_artifact::NativeComponentPublicationReceipt>, String> {
    let directory = output_path
        .parent()
        .ok_or("native process root has no directory")?;
    let mut staged = Vec::with_capacity(components.len().saturating_mul(2) + 2);
    let mut paths = Vec::with_capacity(components.len());
    let mut export_index = String::from("omega-native-components-v1\n");
    for component in &components {
        if component.locator().target().native_target() != target {
            return Err("component file target differs from its selected process root".into());
        }
        let path = directory.join(component.locator().image_filename());
        component
            .verify_contents(component.image().image().contents())
            .map_err(str::to_owned)?;
        staged.push(stage_exact_bytes(
            &path,
            component.image().image().contents(),
            false,
        )?);
        staged.push(stage_exact_bytes(
            &directory.join(component.description_filename()),
            component.description(),
            false,
        )?);
        let requirement = &component.application().requirement_identity;
        if requirement.contains(['\t', '\r', '\n']) {
            return Err("component requirement cannot be represented in the export index".into());
        }
        export_index.push_str(&format!(
            "{}\t{}\t{}\n",
            requirement,
            component.locator().image_filename(),
            component.locator().export_symbol(),
        ));
        paths.push(path);
    }
    staged.push(stage_exact_bytes(
        &directory.join("omega-components.tsv"),
        export_index.as_bytes(),
        false,
    )?);
    staged.push(stage_exact_bytes(output_path, output, true)?);
    if target.object_format == target::ObjectFormat::Coff {
        let provider = runtime_abi::WINDOWS_INTERNET_PROVIDER_LIBRARY;
        let imported = runtime_abi::image_imports_windows_internet_provider(output)
            || components.iter().any(|component| {
                runtime_abi::image_imports_windows_internet_provider(
                    component.image().image().contents(),
                )
            });
        if imported {
            let library = std::str::from_utf8(provider)
                .map_err(|_| "windows internet provider library name is not utf-8")?;
            if components
                .iter()
                .any(|component| component.locator().image_filename() == library)
            {
                return Err(
                    "a component image already uses the windows internet provider library name"
                        .into(),
                );
            }
            staged.push(stage_exact_bytes(
                &directory.join(library),
                &runtime_abi::windows_internet_provider_dll(),
                false,
            )?);
        }
    }
    install_staged_files(&staged)?;
    components
        .into_iter()
        .zip(paths)
        .map(|(component, path)| {
            native_artifact::NativeComponentPublicationReceipt::verify(component, path)
        })
        .collect()
}
