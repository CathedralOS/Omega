use crate::physical::{canonical_usize, hash_bytes};
use boundary_applications::TerminalBoundaryApplicationCoverage;
use sha2::{Digest, Sha256};

use crate::NativePhysicalEvidenceScope;

const CUSTODY_IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.d29-custody.sha256.v2\0";

pub(super) fn validate_boundary_application_coverage(
    module: &terminal_psi::TerminalModule,
    terminal: terminal_psi::TerminalPsiIdentity,
    coverage: Option<&TerminalBoundaryApplicationCoverage>,
    physical_evidence_scope: &NativePhysicalEvidenceScope,
) -> Result<(), &'static str> {
    let Some(coverage) = coverage else {
        if !module.boundary_application_demands.is_empty()
            || physical_evidence_scope.requires_boundary_application_coverage()
        {
            return Err(
                "native artifact claims an exact empty D29 scope without retained coverage",
            );
        }
        return Ok(());
    };
    coverage.validate_for_terminal(terminal)?;
    if coverage.demands().rows() != module.boundary_application_demands {
        return Err("native boundary coverage does not match canonical Terminal demands");
    }
    for reference in coverage.references() {
        let matching_operations = module
            .operations()
            .filter(|operation| operation.id == reference.terminal_operation())
            .count();
        if matching_operations != 1 {
            return Err(
                "native artifact boundary application does not rejoin one Terminal operation",
            );
        }
    }
    Ok(())
}

pub(crate) fn boundary_application_coverage_identity(
    coverage: Option<&TerminalBoundaryApplicationCoverage>,
) -> Option<[u8; 32]> {
    let coverage = coverage?;
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(CUSTODY_IDENTITY_DOMAIN);
    digest.raw(&canonical_usize(coverage.references().len()));
    for reference in coverage.references() {
        digest.raw(&reference.terminal().vocabulary_marker.get().to_le_bytes());
        digest.raw(reference.terminal().program_fingerprint.as_bytes());
        digest.raw(&reference.terminal_operation().get().to_le_bytes());
        digest.raw(reference.coverage().as_bytes());
    }
    let applications = coverage.opaque_applications();
    digest.raw(&canonical_usize(applications.rows().len()));
    for application in applications.rows() {
        hash_bytes(&mut digest, application.requirement_identity.as_bytes());
        digest.raw(&application.shape_root.to_le_bytes());
        digest.raw(&application.application_report_fingerprint.to_le_bytes());
        digest.raw(&application.selected_application_commitment);
    }
    Some(digest.finish())
}
