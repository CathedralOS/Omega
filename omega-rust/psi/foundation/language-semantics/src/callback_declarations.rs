//! Source declaration facts for nominal callback root admission.
//!
//! The checked use owns refinement and entry-resource receipts. This borrowed
//! view carries their presence and exact declaration classification; it neither
//! selects a provider nor grants native placement.

use semantic_vocabulary::{CallbackRequirementId, NativeParameterId, StaticMachineBinderId};

/// An ordinary checked invocation, or an exact callable needing provider selection.
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

/// Capacity of the current normalized calling-policy input vocabulary, not
/// source machine arity or a target's argument-register count. Unsupported
/// boundary projections retain a refusal until an ABI application is requested.
pub const BOUNDARY_NATIVE_PARAMETER_CAPACITY: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryTelescope {
    pub parameters: Vec<BoundaryRuntimeFormal>,
    pub binders: Vec<BoundaryCallbackBinderDeclaration>,
    pub direct_callbacks: Vec<BoundaryDirectCallbackDeclaration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryRuntimeFormal {
    pub identity: semantic_vocabulary::NativeParameterId,
    pub native_ordinal: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryCallbackBinderDeclaration {
    pub binder: semantic_vocabulary::StaticMachineBinderId,
    pub requirement: semantic_vocabulary::CallbackRequirementId,
    /// Counts all static machine binders, including structural ones which do
    /// not name a nominal callback requirement. Gaps are meaningful.
    pub static_machine_ordinal: u32,
    pub requirement_identity: String,
    pub invokes_synchronously: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
