//! Exact in-memory ordinary evidence after local replay and root admission.

mod assembly;
mod model;
mod validation;

pub use assembly::accept_ordinary_closure_evidence;
pub use model::{
    ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION, AcceptedOrdinaryClosureEvidence,
    AcceptedOrdinaryEvidenceError, AcceptedOrdinaryEvidenceSchemaIdentity,
    AcceptedOrdinaryPackageEvidence,
};
