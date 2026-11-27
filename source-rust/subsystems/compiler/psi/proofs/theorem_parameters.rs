//! Logical parameters carry implications, not executable machine selections.
//! Their applications retain the exact fixed subjects and premise obligations
//! even though they add no runtime call, storage access or resource authority.

use crate::{CertificateEnvelope, ValueDeclaration};
use semantic_vocabulary::{
    BlockId, EdgeId, InvocationSite, MachineId, ObligationId, PlaceId, Proposition, ScalarTerm,
    StructuralTypeId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremContract {
    pub parameters: Vec<ValueDeclaration>,
    /// The contract's sum subjects, after its scalar ones. Each is one value
    /// of its sum's inductive declaration, not a tuple of case Booleans and
    /// payload scalars: mathematical_bindings.md, "A sum input in a logical
    /// contract denotes a single mathematical value of that sum's checked
    /// inductive declaration". Clauses name one by its place, as a case test
    /// (`StructuralCaseMembership`) or a payload read under a
    /// `Case(c), Field(f)` path. A record input stays flattened into
    /// `parameters`.
    pub data_subjects: Vec<TheoremDataSubject>,
    pub requires: Vec<Proposition>,
    pub ensures: Vec<Proposition>,
    /// The contract's own proof parameters, in its header order, such as
    /// `Inner` in `machine<Inner: NonzeroIsPositive>(n: u8)`. Each is a
    /// complete contract with none of its own. A proof of the whole contract
    /// receives one proof of each before it is applied, and its implication
    /// is formed over them: mathematical_bindings.md, "If the contract itself
    /// takes a proof parameter, that parameter belongs to the contract's own
    /// header. The outer caller supplies one proof of the whole contract."
    pub proof_parameters: Vec<TheoremContract>,
}

/// One sum subject of a logical contract. Its places number the contract's
/// sum subjects from 1, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TheoremDataSubject {
    pub place: PlaceId,
    pub structural_type: StructuralTypeId,
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
///
/// `source` names the implication: one of the owner's erased proof formals,
/// or a named machine that does no work, whose contract is the implication.
/// A named application is a fact-only invocation: contracts.md, "Citation
/// and induction": "Establish its `requires` under exact operand substitution
/// before importing its `ensures`; erase the call if fact-only."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremApplication {
    pub machine: MachineId,
    pub edge: EdgeId,
    pub successor: BlockId,
    pub source: TheoremSource,
    pub arguments: Vec<ScalarTerm>,
    pub requirements: Vec<ObligationId>,
    /// One proof for each of the applied contract's own proof parameters, in
    /// its header order, such as `inner` in `Step::<inner>(n)`. Each is
    /// checked against that parameter's contract as an instantiation of a
    /// cited theorem is. Empty when the contract has none.
    pub instantiation: Vec<TheoremInstantiation>,
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
    /// A named source that declares proof parameters of its own receives one
    /// supply for each, in its header order, such as `lifted::<inner>`. Each
    /// is checked against the named machine's own alias, as a citation at an
    /// invocation of that machine would be. Empty for every other source.
    pub instantiation: Vec<TheoremInstantiation>,
}

/// One proof given to a cited theorem's own proof parameter. A named supply
/// owes the same three derivations a citation does; a forwarded formal owes
/// none and must state the same implication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TheoremInstantiation {
    pub source: TheoremSource,
    pub derivations: Vec<CertificateEnvelope>,
}
