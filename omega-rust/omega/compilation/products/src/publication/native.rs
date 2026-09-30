//! Native executable and application-package publication, including requested proof pairs.
use super::companions::psi_pair_receipt;
use super::companions::{appended_file_name_path, remove_stale_companion, stage_psi_pair};
use crate::CompileReport;
use crate::PccPublicationReceipt;
use native_artifact::{
    PreparedDynamicElfPublication, PreparedExecutablePublication, build_native_proof_sidecar,
    publish_macos_application_package, validate_native_pair,
};
use platform_custody::file_publication::{install_staged_files, stage_exact_bytes};
use terminal_codec::PreparedPsiProofPair;

/// Publish the retained native product as one flat executable and replace
/// pre-publication custody with an exact publication receipt.
///
/// Compilation ends before this operation. Path selection and filesystem
/// mutation are an explicit product operation, never another compiler
/// request route.
///
/// A selected macOS GUI product instead installs one complete `.app`
/// package whose staged tree includes the requested pairs beside the
/// inner executable. On the flat route an unrequested pair cannot be left
/// behind: a previous publication's `.psi`/`.proof` companions are removed
/// before new executable bytes install, so no stale sidecar is ever
/// associated with bytes this report did not write.
pub(super) fn publish(
    report: CompileReport,
    build_dir: &std::path::Path,
) -> Result<CompileReport, String> {
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

    // A requested native pair is built and self-checked before any byte
    // is installed: the placed-image evidence must decode canonically and
    // replay against the exact bytes about to be published under a
    // self-consistent policy, so a pair that cannot carry its bounded
    // claim never produces a certified-looking install. The behavioral
    // remainder of a native claim still reports `Incomplete` to receivers;
    // that is the honest ceiling of the shipped evidence, and `Reject`
    // here would be a producer defect rather than a publishable pair.
    let native_sidecar_bytes = if report.pcc_requests().native {
        let sidecar = build_native_proof_sidecar(
            artifact,
            report.terminal_admission_profile(),
            &output.bytes,
        )?;
        validate_native_pair(&output.bytes, &sidecar, report.terminal_admission_profile())?;
        Some(sidecar.to_bytes())
    } else {
        None
    };

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
    let mut pcc_publications = Vec::new();
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
        let mut companions = Vec::new();
        // Requested artifact/companion pairs assemble inside the package
        // beside the executable: one staged tree, one rename, so a failed
        // pair can never leave a certified-looking install.
        if report.pcc_requests().psi {
            let pair = PreparedPsiProofPair::new(
                artifact.psi_artifact(),
                report.terminal_admission_profile(),
            )
            .map_err(|error| error.to_string())?;
            companions.push((".psi".to_owned(), pair.artifact_bytes().to_vec()));
            companions.push((".psi.proof".to_owned(), pair.sidecar_bytes().to_vec()));
            let package_root = build_dir.join(format!("{application_name}.app"));
            let macos_dir = std::path::Path::new("Contents").join("MacOS");
            let psi_path = package_root.join(macos_dir.join(format!("{application_name}.psi")));
            pcc_publications.push(psi_pair_receipt(&pair, &psi_path));
        }
        // The native companion sits beside the inner executable as
        // `Contents/MacOS/<name>.proof`; the staged rename makes its
        // install atomic with the executable it commits to.
        if let Some(native_sidecar_bytes) = &native_sidecar_bytes {
            companions.push((".proof".to_owned(), native_sidecar_bytes.clone()));
            let package_root = build_dir.join(format!("{application_name}.app"));
            let macos_dir = std::path::Path::new("Contents").join("MacOS");
            pcc_publications.push(PccPublicationReceipt {
                product: terminal_codec::PccProductKind::Native,
                artifact_path: package_root.join(macos_dir.join(&application_name)),
                artifact_byte_len: output.bytes.len() as u64,
                sidecar_path: package_root
                    .join(macos_dir.join(format!("{application_name}.proof"))),
                sidecar_byte_len: native_sidecar_bytes.len() as u64,
            });
        }
        let receipt = publish_macos_application_package(
            build_dir,
            &application_name,
            &application_identifier,
            &output.bytes,
            container_digest,
            &companions,
        )?;
        let output_path = receipt.inner_executable_path().ok_or_else(|| {
            "macOS package publication produced no inner executable component".to_owned()
        })?;
        package_publication = Some(receipt);
        output_path
    } else {
        let output_path = build_dir.join(&output.file_name);

        // Every requested artifact/companion pair is staged and validated
        // before ANY member becomes visible. Publishing them one at a time
        // was the gap: a failure after the artifact and before its
        // companion left new bytes beside the previous run's `.proof`, and
        // "Publication must not associate a stale sidecar with newly
        // written bytes" (spec, proofs/publication.md). A staging failure
        // leaves the previous consistent pair untouched; an installation
        // failure removes the members this attempt installed.
        let mut staged_products = Vec::new();
        if report.pcc_requests().psi {
            let pair = PreparedPsiProofPair::new(
                artifact.psi_artifact(),
                report.terminal_admission_profile(),
            )
            .map_err(|error| error.to_string())?;
            let psi_path = appended_file_name_path(&output_path, ".psi");
            pcc_publications.push(stage_psi_pair(&pair, &psi_path, &mut staged_products)?);
        } else {
            // An earlier Psi-requested publication's companions must not
            // survive beside an executable they do not commit to; removing
            // them cannot create a certified-looking install, so removal
            // precedes the new bytes. Package publication needs no such
            // pass: the staged tree rename replaces every prior member.
            let psi_path = appended_file_name_path(&output_path, ".psi");
            remove_stale_companion(&appended_file_name_path(&psi_path, ".proof"))?;
            remove_stale_companion(&psi_path)?;
        }
        if let Some(native_sidecar_bytes) = &native_sidecar_bytes {
            let native_sidecar_path = appended_file_name_path(&output_path, ".proof");
            staged_products.push(stage_exact_bytes(
                &native_sidecar_path,
                native_sidecar_bytes,
                false,
            )?);
            pcc_publications.push(PccPublicationReceipt {
                product: terminal_codec::PccProductKind::Native,
                artifact_path: output_path.clone(),
                artifact_byte_len: output.bytes.len() as u64,
                sidecar_path: native_sidecar_path,
                sidecar_byte_len: native_sidecar_bytes.len() as u64,
            });
        } else {
            // An earlier native-PCC publication's sidecar must not
            // survive beside executable bytes it does not commit to.
            remove_stale_companion(&appended_file_name_path(&output_path, ".proof"))?;
        }
        // The executable joins the same staged set. Only after every member
        // stages successfully does sequential installation begin.
        staged_products.push(stage_exact_bytes(&output_path, &output.bytes, true)?);
        install_staged_files(&staged_products)?;
        output_path
    };

    let receipt = publication.into_receipt(output_path)?;

    report.record_native_publication(receipt, pcc_publications, package_publication)
}

