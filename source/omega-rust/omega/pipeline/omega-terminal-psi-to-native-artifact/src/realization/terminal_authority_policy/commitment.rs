//! Optimizer module role: identity leaf. Complete policy-table commitment.

use omega_effects::{
    terminal_mechanism_identity_bytes, TerminalAuthorityDisposition, TerminalMechanismIdentity,
};
use sha2::{Digest, Sha256};

use super::{
    classification::classify_compiler_intrinsic,
    inventory::{committed_policy_mechanisms, CLOSED_POLICY_ROW_COUNT},
    model::TerminalAuthorityPolicyRow,
    TERMINAL_AUTHORITY_POLICY_VERSION,
};

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"omega.terminal-authority.policy.v2\0";

pub(super) fn complete_policy_commitment(explicit_rows: &[TerminalAuthorityPolicyRow]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(POLICY_COMMITMENT_DOMAIN);
    hasher.update(TERMINAL_AUTHORITY_POLICY_VERSION.to_be_bytes());
    hasher.update(
        (CLOSED_POLICY_ROW_COUNT
            + u32::try_from(explicit_rows.len()).expect("policy row count fits u32"))
        .to_be_bytes(),
    );
    for &mechanism in committed_policy_mechanisms() {
        encode_mechanism(&mut hasher, mechanism.into());
        encode_disposition(&mut hasher, &classify_compiler_intrinsic(mechanism));
    }
    for row in explicit_rows {
        encode_mechanism(&mut hasher, row.mechanism);
        encode_disposition(&mut hasher, &row.disposition);
    }
    hasher.finalize().into()
}

fn encode_disposition(hasher: &mut Sha256, disposition: &TerminalAuthorityDisposition) {
    hasher.update(
        u32::try_from(disposition.classes().len())
            .expect("terminal-authority class count fits u32")
            .to_be_bytes(),
    );
    for class in disposition.classes() {
        hasher.update([class.canonical_tag()]);
    }
}

fn encode_mechanism(hasher: &mut Sha256, mechanism: TerminalMechanismIdentity) {
    let bytes = terminal_mechanism_identity_bytes(mechanism);
    hasher.update(
        u32::try_from(bytes.len())
            .expect("terminal-mechanism identity length fits u32")
            .to_be_bytes(),
    );
    hasher.update(bytes);
}
