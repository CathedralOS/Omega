//! The checked machine contract embedded in a callable descriptor.
use crate::signatures::{
    ArgumentSubstitution, Claim, Declaration, MachineParameterKind as ParameterKind, MachineResult,
};
use crate::{
    CodecError,
    fields::*,
    wire::{Reader, Writer, decode_counted},
};
use semantic_vocabulary::Termination as TerminationPromise;
use terminal_psi::{
    ContractClause, ErasedProofFormal, MachineContract, OutcomeSpecificEnsure,
    OutcomeSpecificEvidence, OutcomeSpecificGuard, RuntimeDomainMembership, TerminalIdentityIndex,
};
type CrashRoutes = List<psi_declaration_encoding::crash_routes::Bucket>;
wire_record! {
    Contract: MachineContract {
        id: Id,
        declaration: Optional<MachineDeclaration>,
        selected_application: Optional<crate::signatures::Dispatch>,
        parameters: List<Declaration>,
        parameter_order: List<ParameterKind>,
        structural_parameters: List<crate::signatures::StructuralParameter>,
        result: MachineResult,
        result_lifetimes: Optional<List<crate::signatures::ResultLifetime>>,
        entry_claims: List<Claim>,
        declared_service_reach: Optional<Ids>,
        declared_synchronous_entries: Optional<Ids>,
        declared_crash_routes: Optional<CrashRoutes>,
        declared_suspension: Optional<Flag>,
        declared_blocking: Optional<Flag>,
        published_may_suspend: Flag,
        published_may_block: Flag,
        closed_reach_application: ReachApplication,
        published_service_ceiling: Ids,
        published_synchronous_entries: Ids,
        published_writes: Optional<List<(U32, Ids)>>,
        termination: Termination,
        content_entry_claims: List<psi_declaration_encoding::terms::content::EntryClaim>,
        crash_routes: CrashRoutes,
        erased_scalar_formals: List<Declaration>,
        erased_proof_formals: ProofFormals,
        membership_ensures: List<Membership>,
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
}
wire_enum! {
    Membership: RuntimeDomainMembership, "RuntimeDomainMembership" {
        1 => Scalar { subject: RuntimeScalarTerm, domain: Id },
        2 => Structural {
            root: Id, path: psi_declaration_encoding::structural_fields::FieldPath, version: psi_declaration_encoding::terms::content::PlaceVersion, domain: Id
        },
    }
    Termination: TerminationPromise, "termination guarantee" {
        0 => NoGuarantee,
        1 => Terminates,
    }
}
pub fn encode_proof_formals(
    writer: &mut Writer,
    formals: &[ErasedProofFormal],
) -> Result<(), CodecError> {
    writer.len("erased proof formals", formals.len())?;
    for formal in formals {
        writer.u32(formal.source_position);
        writer.u32(formal.type_identity.0);
    }
    Ok(())
}

pub fn decode_proof_formals(reader: &mut Reader<'_>) -> Result<Vec<ErasedProofFormal>, CodecError> {
    decode_counted(reader, |reader| {
        let source_position = reader.u32()?;
        let type_identity = TerminalIdentityIndex(reader.u32()?);
        if type_identity.is_absent() {
            return Err(CodecError::MalformedStructuralFoundation(
                "empty erased proof formal type",
            ));
        }
        Ok(ErasedProofFormal {
            source_position,
            type_identity,
        })
    })
}

delegated_fields! {
    MachineDeclaration: terminal_psi::CheckedMachineDeclaration
        => crate::machine_declarations::encode, crate::machine_declarations::decode;
    RuntimeScalarTerm: semantic_vocabulary::ScalarTerm
        => psi_declaration_encoding::terms::scalar::encode_scalar_term,
           psi_declaration_encoding::terms::scalar::decode_scalar_term;
    ProofFormals: Vec<ErasedProofFormal> => encode_proof_formals, decode_proof_formals;
}

pub struct ReachApplication;
impl Field<Option<terminal_psi::ClosedReachApplication>> for ReachApplication {
    fn write(
        writer: &mut Writer,
        value: &Option<terminal_psi::ClosedReachApplication>,
    ) -> Result<(), CodecError> {
        crate::reach::encode(writer, value.as_ref())
    }
    fn read(
        reader: &mut Reader<'_>,
    ) -> Result<Option<terminal_psi::ClosedReachApplication>, CodecError> {
        crate::reach::decode(reader)
    }
}
