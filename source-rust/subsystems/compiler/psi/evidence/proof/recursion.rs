use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::ContractId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRankingRelation {
    StructuralSubterm,
    StructuralSubtermCycles,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveMember {
    pub contract: ContractId,
    pub machine_identity: TerminalIdentityIndex,
    pub rank_parameter_identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveField {
    pub identity: TerminalIdentityIndex,
    pub type_identity: TerminalIdentityIndex,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveType {
    pub identity: TerminalIdentityIndex,
    pub fields: Vec<TerminalProofRecursiveField>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRecursiveTransitionLane {
    Target,
    Continuation,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalProofRecursiveCallSite {
    Statement {
        state_identity: TerminalIdentityIndex,
        statement_index: u64,
    },
    Expression {
        state_identity: TerminalIdentityIndex,
        statement_index: u64,
        expression_ordinal: u64,
    },
    Transition {
        state_identity: TerminalIdentityIndex,
        statement_index: u64,
        lane: TerminalProofRecursiveTransitionLane,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveEdge {
    pub caller: ContractId,
    pub callee: ContractId,
    pub site: TerminalProofRecursiveCallSite,
    pub strict_member_path: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalProofRecursiveComponent {
    pub ranking_relation: TerminalProofRankingRelation,
    pub rank_type_identity: TerminalIdentityIndex,
    pub types: Vec<TerminalProofRecursiveType>,
    pub members: Vec<TerminalProofRecursiveMember>,
    pub edges: Vec<TerminalProofRecursiveEdge>,
}
