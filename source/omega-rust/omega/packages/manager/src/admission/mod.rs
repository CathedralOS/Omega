//! Consumer-owned promotion from fresh reconstruction to accepted evidence.

mod evidence;
mod terminal_authority;

pub use evidence::{
    ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION, AcceptedOrdinaryClosureEvidence,
    AcceptedOrdinaryEvidenceError, AcceptedOrdinaryEvidenceSchemaIdentity,
    AcceptedOrdinaryPackageEvidence, accept_ordinary_closure_evidence,
};
pub use terminal_authority::{
    AcceptedTerminalAuthorityPermissionPolicyError, accepted_terminal_authority_permission_policy,
    realize_accepted_terminal_artifact_with_source_evaluated_imports_and_policy,
};
