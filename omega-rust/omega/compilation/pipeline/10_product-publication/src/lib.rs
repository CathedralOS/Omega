#![forbid(unsafe_code)]
//! Stage 10 installs finished Psi, native and component files and returns the
//! receipts for the exact installed products. Receipts are issued after staging.

mod output;

use native_artifact::{
    DynamicElfPublicationReceipt, NativeComponentProduct, NativeComponentPublicationReceipt,
    PreparedDynamicElfPublication, PreparedExecutablePublication,
    publish_macos_application_package,
};
pub use native_artifact::{
    ExecutableContainerDigest, ExecutableInstallationEvidenceDigest, ExecutablePublicationReceipt,
    NativeForeignAdapterImage, NativePackagePublicationReceipt, NativePublicationEvidenceDigest,
    NativePublicationInputsDigest, PackagePublicationComponent, RetainedDynamicElfArtifact,
    RetainedNativeArtifact, TerminalRuntimeHostProvision, TerminalRuntimeProvisions,
    executable_installation_evidence_digest,
};
pub use output::{
    CompileOutputKind, CompletedProduct, FinishedNativeImage, RetainedNativeProgram,
    RetainedNativeProgramIdentity,
};
use platform_custody::file_publication::{install_staged_files, stage_exact_bytes};
use std::path::Path;
use std::sync::Arc;

fn publish_psi(
    root: &Path,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    build_dir: &Path,
) -> Result<(), String> {
    let stem = root
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
    let psi_bytes = artifact.to_bytes();
    let staged_products = vec![stage_exact_bytes(&psi_path, &psi_bytes, false)?];
    install_staged_files(&staged_products)
}

struct ExecutableProduct<'a> {
    artifact: &'a RetainedNativeArtifact,
    components: &'a [Arc<NativeComponentProduct>],
    intent: Option<target::HostedApplicationIntent>,
    name: Option<&'a str>,
    identifier: Option<&'a target::ApplicationIdentifier>,
}

struct PublishedExecutable {
    receipt: ExecutablePublicationReceipt,
    package: Option<NativePackagePublicationReceipt>,
    components: Vec<NativeComponentPublicationReceipt>,
}

fn publish_executable(
    product: ExecutableProduct<'_>,
    build_dir: &Path,
) -> Result<PublishedExecutable, String> {
    let ExecutableProduct {
        artifact,
        components,
        intent,
        name,
        identifier,
    } = product;
    let publication = PreparedExecutablePublication::new(artifact)?;
    let output = artifact.image().output();

    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;

    let package = artifact.target().object_format == target::ObjectFormat::MachO
        && matches!(intent, Some(target::HostedApplicationIntent::Gui));
    let container_digest = publication.container_digest();
    if package && !components.is_empty() {
        return Err("macOS application v1 publication admits only its exact executable/plist tree; component companion placement requires its separate package contract".into());
    }
    let mut component_receipts = Vec::new();
    let mut package_publication = None;
    let output_path = if package {
        let application_name = name.ok_or_else(|| {
            "macOS GUI package publication requires the retained authored application name"
                .to_owned()
        })?;
        let application_identifier = identifier.ok_or_else(|| {
            "macOS GUI package publication requires the retained authored application identifier"
                .to_owned()
        })?;
        let receipt = publish_macos_application_package(
            build_dir,
            application_name,
            application_identifier,
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
        component_receipts = publish_flat_native_files(
            &output_path,
            &output.bytes,
            artifact.target(),
            artifact.image().companion_images(),
            components,
        )?;
        output_path
    };

    let receipt = publication.into_receipt(output_path)?;
    Ok(PublishedExecutable {
        receipt,
        package: package_publication,
        components: component_receipts,
    })
}

fn publish_dynamic_elf(
    artifact: &RetainedDynamicElfArtifact,
    components: &[Arc<NativeComponentProduct>],
    build_dir: &Path,
) -> Result<
    (
        DynamicElfPublicationReceipt,
        Vec<NativeComponentPublicationReceipt>,
    ),
    String,
> {
    let publication = PreparedDynamicElfPublication::new(artifact)?;
    let output = artifact.image().output();
    std::fs::create_dir_all(build_dir).map_err(|error| {
        format!(
            "failed to create output directory {}: {error}",
            build_dir.display()
        )
    })?;
    let output_path = build_dir.join(&output.file_name);
    let receipts = publish_flat_native_files(
        &output_path,
        &output.bytes,
        artifact.target(),
        &[],
        components,
    )?;
    let receipt = publication.into_receipt(output_path);
    Ok((receipt, receipts))
}

fn publish_flat_native_files(
    output_path: &Path,
    output: &[u8],
    target: target::NativeTarget,
    root_companions: &[image_emission::NativeCompanionImage],
    components: &[Arc<NativeComponentProduct>],
) -> Result<Vec<native_artifact::NativeComponentPublicationReceipt>, String> {
    let directory = output_path
        .parent()
        .ok_or("native process root has no directory")?;
    let mut staged = Vec::with_capacity(components.len().saturating_mul(2) + 2);
    let mut paths = Vec::with_capacity(components.len());
    let mut export_index = String::from("omega-native-components-v1\n");
    for component in components {
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
    let mut companions = Vec::<&image_emission::NativeCompanionImage>::new();
    for companion in root_companions.iter().chain(
        components
            .iter()
            .flat_map(|component| component.image().companion_images()),
    ) {
        if components
            .iter()
            .any(|component| component.locator().image_filename() == companion.filename())
            || output_path
                .file_name()
                .is_some_and(|name| name == companion.filename())
        {
            return Err("a native product already uses a compiler companion filename".into());
        }
        if let Some(previous) = companions
            .iter()
            .find(|previous| previous.filename() == companion.filename())
        {
            if previous.contents() != companion.contents() {
                return Err("native products disagree on a compiler companion image".into());
            }
        } else {
            companions.push(companion);
        }
    }
    for companion in companions {
        staged.push(stage_exact_bytes(
            &directory.join(companion.filename()),
            companion.contents(),
            false,
        )?);
    }
    install_staged_files(&staged)?;
    components
        .iter()
        .cloned()
        .zip(paths)
        .map(|(component, path)| {
            native_artifact::NativeComponentPublicationReceipt::verify(component, path)
        })
        .collect()
}
