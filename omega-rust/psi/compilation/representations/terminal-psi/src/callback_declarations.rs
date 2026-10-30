//! Complete checked nominal-use declarations, independent of emitted calls.
//!
//! This inventory precedes target policy and executable lowering. Its arena
//! coordinates belong to one authenticated declaration product. Unavailable
//! declarations retain their obligations; they never authorize a substitute.
//! It is not the operation-bound callback placement roster or a wire schema.

use crate::{CheckedMachineDeclaration, NominalDeclarationIdentity};
use arena::{Arena, Handle};
use language_semantics::callback_declarations::{
    CallbackDeclaration, CallbackRegistrationDeclaration,
};

pub type CallbackUseId = Handle<CallbackUseDeclaration>;
pub type CallbackRequirementId = Handle<CallbackRequirementDeclaration>;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackDeclarationRoster {
    pub uses: Arena<CallbackUseDeclaration>,
    pub requirements: Arena<CallbackRequirementDeclaration>,
}

/// Exact owner/member coordinates. The callable is independent of a selected
/// callback body's stronger contract. Invocation-local identities retain the
/// identity table's private scope and cannot become portable package owners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackRequirementDeclaration {
    pub owner: Result<NominalDeclarationIdentity, String>,
    pub member: Result<NominalDeclarationIdentity, String>,
    pub callable: Result<String, String>,
    pub is_boundary: bool,
}

impl Default for CallbackRequirementDeclaration {
    fn default() -> Self {
        Self {
            owner: Err("callback requirement has no declaration owner".into()),
            member: Err("callback requirement has no declaration member".into()),
            callable: Err("callback requirement has no callable identity".into()),
            is_boundary: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackContractEnvelope {
    pub report_fingerprint: u64,
    pub commitment: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CallbackContractRefinement {
    pub published: CallbackContractEnvelope,
    pub actual: CallbackContractEnvelope,
}

/// Resource reports remain reports, not a native entry grant. The containing
/// use retains its exact selected declaration (or explicit unavailability),
/// published requirement and independently checked refinement endpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackEntryResources {
    pub machine: Result<NominalDeclarationIdentity, String>,
    pub entry: Result<NominalDeclarationIdentity, String>,
    pub contract: CallbackContractEnvelope,
    pub stack_report_fingerprint: u64,
    pub logical_work_report_fingerprint: u64,
    pub machine_state_report_fingerprint: u64,
    pub envelope_report_fingerprint: u64,
}

impl Default for CallbackEntryResources {
    fn default() -> Self {
        Self {
            machine: Err("callback entry has no machine declaration".into()),
            entry: Err("callback entry has no entry declaration".into()),
            contract: CallbackContractEnvelope::default(),
            stack_report_fingerprint: 0,
            logical_work_report_fingerprint: 0,
            machine_state_report_fingerprint: 0,
            envelope_report_fingerprint: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallbackUseDeclaration {
    pub application_index: u32,
    pub static_machine_ordinal: u32,
    pub registration: Result<CallbackRegistrationDeclaration, String>,
    pub registration_declaration: Result<NominalDeclarationIdentity, String>,
    pub requirement: CallbackRequirementId,
    pub requirement_overload: String,
    pub published: CallbackContractEnvelope,
    pub actual: CallbackContractEnvelope,
    pub refinement: CallbackContractRefinement,
    pub entry: Option<CallbackEntryResources>,
    pub selected_declaration: Result<CheckedMachineDeclaration, String>,
    pub selected_entry: Result<NominalDeclarationIdentity, String>,
}

impl Default for CallbackUseDeclaration {
    fn default() -> Self {
        Self {
            application_index: 0,
            static_machine_ordinal: 0,
            registration: Err("callback use has no registration declaration".into()),
            registration_declaration: Err("callback use has no registration owner".into()),
            requirement: Handle::default(),
            requirement_overload: String::new(),
            published: CallbackContractEnvelope::default(),
            actual: CallbackContractEnvelope::default(),
            refinement: CallbackContractRefinement::default(),
            entry: None,
            selected_declaration: Err("callback use has no selected declaration".into()),
            selected_entry: Err("callback use has no selected entry".into()),
        }
    }
}

impl CallbackDeclarationRoster {
    /// Borrow the checked classification, without source queries or target
    /// selection. Body availability cannot erase entry/calling-plan obligations.
    pub fn declaration<'a>(&self, usage: &'a CallbackUseDeclaration) -> CallbackDeclaration<'a> {
        CallbackDeclaration {
            registration: usage.registration.clone(),
            satisfaction_is_boundary: self.requirements.get(usage.requirement).is_boundary,
            has_checked_entry: usage.entry.is_some(),
            requirement_overload: &usage.requirement_overload,
        }
    }
}
