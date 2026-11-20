use semantic_vocabulary::{CallbackRequirementId, NativeParameterId, StaticMachineBinderId};

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct PrivateCallbackSlotSelection {
    pub declaration_identity: String,
    pub trait_identity: String,
    pub semantic_identity: String,
    pub layout_identity: String,
    pub requirement_identity: String,
    pub application_report_fingerprint: u64,
    pub application_commitment: [u8; 32],
    pub application_encoding: std::sync::Arc<[u8]>,
}

impl PrivateCallbackSlotSelection {
    pub fn has_complete_identity(&self) -> bool {
        !self.declaration_identity.is_empty()
            && !self.trait_identity.is_empty()
            && !self.semantic_identity.is_empty()
            && !self.layout_identity.is_empty()
            && !self.requirement_identity.is_empty()
            && self.application_commitment != [0; 32]
            && !self.application_encoding.is_empty()
    }

    pub fn validate_identity_encoding(&self) -> Result<(), String> {
        if !self.has_complete_identity() {
            return Err(
                "private callback slot lost its exact selected application identity".into(),
            );
        }
        let (report, commitment) =
            crate::conformance_identity::source_identity_commitments(&self.application_encoding);
        if report != self.application_report_fingerprint
            || commitment != self.application_commitment
        {
            return Err("private callback slot changed its Source-v4 identity preimage".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallbackRegistrationDeclaration {
    CheckedSource,
    External(String),
}

pub struct CallbackDeclaration<'usage> {
    pub registration: Result<CallbackRegistrationDeclaration, String>,
    pub satisfaction_is_boundary: bool,
    pub has_checked_entry: bool,
    pub requirement_overload: &'usage str,
}

pub const BOUNDARY_NATIVE_PARAMETER_CAPACITY: usize = 32;

pub const BOUNDARY_CALLBACK_CAPACITY: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Default, record_storage::Record)]
pub struct BoundaryTelescope {
    pub parameters: Vec<BoundaryRuntimeFormal>,
    pub binders: Vec<BoundaryCallbackBinderDeclaration>,
    pub direct_callbacks: Vec<BoundaryDirectCallbackDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct BoundaryRuntimeFormal {
    pub identity: semantic_vocabulary::NativeParameterId,
    pub native_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct BoundaryCallbackBinderDeclaration {
    pub binder: semantic_vocabulary::StaticMachineBinderId,
    pub requirement: semantic_vocabulary::CallbackRequirementId,
    pub static_machine_ordinal: u32,
    pub requirement_identity: String,
    pub invokes_synchronously: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct BoundaryDirectCallbackDeclaration {
    pub name: String,
    pub identity: semantic_vocabulary::NativeParameterId,
    pub native_ordinal: u32,
    pub binder: semantic_vocabulary::StaticMachineBinderId,
    pub requirement: semantic_vocabulary::CallbackRequirementId,
}

impl BoundaryDirectCallbackDeclaration {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub const fn identity(&self) -> NativeParameterId {
        self.identity
    }

    pub const fn native_ordinal(&self) -> u32 {
        self.native_ordinal
    }

    pub const fn binder(&self) -> StaticMachineBinderId {
        self.binder
    }

    pub const fn requirement(&self) -> CallbackRequirementId {
        self.requirement
    }
}
