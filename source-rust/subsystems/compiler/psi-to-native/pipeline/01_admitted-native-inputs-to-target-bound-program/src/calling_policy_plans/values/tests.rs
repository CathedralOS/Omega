use super::*;
use semantic_vocabulary::{BlockId, ContractId, EdgeId, MachineId, PlaceId, StructuralFieldId};
use semantic_vocabulary::{ScalarType, StructuralPlaceKind};
use terminal_interpreter::{
    TerminalExecutionStatus, TerminalInstantiation, TerminalStructuralInputs,
    TerminalStructuralValue,
};
use terminal_psi::{
    Block, BoundaryParameterKind, MachineContract, RegisterTargetApplicability, StructuralAccess,
    StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralRecordStorage,
    StructuralResultDeclaration, StructuralTypeDeclaration, StructuralTypeShape, TerminalMachine,
    TerminalMachineResult, TerminalModule, Terminator,
};
fn type_id() -> StructuralTypeId {
    StructuralTypeId::new(1).unwrap()
}
fn machine() -> TerminalMachine {
    let place = PlaceId::new(1).unwrap();
    let mut contract = MachineContract::empty(ContractId::new(1).unwrap());
    contract
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            structural_type: type_id(),
            access: StructuralAccess::Owned,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
        });
    contract
        .parameter_order
        .push(BoundaryParameterKind::Structural);
    contract.result = TerminalMachineResult::Structural(StructuralResultDeclaration {
        place: PlaceId::new(2).unwrap(),
        structural_type: type_id(),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        reference_sources: Vec::new(),
    });
    let entry = BlockId::new(1).unwrap();
    TerminalMachine {
        assembly_target_requirement: RegisterTargetApplicability::Any,
        id: MachineId::new(1).unwrap(),
        attachment: None,
        ranked_scc: None,
        structural_places: vec![
            StructuralPlaceDeclaration {
                id: place,
                kind: StructuralPlaceKind::Parameter { position: 0 },
            },
            StructuralPlaceDeclaration {
                id: PlaceId::new(2).unwrap(),
                kind: StructuralPlaceKind::Result,
            },
        ],
        call_formations: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block::plain(
            entry,
            Vec::new(),
            Terminator::ReturnStructural {
                edge: EdgeId::new(1).unwrap(),
                source: place,
                returned_claims: Vec::new(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract,
    }
}

fn echo_record() -> terminal_codec::CanonicalTerminalArtifact {
    let mut module = TerminalModule::for_entry(MachineId::new(1).unwrap());
    let mut labels = terminal_psi::TerminalIdentityTableBuilder::default();
    module.structural_types.push(StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        id: type_id(),
        identity: labels.intern("PolicySignature"),
        quotient_carrier: None,
        owns_linear_claim: false,
        multiplicity: StructuralMultiplicity::Unrestricted,
        no_code_affine_cleanup: true,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        scalar_restriction: None,
        requires: Vec::new(),
        shape: StructuralTypeShape::Record {
            fields: vec![StructuralFieldDeclaration {
                declaration_identity: Default::default(),
                inline_carrier: None,
                id: StructuralFieldId::new(1).unwrap(),
                identity: labels.intern("available"),
                relevance: Default::default(),
                field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                storage: None,
            }],
            storage: StructuralRecordStorage::Packed,
        },
    });
    module.machines.push(machine());
    module.identities = labels.build();
    let record = terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
    terminal_codec::CanonicalTerminalArtifact::from_parts(module, record, None).unwrap()
}

#[test]
fn projects_a_declared_field_through_the_interpreted_policy() {
    let artifact = echo_record();
    let program = TerminalProgram::borrow(artifact.view());
    let signature = BuildTimeValue::Struct {
        type_name: "PolicySignature".into(),
        fields: vec![("available".into(), BuildTimeValue::Bool(true))],
    };
    let contents = [signature_contents(&program, type_id(), &signature).unwrap()];
    let args = [TerminalStructuralValue::minted(1, type_id(), Vec::new())];
    let mut instance = program
        .instantiate(TerminalInstantiation {
            structural_inputs: TerminalStructuralInputs {
                arguments: &args,
                contents: &contents,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(100))
        .unwrap();
    assert!(matches!(status, TerminalExecutionStatus::Complete(_)));
    assert_eq!(plan_value(&instance, type_id()).unwrap(), signature);
    let malformed = BuildTimeValue::Struct {
        type_name: "PolicySignature".into(),
        fields: Vec::new(),
    };
    assert!(signature_contents(&program, type_id(), &malformed).is_err());
}
