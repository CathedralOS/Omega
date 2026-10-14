//! Optimizer module role: identity leaf. Complete policy-table commitment.

use effects::{
    TerminalAuthorityDisposition, TerminalMechanismIdentity, terminal_mechanism_identity_bytes,
};

use super::{
    TERMINAL_AUTHORITY_POLICY_VERSION,
    classification::classify_compiler_intrinsic,
    inventory::{CLOSED_POLICY_ROW_COUNT, committed_policy_mechanisms},
    model::TerminalAuthorityPolicyRow,
};

const POLICY_COMMITMENT_DOMAIN: &[u8] = b"omega.terminal-authority.policy.v3\0";

pub(super) fn complete_policy_commitment(explicit_rows: &[TerminalAuthorityPolicyRow]) -> [u8; 32] {
    let mut hasher = crate::digest_encoder::DomainDigestEncoder::new(POLICY_COMMITMENT_DOMAIN);
    hasher.raw(&TERMINAL_AUTHORITY_POLICY_VERSION.to_be_bytes());
    hasher.raw(
        &(CLOSED_POLICY_ROW_COUNT
            + super::parameterized_intrinsics::PARAMETERIZED_POLICY_FAMILY_COUNT
            + u32::try_from(explicit_rows.len()).expect("policy row count fits u32"))
        .to_be_bytes(),
    );
    for &mechanism in committed_policy_mechanisms() {
        encode_mechanism(&mut hasher, mechanism.into());
        encode_disposition(&mut hasher, &classify_compiler_intrinsic(mechanism));
    }
    super::parameterized_intrinsics::encode_commitment(&mut hasher);
    for row in explicit_rows {
        encode_mechanism(&mut hasher, row.mechanism);
        encode_disposition(&mut hasher, &row.disposition);
    }
    hasher.finish()
}

fn encode_disposition(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    disposition: &TerminalAuthorityDisposition,
) {
    hasher.raw(
        &u32::try_from(disposition.classes().len())
            .expect("terminal-authority class count fits u32")
            .to_be_bytes(),
    );
    for class in disposition.classes() {
        hasher.byte(class.canonical_tag());
    }
}

fn encode_mechanism(
    hasher: &mut crate::digest_encoder::DomainDigestEncoder,
    mechanism: TerminalMechanismIdentity,
) {
    let bytes = terminal_mechanism_identity_bytes(mechanism);
    hasher.raw(
        &u32::try_from(bytes.len())
            .expect("terminal-mechanism identity length fits u32")
            .to_be_bytes(),
    );
    hasher.raw(&bytes);
}
