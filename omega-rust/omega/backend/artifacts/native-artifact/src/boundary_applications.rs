use boundary_applications::TerminalBoundaryApplicationCoverage;
use sha2::{Digest, Sha256};

use crate::NativePhysicalEvidenceScope;

const CUSTODY_IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.d29-custody.sha256.v1\0";

pub(super) fn validate_boundary_application_coverage(
    module: &terminal_psi::TerminalModule,
    terminal: terminal_psi::TerminalPsiIdentity,
    coverage: Option<&TerminalBoundaryApplicationCoverage>,
    physical_evidence_scope: &NativePhysicalEvidenceScope,
) -> Result<(), &'static str> {
    let Some(coverage) = coverage else {
        if physical_evidence_scope.requires_boundary_application_coverage() {
            return Err(
                "native artifact claims an exact empty D29 scope without retained coverage",
            );
        }
        return Ok(());
    };
    coverage.validate_for_terminal(terminal)?;
    for reference in coverage.references() {
        let matching_operations = module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
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

pub(super) fn boundary_application_coverage_identity(
    coverage: Option<&TerminalBoundaryApplicationCoverage>,
) -> Option<[u8; 32]> {
    let coverage = coverage?;
    let mut digest = Sha256::new();
    digest.update(CUSTODY_IDENTITY_DOMAIN);
    digest.update(canonical_usize(coverage.references().len()));
    for reference in coverage.references() {
        digest.update(reference.terminal().vocabulary_marker.get().to_le_bytes());
        digest.update(reference.terminal().program_fingerprint.as_bytes());
        digest.update(reference.terminal_operation().get().to_le_bytes());
        digest.update(reference.coverage().as_bytes());
    }
    Some(digest.finalize().into())
}

fn canonical_usize(value: usize) -> [u8; 8] {
    u64::try_from(value)
        .expect("native boundary-application custody length fits u64")
        .to_le_bytes()
}
