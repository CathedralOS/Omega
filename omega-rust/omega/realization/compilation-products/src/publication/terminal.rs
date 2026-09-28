//! Terminal artifact publication, including requested proof pairs.
use super::companions::{appended_file_name_path, remove_stale_companion, stage_psi_pair};
use crate::CompileReport;
use platform_custody::file_publication::{install_staged_files, stage_exact_bytes};
use terminal_codec::PreparedPsiProofPair;

/// Publish the retained Terminal product as one `<root>.psi` artifact,
/// plus its adjacent `.proof` companion when the normalized Build
/// requested Psi PCC. Both members are prepared, validated and staged
/// before either destination is installed. A native proof request reports
/// `Incomplete` rather than downgrading to custody-only success, and an
/// earlier publication's unrequested `.proof` is removed before the new
/// artifact installs so no stale sidecar binds the new bytes.
pub(super) fn publish(
    report: CompileReport,
    build_dir: &std::path::Path,
) -> Result<CompileReport, String> {
    // A Terminal stop cannot satisfy a native proof-product request; the
    // compile route rejects this earlier, and the report fails closed
    // rather than silently downgrading the requested pair.
    if report.pcc_requests().native {
        let outcome = terminal_codec::PccVerificationOutcome::Incomplete(
            terminal_codec::PccIncompleteness::UnsupportedEvidence {
                product: terminal_codec::PccProductKind::Native,
            },
        );
        return Err(format!(
            "native PCC publication is {outcome:?}: a Terminal stop carries no native product"
        ));
    }
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
    let mut pcc_publications = Vec::new();
    let mut staged_products = Vec::new();
    if report.pcc_requests().psi {
        let pair =
            PreparedPsiProofPair::new(artifact.artifact(), report.terminal_admission_profile())
                .map_err(|error| error.to_string())?;
        pcc_publications.push(stage_psi_pair(&pair, &psi_path, &mut staged_products)?);
    } else {
        let psi_bytes = artifact.artifact().to_bytes();
        // Without a Psi proof request no `.proof` companion may remain
        // beside the artifact bytes about to be replaced: an earlier
        // publication's sidecar commits to bytes this run did not write.
        remove_stale_companion(&appended_file_name_path(&psi_path, ".proof"))?;
        staged_products.push(stage_exact_bytes(&psi_path, &psi_bytes, false)?);
    }
    install_staged_files(&staged_products)?;
    report.record_terminal_publication(pcc_publications)
}
