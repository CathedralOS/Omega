//! Optimizer module role: carrier leaf. Receiving-policy rows, errors, and classification API.

use omega_effects::{
    CompilerIntrinsicExecutionIdentity, TerminalAuthorityDisposition,
    TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity,
};

use super::{classification::classify_from_inventory, inventory::committed_policy_mechanisms};

/// One explicit receiving-policy row outside the compiler-owned intrinsic inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPolicyRow {
    pub(super) mechanism: TerminalMechanismIdentity,
    pub(super) disposition: TerminalAuthorityDisposition,
}

impl TerminalAuthorityPolicyRow {
    pub fn new(
        mechanism: TerminalMechanismIdentity,
        disposition: TerminalAuthorityDisposition,
    ) -> Self {
        Self {
            mechanism,
            disposition,
        }
    }

    pub const fn mechanism(&self) -> TerminalMechanismIdentity {
        self.mechanism
    }

    pub const fn disposition(&self) -> &TerminalAuthorityDisposition {
        &self.disposition
    }
}

/// Current receiving-realization policy over the closed compiler inventory and explicit foreign rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalAuthorityPolicy {
    identity: TerminalAuthorityPolicyIdentity,
    explicit_rows: Vec<TerminalAuthorityPolicyRow>,
}

impl TerminalAuthorityPolicy {
    pub(super) const fn new(
        identity: TerminalAuthorityPolicyIdentity,
        explicit_rows: Vec<TerminalAuthorityPolicyRow>,
    ) -> Self {
        Self {
            identity,
            explicit_rows,
        }
    }

    pub const fn identity(&self) -> TerminalAuthorityPolicyIdentity {
        self.identity
    }

    pub fn explicit_rows(&self) -> &[TerminalAuthorityPolicyRow] {
        &self.explicit_rows
    }

    /// Classify one exact role-tagged mechanism. Compiler intrinsics use the
    /// exhaustive closed inventory; all other roles require one exact row.
    pub fn classify(
        &self,
        mechanism: impl Into<TerminalMechanismIdentity>,
    ) -> Result<TerminalAuthorityDisposition, UnclassifiedTerminalMechanism> {
        let mechanism = mechanism.into();
        match mechanism {
            TerminalMechanismIdentity::CompilerIntrinsic(intrinsic) => {
                classify_from_inventory(committed_policy_mechanisms(), intrinsic)
            }
            TerminalMechanismIdentity::NormalizedForeign(_) => self
                .explicit_rows
                .iter()
                .find(|row| row.mechanism == mechanism)
                .map(|row| row.disposition.clone())
                .ok_or(UnclassifiedTerminalMechanism { mechanism }),
        }
    }
}

/// The requested closed mechanism has no row in this exact policy version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnclassifiedTerminalMechanism {
    pub(super) mechanism: TerminalMechanismIdentity,
}

impl UnclassifiedTerminalMechanism {
    pub const fn mechanism(self) -> TerminalMechanismIdentity {
        self.mechanism
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalAuthorityPolicyBuildError {
    CompilerIntrinsicRowIsReserved(CompilerIntrinsicExecutionIdentity),
    EmptyImplementationContract(TerminalMechanismIdentity),
    DuplicateMechanism(TerminalMechanismIdentity),
}
