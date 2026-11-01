//! The terminal control-flow envelope: each terminator kind behind its tag.
//!
//! Retired tags stay unassigned: 5, 7 and 8 were split Unit cleanup
//! envelopes (now 13), 6 was the ID-only structural return (now 18, which
//! retains typed claim paths), and the ordered envelope never reinterprets
//! retired split-list tags. External completion has its own tag so a Unit
//! call cannot acquire terminal semantics by changing its declaration alone.

use super::super::CodecError;
use super::super::contract_wire::{CrashPredicate, Successor};
use super::super::wire::{Reader, Writer};
use super::super::wire_fields::{
    Arguments, ClaimBinding, ClaimTransfers, CleanupAction, CrashRoutes, Field, Id, Ids, List,
    Optional, Path, ProofTerms, ScalarTerms, U32, wire_enum, wire_record,
};
use terminal_psi::{ExternalCompletionKind, StructuralCaseSuccessorEdge, Terminator};

wire_record! {
    CaseSuccessor: StructuralCaseSuccessorEdge {
        edge: Id,
        target: Id,
        case: Id,
        payload_fields: Ids,
        structural_arguments: Arguments,
        cleanup_actions: List<CleanupAction>,
    }
}

wire_enum! {
    TerminatorRow: Terminator, "Terminator" {
        19 => InvokeNoNormal {
            edge: Id,
            static_reach_binding: Optional<U32>,
            callee: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            claim_transfers: ClaimTransfers,
            requirement_obligations: Ids,
            crash_continuations: CrashRoutes,
        },
        17 => ExternalCompletion {
            edge: Id,
            boundary: Id,
            kind: CompletionKind,
            arguments: Ids,
            structural_arguments: Arguments,
            requirement_obligations: Ids,
            frontier_lower_bound: Ids,
        },
        14 => Jump {
            edge: Id,
            target: Id,
            arguments: Ids,
            erased_arguments: ScalarTerms,
            erased_proof_arguments: ProofTerms,
            structural_arguments: Arguments,
            cleanup_actions: List<CleanupAction>,
        },
        2 => Return { edge: Id, value: Id, cleanup_actions: List<CleanupAction> },
        13 => ReturnUnit { edge: Id, cleanup_actions: List<CleanupAction> },
        18 => ReturnStructural {
            edge: Id,
            source: Id,
            returned_claims: List<ClaimBinding>,
            cleanup_actions: List<CleanupAction>,
        },
        15 => Conditional { condition: Id, when_true: Successor, when_false: Successor },
        16 => StructuralCase { source: Id, path: Path, cases: List<CaseSuccessor> },
        4 => Crash {
            edge: Id,
            cause: super::super::contract_wire::Cause,
            site_guard: List<CrashPredicate>,
            frontier_lower_bound: Ids,
        },
    }
    CompletionKind: ExternalCompletionKind, "ExternalCompletionKind" { 1 => ProcessExit }
}

pub(super) fn encode_terminator(
    writer: &mut Writer,
    terminator: &Terminator,
) -> Result<(), CodecError> {
    TerminatorRow::write(writer, terminator)
}

pub(super) fn decode_terminator(reader: &mut Reader<'_>) -> Result<Terminator, CodecError> {
    TerminatorRow::read(reader)
}
