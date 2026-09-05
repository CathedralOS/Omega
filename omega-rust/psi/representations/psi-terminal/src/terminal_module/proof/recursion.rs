use psi_core::ContractId;

/// The closed proof-recursion relation vocabulary retained in Terminal Psi.
/// The verifier, not the producer, reconstructs its relation identity and
/// proof obligations from the complete component row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRankingRelation {
    StructuralSubterm,
}

/// One proof-only callable participating in a recursive component. Its
/// contract ID joins kernel recursion admission; source identities retain the
/// exact declarations without frontend arena handles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveMember {
    pub contract: ContractId,
    pub machine_identity: String,
    pub rank_parameter_identity: String,
}

/// One field in the closed finite-inductive proof-type graph used to replay a
/// structural-subterm ranking path. These rows describe proof data only; they
/// do not authorize runtime layout or projection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveField {
    pub identity: String,
    pub type_identity: String,
}

/// One nominal node in a recursive proof-data graph. The verifier requires
/// every retained strict path to resolve field-by-field in this graph and end
/// back at the component's rank type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveType {
    pub identity: String,
    pub fields: Vec<TerminalProofRecursiveField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRecursiveTransitionLane {
    Target,
    Continuation,
}

/// Exact source-independent coordinate of one internal recursive call.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRecursiveCallSite {
    Statement {
        state_identity: String,
        statement_index: u64,
    },
    Expression {
        state_identity: String,
        statement_index: u64,
        expression_ordinal: u64,
    },
    Transition {
        state_identity: String,
        statement_index: u64,
        lane: TerminalProofRecursiveTransitionLane,
    },
}

/// One exact internal edge and its strict declaration-identity path. Repeated
/// calls between the same member pair remain separate rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveEdge {
    pub caller: ContractId,
    pub callee: ContractId,
    pub site: TerminalProofRecursiveCallSite,
    pub strict_member_path: Vec<String>,
}

/// One canonical proof-only strongly connected component. Ranking and
/// well-foundedness occur once; per-edge decrease obligations are verifier-
/// reconstructed from these exact semantic rows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveComponent {
    pub ranking_relation: TerminalProofRankingRelation,
    pub rank_type_identity: String,
    pub types: Vec<TerminalProofRecursiveType>,
    pub members: Vec<TerminalProofRecursiveMember>,
    pub edges: Vec<TerminalProofRecursiveEdge>,
}
