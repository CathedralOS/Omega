//! Source declaration facts for nominal callback root admission.
//!
//! The checked use owns refinement and entry-resource receipts. This borrowed
//! view carries their presence and exact declaration classification; it neither
//! selects a provider nor grants native placement.

/// An ordinary checked invocation, or an exact callable needing provider selection.
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
