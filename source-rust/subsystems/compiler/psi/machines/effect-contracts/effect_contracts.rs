//! Effects that a Psi program retains over explicit parameter positions.
#![forbid(unsafe_code)]
mod carry;
mod commitment;

pub use carry::{
    PermissionArrival, PermissionClaimIdentity as ScopedPermissionClaimIdentity,
    PermissionEventSource as ScopedPermissionEventSource,
    PermissionProvenance as ScopedPermissionProvenance, SuspensionValueOrigin,
};
pub use commitment::{template_contract_commitment, validate_published_dependency};

/// A capability use retained in a callable behavior policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
pub enum CapabilityFlowKind {
    #[default]
    Uses,
    Returns,
    Acquires,
    Stores,
    Derives,
}

impl CapabilityFlowKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uses => "uses",
            Self::Returns => "returns",
            Self::Acquires => "acquires",
            Self::Stores => "stores",
            Self::Derives => "derives",
        }
    }
}

// A helper adds its own effects and those of its exact callback
// parameters. The same expression is built with local vectors, retained
// in checked arenas, and normalized to binder positions for publication.
// Reach and operational effects differ only in the fixed value's type.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
)]
pub struct ContractDependency<Contribution, Parameters> {
    pub concrete: Contribution,
    pub parameters: Parameters,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, record_storage::Record)]
// Keep the same live subject and acquisition when a helper is selected.
// Each row requires suspension permission under its condition; the row
// grants no permission and changes none of the other carry obligations.
pub struct ConditionalCarryRequirement {
    pub state: u32,
    pub statement_index: u64,
    pub call_ordinal: u64,
    pub subject: crate::SuspensionValueOrigin<String>,
    pub value_type: String,
    pub claims: Vec<crate::PermissionArrival<String>>,
    pub condition: ContractDependency<bool, Vec<u32>>,
}

impl<Parameters: Default> From<bool> for ContractDependency<bool, Parameters> {
    fn from(concrete: bool) -> Self {
        Self {
            concrete,
            parameters: Parameters::default(),
        }
    }
}

impl<Parameter> ContractDependency<bool, Vec<Parameter>> {
    pub fn parameter(parameter: Parameter, possible: bool) -> Self {
        Self {
            concrete: false,
            parameters: possible.then_some(parameter).into_iter().collect(),
        }
    }

    pub fn possible(&self) -> bool {
        self.concrete || !self.parameters.is_empty()
    }
}

#[cfg(test)]
mod tests;
