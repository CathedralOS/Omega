//! Complete contracts, descriptors and invocation envelopes retained in native code catalogs.
use crate::CodecError;
use crate::contracts::Contract as ProofContract;
use crate::evidence::{LaneKind, Subject, Term};
use crate::fields::Field;
use crate::fields::{
    CallResult, Flag, Id, Ids, Index, List, Optional, Path, U32, wire_enum, wire_record,
};
use crate::proof_declarations::{
    Application as EvidenceApplication, Declaration as EvidenceDeclaration,
};
use crate::signatures::{Declaration, MachineResult};
use crate::signatures::{ParameterKind, StructuralParameter};
use crate::wire::{Writer, decode_canonical};
use psi_declaration_encoding::dispatch::{ClaimPath, Region};
use terminal_psi::{
    TerminalBoundaryEnvelope, TerminalCallableCatalog, TerminalCallableContract,
    TerminalCallableEnvelope, TerminalCallableEvidenceLane, TerminalCallableInvocation,
    TerminalCallableTermination, TerminalCallableTerminationPremise, TerminalDeclarationEnvelope,
    TerminalMachineDescriptor, TerminalMachineDescriptorOrigin, TerminalMachineEnvelope,
};

wire_record! {
    Catalog: TerminalCallableCatalog {
        contracts: List<Contract, 54>,
        descriptors: List<Descriptor, 38>,
        machine_envelopes: List<Optional<NamedEnvelope>>,
        boundary_envelopes: List<Optional<BoundaryEnvelope>>,
        declaration_envelopes: List<DeclarationEnvelope, 19>,
    }
    Contract: TerminalCallableContract {
        id: Id,
        identity: Index,
        scalar_parameters: List<Declaration, 17>,
        parameter_order: List<ParameterKind>,
        structural_parameters: List<StructuralParameter, 34>,
        result: CallResult,
        proof_result: MachineResult,
        entry_claim_paths: List<ClaimPath, 8>,
        result_claim_paths: List<Path, 4>,
        result_reference_regions: List<Region, 14>,
        proof: ProofContract,
        evidence_contract: List<EvidenceLane>,
        envelope: Envelope,
    }
    EvidenceLane: TerminalCallableEvidenceLane {
        kind: LaneKind,
        position: U32,
        declaration: EvidenceDeclaration,
        application: EvidenceApplication,
        term: Term,
        argument_subjects: List<Subject>,
    }
    Descriptor: TerminalMachineDescriptor {
        id: Id,
        contract: Id,
        machine: Id,
        origin: Origin,
        refinement_obligations: Ids,
        refinement_evidence: List<crate::certificates::Certificate>,
    }
    Origin: TerminalMachineDescriptorOrigin {
        application: Index,
        conformance: Optional<Index>,
        requirement: Optional<Index>,
    }
    NamedEnvelope: TerminalMachineEnvelope { machine: Id, envelope: Envelope }
    BoundaryEnvelope: TerminalBoundaryEnvelope { boundary: Id, envelope: Envelope }
    DeclarationEnvelope: TerminalDeclarationEnvelope { contract: Id, envelope: Envelope }
    Envelope: TerminalCallableEnvelope {
        service_reach: Ids,
        invocations: List<Invocation, 5>,
        suspends: Flag,
        blocks: Flag,
        termination: Termination,
    }
    Premise: TerminalCallableTerminationPremise { parameter: U32, path: Path, domain: Id }
}

wire_enum! {
    Invocation: TerminalCallableInvocation, "TerminalCallableInvocation" {
        1 => Parameter(parameter: U32),
        2 => Service(service: Id),
    }
    Termination: TerminalCallableTermination, "TerminalCallableTermination" {
        0 => NotGuaranteed,
        1 => Terminates { premises: List<Premise, 16> },
    }
}

/// Encode the complete callable contracts, selections and envelopes using
/// their semantic-module field definitions, without an enclosing module.
pub fn encode_callable_catalog(catalog: &TerminalCallableCatalog) -> Result<Vec<u8>, CodecError> {
    let mut writer = Writer::default();
    Catalog::write(&mut writer, catalog)?;
    Ok(writer.finish())
}

pub fn decode_callable_catalog(bytes: &[u8]) -> Result<TerminalCallableCatalog, CodecError> {
    decode_canonical(bytes, Catalog::read, encode_callable_catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        fields::Field,
        wire::{Reader, Writer},
    };
    use semantic_vocabulary::{ContractId, ServiceId};

    #[test]
    fn declaration_envelopes_keep_parameter_invocations_and_exact_contract_ids() {
        let catalog = TerminalCallableCatalog {
            declaration_envelopes: vec![TerminalDeclarationEnvelope {
                contract: ContractId::new(7).unwrap(),
                envelope: TerminalCallableEnvelope {
                    service_reach: vec![ServiceId::new(1).unwrap()],
                    invocations: vec![TerminalCallableInvocation::Parameter(2)],
                    suspends: false,
                    blocks: true,
                    termination: TerminalCallableTermination::NotGuaranteed,
                },
            }],
            ..Default::default()
        };
        let mut writer = Writer::default();
        Catalog::write(&mut writer, &catalog).unwrap();
        let bytes = writer.finish();
        assert_eq!(encode_callable_catalog(&catalog).unwrap(), bytes);
        assert_eq!(decode_callable_catalog(&bytes).unwrap(), catalog);
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            decode_callable_catalog(&trailing),
            Err(CodecError::TrailingBytes(1))
        ));
        let mut reader = Reader::new(&bytes);
        assert_eq!(Catalog::read(&mut reader).unwrap(), catalog);
        assert_eq!(reader.remaining(), 0);
        let mut duplicate = catalog.clone();
        duplicate
            .declaration_envelopes
            .push(catalog.declaration_envelopes[0].clone());
        assert!(
            duplicate
                .declaration_envelope(ContractId::new(7).unwrap())
                .is_none()
        );
        assert!(
            catalog
                .declaration_envelope(ContractId::new(8).unwrap())
                .is_none()
        );
    }
}
