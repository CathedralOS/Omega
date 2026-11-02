//! Checked callable descriptions survive independently of executable selection.
//!
//! Pruning may remove a body without removing a condition owned by its
//! declaration. These rows retain the same signature and commitments used for
//! emitted declarations; retaining a row never requests that body's execution.

use arena::{Arena, HandleSpan};
use symbols::SymbolHandle;
use typed_trees::type_identity::{CallableIdentityId, TypeIdentityId};

use super::MachineContractCommitment;

/// Independently decidable callable facts, even when its full description is
/// unavailable. These describe source supply, never executable installation.
#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallableHeader {
    pub machine: SymbolHandle,
    pub callable: CallableIdentityId,
    pub package: Option<semantic_vocabulary::PackageKeyIdentity>,
    pub source_target: Option<String>,
    pub product_eligible: bool,
    pub supplies_checked_body: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationParameter {
    pub normalized: TypeIdentityId,
    pub qualified: TypeIdentityId,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationCapture {
    pub template_identity: TypeIdentityId,
    pub template_contract: [u8; 32],
    pub specialization: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationSpecialization {
    pub template: SymbolHandle,
    pub callable: CallableIdentityId,
    pub commitment: [u8; 32],
    pub capture: Option<CheckedDeclarationCapture>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedMachineDescription {
    pub machine: SymbolHandle,
    pub entry: SymbolHandle,
    pub parameters: HandleSpan<CheckedDeclarationParameter>,
    pub result: TypeIdentityId,
    pub callable: CallableIdentityId,
    pub contract: MachineContractCommitment,
    pub specialization: Option<CheckedDeclarationSpecialization>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub enum CheckedDeclarationDescription {
    #[default]
    Missing,
    Machine(CheckedMachineDescription),
    /// A symbolic or otherwise unpublishable description must not make an
    /// unrelated product fail. Its exact refusal is reported when demanded.
    Unavailable {
        machine: SymbolHandle,
        reason: String,
    },
}

impl CheckedDeclarationDescription {
    pub fn machine(&self) -> SymbolHandle {
        match self {
            Self::Missing => SymbolHandle::default(),
            Self::Machine(description) => description.machine,
            Self::Unavailable { machine, .. } => *machine,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedDeclarationDescriptions {
    pub headers: Arena<CheckedCallableHeader>,
    pub machines: Arena<CheckedDeclarationDescription>,
    pub parameters: Arena<CheckedDeclarationParameter>,
}

impl CheckedDeclarationDescriptions {
    pub fn description(&self, machine: SymbolHandle) -> Option<&CheckedDeclarationDescription> {
        self.machines
            .iter()
            .find_map(|(_, row)| (machine.is_valid() && row.machine() == machine).then_some(row))
    }

    pub fn for_machine(&self, machine: SymbolHandle) -> Result<&CheckedMachineDescription, &str> {
        match self.description(machine) {
            Some(CheckedDeclarationDescription::Machine(description)) => Ok(description),
            Some(CheckedDeclarationDescription::Unavailable { reason, .. }) => Err(reason),
            _ => Err("checked machine lost its retained declaration description"),
        }
    }
}
