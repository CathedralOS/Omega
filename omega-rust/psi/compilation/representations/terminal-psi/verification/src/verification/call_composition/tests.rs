use super::*;
use semantic_vocabulary::{
    BlockId, ContractId, EdgeId, OperationId, StructuralFieldId, StructuralPlaceKind,
};
use terminal_psi::{
    Block, MachineContract, OperationResult, StructuralFieldDeclaration, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPlaceDeclaration, StructuralTypeDeclaration,
    TerminalMachineResult, Terminator,
};

fn fixture() -> (TerminalModule, TerminalMachine) {
    let identity = MachineId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let entry = BlockId::new(1).unwrap();
    let parameters = (0..2)
        .map(|position| StructuralParameterDeclaration {
            place: PlaceId::new(u64::from(position) + 1).unwrap(),
            position,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        })
        .collect::<Vec<_>>();
    let machine = TerminalMachine {
        id: identity,
        attachment: None,
        parameters: Vec::new(),
        structural_places: parameters
            .iter()
            .map(|parameter| StructuralPlaceDeclaration {
                id: parameter.place,
                kind: StructuralPlaceKind::Parameter {
                    position: parameter.position,
                    is_self: false,
                },
            })
            .collect(),
        structural_parameters: parameters,
        ranked_scc: None,
        result: TerminalMachineResult::Unit,
        entry_claims: Vec::new(),
        declared_service_reach: Vec::new(),
        closed_reach_application: None,
        published_service_ceiling: Vec::new(),
        content_entry_claims: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
        entry,
        blocks: vec![Block {
            id: entry,
            parameters: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            structural_parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
        contract: MachineContract {
            id: ContractId::new(1).unwrap(),
            crash_routes: Vec::new(),
            erased_scalar_formals: Vec::new(),
            erased_proof_formals: Vec::new(),
            requires: Vec::new(),
            ensures: Vec::new(),
            outcome_specific_ensures: Vec::new(),
        },
    };
    let mut module = TerminalModule::for_entry(identity);
    module.structural_types.push(StructuralTypeDeclaration {
        id: structural_type,
        identity: "Cell".into(),
        shape: StructuralTypeShape::Record {
            fields: vec![
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "atomic".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Atomic(ScalarType::Boolean),
                },
                StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: "plain".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                },
            ],
        },
    });
    (module, machine)
}

fn observation(root: u64, field: u64) -> Proposition {
    Proposition::Equal(
        ScalarTerm::BooleanField {
            root: PlaceId::new(root).unwrap(),
            path: vec![CanonicalStructuralPathSegment::Field(
                StructuralFieldId::new(field).unwrap(),
            )],
        },
        ScalarTerm::Boolean(true),
    )
}

#[test]
fn shared_atomic_call_expires_alias_observations_but_keeps_plain_siblings_and_snapshots() {
    let (module, machine) = fixture();
    let argument = StructuralArgument {
        place: PlaceId::new(1).unwrap(),
        path: Vec::new(),
        access: StructuralAccess::SharedBorrow,
    };
    let operation = Operation {
        static_reach_binding: None,
        id: OperationId::new(1).unwrap(),
        suspension_crossing: None,
        result: OperationResult::Unit,
        kind: OperationKind::CallUnit {
            callee: MachineId::new(2).unwrap(),
            arguments: Vec::new(),
            erased_arguments: Vec::new(),
            erased_proof_arguments: Vec::new(),
            structural_arguments: vec![argument.clone()],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        },
    };
    let snapshot = Proposition::Equal(
        ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
        ScalarTerm::Boolean(true),
    );
    let before = vec![
        observation(1, 1),
        observation(1, 2),
        observation(2, 1),
        observation(2, 2),
        snapshot.clone(),
    ];
    let mut axioms = before.clone();
    let storage = super::super::storage_observations::StorageObservations::new(&module, &machine);
    invalidate_mutated_arguments(
        &module,
        &machine,
        &operation,
        &mut axioms,
        &[argument.clone()],
        &storage,
        false,
    );
    assert_eq!(axioms, vec![observation(1, 2), observation(2, 2), snapshot]);

    // Lending only the ordinary field grants no access to its atomic sibling.
    let ordinary = StructuralArgument {
        path: vec![terminal_psi::StructuralPathSegment::Field("plain".into())],
        ..argument
    };
    let mut axioms = before.clone();
    invalidate_mutated_arguments(
        &module,
        &machine,
        &operation,
        &mut axioms,
        &[ordinary],
        &storage,
        false,
    );
    assert_eq!(axioms, before);
}
