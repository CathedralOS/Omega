//! Optimizer module role: stage group. Closed inventory and explicit foreign-row policy replay.

use omega_effects::{
    TerminalAuthorityClass, TerminalAuthorityDisposition, TerminalMechanismIdentity,
};

use super::*;

mod checked_physical_rows;
mod foreign_rows;
mod inventory;

fn foreign_mechanism(
    candidate: omega_target::ForeignLocatorCandidate,
    target: omega_target::TargetProfile,
    contract_byte: u8,
) -> TerminalMechanismIdentity {
    let locator = omega_target::normalize_foreign_locator(candidate, target)
        .expect("test locator must normalize");
    omega_effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
        &locator,
        omega_effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
            [contract_byte; 32],
        ),
    )
    .into()
}

fn row(
    mechanism: TerminalMechanismIdentity,
    classes: impl IntoIterator<Item = TerminalAuthorityClass>,
) -> TerminalAuthorityPolicyRow {
    TerminalAuthorityPolicyRow::new(
        mechanism,
        TerminalAuthorityDisposition::from_classes(classes),
    )
}
