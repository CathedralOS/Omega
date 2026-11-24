//! Logical parameters carry implications, not executable machine selections.
//! Their applications retain the exact fixed subjects and premise obligations
//! even though they add no runtime call, storage access or resource authority.

use crate::{CertificateEnvelope, ValueDeclaration};
use semantic_vocabulary::{
    BlockId, EdgeId, InvocationSite, MachineId, ObligationId, Proposition, ScalarTerm,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremContract {
    pub parameters: Vec<ValueDeclaration>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<Proposition>,
}

/// The proof position indexes the owning machine's erased proof formals.
/// The formal itself retains its position in the complete source header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremParameter {
    pub machine: MachineId,
    pub proof_position: u32,
    pub contract: TheoremContract,
}

/// One proof application continues on its own ordinary control-flow edge.
/// Its conclusions are available only in the successor, after every supplied
/// premise has been checked on that edge under the captured substitution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremApplication {
    pub machine: MachineId,
    pub edge: EdgeId,
    pub successor: BlockId,
    pub proof_position: u32,
    pub arguments: Vec<ScalarTerm>,
    pub requirements: Vec<ObligationId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TheoremSource {
    Named(MachineId),
    Formal(u32),
}

/// A named citation owes its declared premises over all alias subjects.
/// The source checker discharges the derivations over the complete contract.
/// Each retained judgment stays associated with its source and invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremCitation {
    pub machine: MachineId,
    pub site: InvocationSite,
    pub proof_position: u32,
    pub source: TheoremSource,
    pub derivations: Vec<CertificateEnvelope>,
}
