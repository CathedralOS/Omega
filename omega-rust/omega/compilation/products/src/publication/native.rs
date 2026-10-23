//! Native executable and application-package publication.
use crate::CompileReport;
use native_artifact::{
    PreparedDynamicElfPublication, PreparedExecutablePublication, publish_macos_application_package,
};
use platform_custody::file_publication::{install_staged_files, stage_exact_bytes};

/// Publish the retained native product as one flat executable and replace
/// pre-publication custody with an exact publication receipt.
///
/// Compilation ends before this operation. Path selection and filesystem
/// mutation are an explicit product operation, never another compiler
/// request route.
///
/// A selected macOS GUI product instead installs one complete `.app`
/// package.
pub(super) fn publish(
    mut report: CompileReport,
    build_dir: &std::path::Path,
) -> Result<CompileReport, String> {
    report.validate_component_product_inputs()?;
    if report.retained_dynamic_elf_artifact().is_some() {
        return publish_dynamic_elf(report, build_dir);
    }
    // The entrypoint already checked complete input custody and selected this
    // product; native evidence preparation below owns its additional judgments.
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

    // Complete selected macOS GUI output is one `.app` package; every
    // other selected product keeps the flat executable path. The
    // retained authored name and identifier — not the emitted leaf or a
    // re-derived folder name — supply the package shape.
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

/// Publish a retained dynamically linked ELF product as one flat file and
/// replace its pre-publication custody with an exact dynamic receipt.
///
/// Dynamic output is always Linux ELF, so only the flat route applies.
fn publish_dynamic_elf(
    mut report: CompileReport,
    build_dir: &std::path::Path,
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

/// Stage the complete flat process graph before any member becomes visible.
/// The caller retains its distinct ELF/Mach-O root receipt. Companions keep
/// their own sealed producing products and content-addressed path/byte joins;
/// neither file publication nor co-location supplies InstalledCode authority.
fn publish_flat_native_files(
    output_path: &std::path::Path,
    output: &[u8],
    target: target::NativeTarget,
    components: Vec<std::sync::Arc<native_artifact::NativeComponentProduct>>,
) -> Result<Vec<native_artifact::NativeComponentPublicationReceipt>, String> {
    let directory = output_path
        .parent()
        .ok_or("native process root has no directory")?;
    let mut staged = Vec::with_capacity(components.len().saturating_mul(2) + 1);
    let mut paths = Vec::with_capacity(components.len());
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
        paths.push(path);
    }
    staged.push(stage_exact_bytes(output_path, output, true)?);
    install_staged_files(&staged)?;
    components
        .into_iter()
        .zip(paths)
        .map(|(component, path)| {
            native_artifact::NativeComponentPublicationReceipt::verify(component, path)
        })
        .collect()
}
