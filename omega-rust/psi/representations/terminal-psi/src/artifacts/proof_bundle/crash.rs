//! Producer-supplied certificates for reconstructed crash obligations.
//!
//! A crash obligation is a question the verifier reconstructs from the module
//! alone: an asserted `Crash` terminator guard against every independently
//! reconstructed path into its site, or one uncovered call continuation's
//! coverage/refutation goals under invocation-entry requirements. The rows
//! here carry the producer's answer — which denotation lane each proof node
//! was built under and the node itself — keyed by the exact reconstructed
//! question owner. Verification looks rows up by owner and re-decides every
//! supplied node; it never searches and never trusts the key ordering.

use semantic_vocabulary::{BlockId, EdgeId, MachineId, OperationId};

use crate::CrashCause;
use crate::artifacts::proof_bundle::ProofNode;

/// The exact crash question a certificate roster answers.
///
/// `Site` names one `Crash` terminator by its machine, block, and outgoing
/// edge. `Continuation` names one uncovered same-cause continuation bucket at
/// one call or crash-contract operation. Reconstruction derives every owner
/// from the module; a bundle row naming anything else is unknown evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashObligationOwner {
    Site {
        machine: MachineId,
        block: BlockId,
        edge: EdgeId,
    },
    Continuation {
        machine: MachineId,
        operation: OperationId,
        cause: CrashCause,
    },
}

/// One producer-supplied certificate for one crash goal: which denotation
/// lane the producing search ran under and the proof node it emitted. The
/// accepting side replays only the recorded conversion and re-decides the
/// node against the goal, requirements, and axioms it reconstructed itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashCertificate {
    /// Whether the producing search ran under contextual value-equality
    /// transport. The same flag selects the identical conversion at check
    /// time; a node cannot change lanes after production.
    pub with_value_equalities: bool,
    pub proof: ProofNode,
}

/// The certificate roster answering one reconstructed crash obligation.
///
/// Slot arity is part of the canonical row shape and is checked against the
/// reconstructed question:
///
/// - For a `Site` owner, `coverage` has one roster per asserted `site_guard`
///   predicate in guard order; every roster is replayed under each
///   reconstructed path's axiom list. `refutation` has one roster per
///   reconstructed path into the site, in path order; a roster proves
///   `Falsehood` under its path's axioms to discharge that path as
///   unreachable. An absent or failed infeasibility certificate never removes
///   a path — the guard must then be proved there.
/// - For a `Continuation` owner, `coverage` has one roster per reconstructed
///   same-cause published-bucket coverage goal, and `refutation` is empty
///   when the uncovered roster forms no complement goal or carries exactly
///   one roster proving that goal. Either lane discharging the question
///   accepts the continuation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashObligationEvidence {
    pub owner: CrashObligationOwner,
    /// Per-goal proving rosters for the question's positive goals.
    pub coverage: Vec<Vec<CrashCertificate>>,
    /// Per-path infeasibility rosters (sites) or the single complement-goal
    /// roster (continuations).
    pub refutation: Vec<Vec<CrashCertificate>>,
}
