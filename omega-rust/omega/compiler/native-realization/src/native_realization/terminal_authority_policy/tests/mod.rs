//! Optimizer module role: stage group. Closed inventory and explicit foreign-row policy replay.

use effects::{TerminalAuthorityClass, TerminalAuthorityDisposition, TerminalMechanismIdentity};

use super::*;

mod checked_physical_rows;
mod foreign_rows;
mod inventory;
mod syscall_rows;

fn foreign_mechanism(
    candidate: target::ForeignLocatorCandidate,
    target: target::TargetProfile,
    contract_byte: u8,
) -> TerminalMechanismIdentity {
    let locator =
        target::normalize_foreign_locator(candidate, target).expect("test locator must normalize");
    effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
        &locator,
        effects::provider_plan::BoundaryCallingPlanCommitment::from_digest([contract_byte; 32]),
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
