//! Canonical control-flow contract and crash-route wire format.
//!
//! This module owns successor-edge envelopes, machine contracts, ordered crash
//! buckets/guards, and crash-predicate envelopes. Recursive proposition bytes
//! remain in the parent codec.

use terminal_psi::{
    ContractClause, CrashCause, CrashPredicateTerm, CrashRouteBucket, CrashRouteGuard,
    MachineContract, OutcomeSpecificEnsure, OutcomeSpecificEvidence, OutcomeSpecificGuard,
    SuccessorEdge,
};

use super::machine_wire::Declaration;
use super::wire::{Reader, Writer};
use super::wire_fields::{
    ArgumentSubstitution, Arguments, CleanupAction, CrashRoutes, Field, Id, Ids, List, Optional,
    ProofTerms, Proposition, ScalarTerms, Text, U32, delegated_fields, wire_enum, wire_record,
};
use super::{CodecError, decode_proposition, encode_proposition};

wire_record! {
    Successor: SuccessorEdge {
        edge: Id,
        target: Id,
        arguments: Ids,
        erased_arguments: ScalarTerms,
        erased_proof_arguments: ProofTerms,
        structural_arguments: Arguments,
        cleanup_actions: List<CleanupAction>,
    }
    Contract: MachineContract {
        id: Id,
        crash_routes: CrashRoutes,
        erased_scalar_formals: List<Declaration>,
        erased_proof_formals: ProofFormals,
        requires: List<Proposition>,
        source_checked_requires: List<Proposition>,
        ensures: List<Clause>,
        outcome_specific_ensures: List<OutcomeEnsure>,
    }
    Clause: ContractClause { obligation: Id, proposition: Proposition }
    OutcomeEnsure: OutcomeSpecificEnsure {
        guard: Guard,
        position: U32,
        obligation: Id,
        proposition: Proposition,
        evidence: Optional<Evidence>,
    }
    Guard: OutcomeSpecificGuard { result_type: Id, result_case: Id }
    Evidence: OutcomeSpecificEvidence {
        term: Id,
        output_field: Text,
        argument_subjects: List<ArgumentSubstitution>,
    }
    Bucket: CrashRouteBucket { cause: Cause, alternatives: List<RouteGuard> }
}

wire_enum! {
    Cause: CrashCause, "CrashCause" { 1 => Trap, 2 => Abort }
    RouteGuard: CrashRouteGuard, "CrashRouteGuard" {
        0 => Truth,
        1 => Predicate(predicate: CrashPredicate),
    }
}

delegated_fields! {
    ProofFormals: Vec<terminal_psi::ErasedProofFormal>
        => super::proof_term_wire::encode_proof_formals,
           super::proof_term_wire::decode_proof_formals;
}

/// A crash predicate is one top-level proposition.
pub(super) struct CrashPredicate;
impl Field<CrashPredicateTerm> for CrashPredicate {
    fn write(writer: &mut Writer, value: &CrashPredicateTerm) -> Result<(), CodecError> {
        encode_proposition(writer, value.proposition(), 0)
    }
    fn read(reader: &mut Reader<'_>) -> Result<CrashPredicateTerm, CodecError> {
        Ok(CrashPredicateTerm::new(decode_proposition(reader, 0)?))
    }
}
