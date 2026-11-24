use super::input_corresponds;
use semantic_vocabulary::{
    BlockId, ClaimId, ContentAlgebra, ContentAlgebraKind, ContentConservation, ContentDomainId,
    ContentPlaceVersion, ContentProjectionIdentity, ContentStructuralPlace, ContentTerm,
    ContractId, EdgeId, MachineId, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    Block, ClaimContentProjection, ContentBodyProjectionSubstitution, ContentClaimProjection,
    ContentEntryClaim, ContentTransformationCustody, ContentTransformationDerivation,
    MachineContract, Operation, OperationKind, OperationResult, RegisterTargetApplicability,
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    TerminalMachine, TerminalModule, Terminator,
};

#[test]
fn a_public_input_can_use_only_its_own_entry_content_and_descendants() {
    let claim = |id| ClaimId::new(id).unwrap();
    let subject = |root, version| ContentStructuralPlace {
        root: PlaceId::new(root).unwrap(),
        version,
        segments: Vec::new(),
    };
    let projection = ContentProjectionIdentity {
        domain: ContentDomainId::new(1).unwrap(),
        projection_report_fingerprint: 81,
    };
    let term = |subject| ContentTerm::Projection {
        projection,
        subject,
    };
    let algebra = ContentAlgebra {
        kind: ContentAlgebraKind::IntervalSet,
        parameter: "coordinate-space".into(),
    };
    let formal = StructuralParameterDeclaration {
        place: PlaceId::new(10).unwrap(),
        position: 0,
        structural_type: StructuralTypeId::new(1).unwrap(),
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        additional_input_qualifications: Vec::new(),
    };
    let entry = |id, root| ContentEntryClaim {
        claim: claim(id),
        input: subject(root, ContentPlaceVersion::Entry),
        projections: vec![ClaimContentProjection {
            projection,
            algebra: algebra.clone(),
        }],
    };
    let entries = [entry(1, 10), entry(2, 20)];
    let binding = |id| ContentClaimProjection {
        claim: claim(id),
        projection,
        subject: subject(id * 10, ContentPlaceVersion::Current),
    };
    let current = |id| term(binding(id).subject);
    let input = term(subject(1, ContentPlaceVersion::Entry));
    let subst = |target| ContentBodyProjectionSubstitution {
        source: input.clone(),
        target,
    };
    let mut derived = ContentTransformationDerivation {
        conservation: ContentConservation::new(algebra, current(1), current(2)),
        lineage_roots: vec![claim(1), claim(2)],
        ancestry: Vec::new(),
        inputs: vec![binding(1), binding(2)],
        outputs: Vec::new(),
        custody: ContentTransformationCustody::Derived,
    };
    let machine_id = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine_id);
    let block = BlockId::new(1).unwrap();
    let mut machine = TerminalMachine {
        id: machine_id,
        entry: block,
        blocks: vec![Block::plain(
            block,
            Vec::new(),
            Terminator::ReturnUnit {
                edge: EdgeId::new(1).unwrap(),
                cleanup_actions: Vec::new(),
            },
        )],
        contract: MachineContract {
            structural_parameters: vec![
                formal.clone(),
                StructuralParameterDeclaration {
                    place: entries[1].input.root,
                    position: 1,
                    ..formal.clone()
                },
            ],
            content_entry_claims: entries.to_vec(),
            ..MachineContract::empty(ContractId::new(1).unwrap())
        },
        call_formations: Vec::new(),
        assembly_target_requirement: RegisterTargetApplicability::Any,
        attachment: None,
        ranked_scc: None,
        structural_places: Vec::new(),
        block_entry_claims: Vec::new(),
        loan_completions: Vec::new(),
        content_identity_reshuffles: Vec::new(),
        content_partition_compositions: Vec::new(),
    };
    let matches = |module: &TerminalModule,
                   machine: &TerminalMachine,
                   derived: &ContentTransformationDerivation,
                   target| {
        input_corresponds(
            module,
            machine,
            &formal,
            &subst(target),
            derived,
            &mut super::super::ancestry::OriginTraversal::new(),
        )
    };
    assert!(matches(&module, &machine, &derived, current(1)));
    assert!(!matches(&module, &machine, &derived, current(2)));
    machine.contract.content_entry_claims[0].input.version = ContentPlaceVersion::Current;
    assert!(!matches(&module, &machine, &derived, current(1)));
    machine.contract.content_entry_claims[0] = entries[0].clone();
    machine.contract.content_entry_claims[0].projections[0]
        .algebra
        .parameter = "another-space".into();
    assert!(!matches(&module, &machine, &derived, current(1)));
    machine.contract.content_entry_claims[0] = entries[0].clone();

    // A completed internal call forwards the first actual and returns a fresh
    // caller-side claim ID. Its return edge specifies which entry it continues.
    let mut callee = machine.clone();
    callee.id = MachineId::new(2).unwrap();
    callee.contract.content_entry_claims.truncate(1);
    callee.contract.structural_parameters.truncate(1);
    callee.contract.entry_claims = vec![terminal_psi::EntryClaim {
        claim: claim(1),
        input: formal.place,
        path: Vec::new(),
    }];
    callee.contract.parameter_order = vec![terminal_psi::BoundaryParameterKind::Structural];
    callee.contract.result = terminal_psi::TerminalMachineResult::Structural(
        terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(11).unwrap(),
            structural_type: formal.structural_type,
            multiplicity: formal.multiplicity,
            qualifications: formal.qualifications.clone(),
            projected_qualifications: Vec::new(),
            reference_sources: Vec::new(),
        },
    );
    let returned = |id| terminal_psi::StructuralResultClaimBinding {
        claim: claim(id),
        path: Vec::new(),
    };
    callee.blocks[0].terminator = Terminator::ReturnStructural {
        edge: EdgeId::new(1).unwrap(),
        source: formal.place,
        returned_claims: vec![returned(1)],
        cleanup_actions: Vec::new(),
    };
    machine.blocks[0].operations.push(Operation::unsuspended(
        OperationId::new(1).unwrap(),
        OperationResult::Structural(terminal_psi::StructuralOperationResult {
            place: binding(3).subject.root,
            structural_type: formal.structural_type,
            multiplicity: formal.multiplicity,
            qualifications: formal.qualifications.clone(),
            projected_qualifications: Vec::new(),
            additional_input_qualifications: Vec::new(),
            qualification_establishments: Vec::new(),
            claims: vec![returned(3)],
        }),
        OperationKind::CallStructural {
            callee: callee.id,
            structural_arguments: vec![StructuralArgument::whole(
                formal.place,
                StructuralAccess::Owned,
            )],
            claim_transfers: vec![terminal_psi::ClaimTransfer {
                claim: claim(1),
                argument_index: 0,
                path: Vec::new(),
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
    ));
    module.machines.push(callee);
    derived.inputs = vec![binding(2), binding(3)];
    assert!(matches(&module, &machine, &derived, current(3)));
    assert!(!matches(
        &module,
        &machine,
        &derived,
        ContentTerm::separate([current(2), current(3)]).unwrap()
    ));

    // Sending the other input through the same helper changes the returned
    // claim's origin even when the result place and projection are identical.
    let OperationKind::CallStructural {
        structural_arguments,
        claim_transfers,
        ..
    } = &mut machine.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    structural_arguments[0].place = entries[1].input.root;
    claim_transfers[0].claim = entries[1].claim;
    assert!(!matches(&module, &machine, &derived, current(3)));
}
