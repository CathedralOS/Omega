//! Consumer-owned promotion from fresh reconstruction to accepted evidence.

mod evidence;

pub use evidence::{
    ACCEPTED_ORDINARY_EVIDENCE_SCHEMA_VERSION, AcceptedOrdinaryClosureEvidence,
    AcceptedOrdinaryEvidenceError, AcceptedOrdinaryEvidenceSchemaIdentity,
    AcceptedOrdinaryPackageEvidence, accept_ordinary_closure_evidence,
};
