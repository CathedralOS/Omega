use psi_optimization::PsiOptimizationSelectionIdentity;
use psi_terminal::TerminalPsiIdentity;
use psi_terminal_codec::ProofBundleFingerprint;
use sha2::{Digest, Sha256};

use super::PsiOptimizationExecutionIdentity;

const EXECUTION_IDENTITY_DOMAIN: &[u8] = b"psi.preterminal-optimization-execution.v1\0";

pub(super) fn execution_identity(
    selection: PsiOptimizationSelectionIdentity,
    input_semantic: TerminalPsiIdentity,
    input_proof: ProofBundleFingerprint,
    output_semantic: TerminalPsiIdentity,
    output_proof: ProofBundleFingerprint,
) -> PsiOptimizationExecutionIdentity {
    let mut digest = Sha256::new();
    digest.update(EXECUTION_IDENTITY_DOMAIN);
    digest.update(selection.bytes());
    append_terminal_identity(&mut digest, input_semantic);
    digest.update(input_proof.as_bytes());
    append_terminal_identity(&mut digest, output_semantic);
    digest.update(output_proof.as_bytes());
    PsiOptimizationExecutionIdentity(digest.finalize().into())
}

fn append_terminal_identity(digest: &mut Sha256, identity: TerminalPsiIdentity) {
    digest.update(identity.vocabulary_marker.get().to_le_bytes());
    digest.update(identity.program_fingerprint.as_bytes());
}
