//! Exact in-memory ordinary evidence after fresh reconstruction and project-policy comparison.

mod assembly;
mod model;
mod validation;

pub use assembly::accept_ordinary_closure_evidence;
pub use model::{
    ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION, AcceptedOrdinaryClosureEvidence,
    AcceptedOrdinaryEvidenceError, AcceptedOrdinaryEvidenceSchemaIdentity,
    AcceptedOrdinaryPackageEvidence,
};