/// Publish a retained dynamically linked ELF product as one flat file and
/// replace its pre-publication custody with an exact dynamic receipt.
///
/// Dynamic output is always Linux ELF, so only the flat route applies. A
/// requested Psi pair stages beside the executable exactly as on the direct
/// route; a native proof product was already refused when the report
/// retained its proof context. Every stale companion that no longer commits
/// to these bytes is removed before the new bytes install.
fn publish_dynamic_elf(
    report: CompileReport,
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
    let mut staged_products = Vec::new();
    let mut pcc_publications = Vec::new();
    let psi_path = appended_file_name_path(&output_path, ".psi");
    if report.pcc_requests().psi {
        let pair =
            PreparedPsiProofPair::new(artifact.psi_artifact(), report.terminal_admission_profile())
                .map_err(|error| error.to_string())?;
        pcc_publications.push(stage_psi_pair(&pair, &psi_path, &mut staged_products)?);
    } else {
        remove_stale_companion(&appended_file_name_path(&psi_path, ".proof"))?;
        remove_stale_companion(&psi_path)?;
    }
    remove_stale_companion(&appended_file_name_path(&output_path, ".proof"))?;
    staged_products.push(stage_exact_bytes(&output_path, &output.bytes, true)?);
    install_staged_files(&staged_products)?;
    let receipt = publication.into_receipt(output_path);
    report.record_dynamic_elf_publication(receipt, pcc_publications)
}
