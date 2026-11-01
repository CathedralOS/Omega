//! Proof recursive components on the wire.

use crate::sections::semantic_module::wire_fields::{
    Id, Index, List, Strings, U64, wire_enum, wire_record,
};
use terminal_psi::{
    TerminalProofRankingRelation, TerminalProofRecursiveCallSite, TerminalProofRecursiveComponent,
    TerminalProofRecursiveEdge, TerminalProofRecursiveField, TerminalProofRecursiveMember,
    TerminalProofRecursiveTransitionLane, TerminalProofRecursiveType,
};

wire_record! {
    Component: TerminalProofRecursiveComponent {
        ranking_relation: Ranking,
        rank_type_identity: Index,
        types: List<RecursiveType>,
        members: List<Member>,
        edges: List<Edge>,
    }
    RecursiveType: TerminalProofRecursiveType { identity: Index, fields: List<RecursiveField> }
    RecursiveField: TerminalProofRecursiveField { identity: Index, type_identity: Index }
    Member: TerminalProofRecursiveMember {
        contract: Id,
        machine_identity: Index,
        rank_parameter_identity: Index,
    }
    Edge: TerminalProofRecursiveEdge {
        caller: Id,
        callee: Id,
        site: CallSite,
        strict_member_path: Strings,
    }
}

wire_enum! {
    Ranking: TerminalProofRankingRelation, "TerminalProofRankingRelation" {
        1 => StructuralSubterm,
        2 => StructuralSubtermCycles,
    }
    CallSite: TerminalProofRecursiveCallSite, "TerminalProofRecursiveCallSite" {
        1 => Statement { state_identity: Index, statement_index: U64 },
        2 => Expression { state_identity: Index, statement_index: U64, expression_ordinal: U64 },
        3 => Transition { state_identity: Index, statement_index: U64, lane: Lane },
    }
    Lane: TerminalProofRecursiveTransitionLane, "TerminalProofRecursiveTransitionLane" {
        1 => Target,
        2 => Continuation,
    }
}
