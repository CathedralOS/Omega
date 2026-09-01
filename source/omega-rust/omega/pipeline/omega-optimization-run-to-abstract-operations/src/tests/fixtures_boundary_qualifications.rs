//! Whole-root boundary qualification consumption fixture.

use super::*;

pub(super) fn boundary_qualification_verified() -> VerifiedPsiOptimizationUnit {
    let machine = MachineId::new(1_901).unwrap();
    let block = BlockId::new(1_902).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(1_903).unwrap();
    let structural_type = psi_core::StructuralTypeId::new(1_904).unwrap();
    let required_domain = psi_core::StructuralDomainId::new(1_905).unwrap();
    let unrelated_domain = psi_core::StructuralDomainId::new(1_906).unwrap();
    let caller_place = psi_core::PlaceId::new(1_907).unwrap();
    let boundary_place = psi_core::PlaceId::new(1_908).unwrap();
    let operation = OperationId::new(1_909).unwrap();
    let parameter = |place, qualifications| psi_terminal::StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
        access: psi_terminal::StructuralAccess::SharedBorrow,
        qualifications,
    };
    let mut module = module_with_blocks(
        machine,
        block,
        TerminalMachineResult::Unit,
        vec![Block {
            id: block,
            parameters: Vec::new(),
            operations: vec![Operation {
                id: operation,
                result: OperationResult::Unit,
                kind: OperationKind::BoundaryCall {
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![psi_terminal::StructuralArgument {
                        place: caller_place,
                        path: Vec::new(),
                        access: psi_terminal::StructuralAccess::SharedBorrow,
                    }],
                    completion_receipts: Vec::new(),
                },
            }],
            terminator: Terminator::ReturnUnit {
                edge: EdgeId::new(1_910).unwrap(),
                trivial_affine_discards: Vec::new(),
            },
        }],
    );
    module.structural_types = vec![psi_terminal::StructuralTypeDeclaration {
        id: structural_type,
        identity: "test::QualifiedBoundaryCarrier".into(),
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    }];
    module.structural_domains = vec![
        psi_terminal::StructuralDomainDeclaration {
            id: required_domain,
            semantic_domain: psi_core::DomainSemanticId::new(1_911).unwrap(),
            identity: "test::RequiredBoundaryQualification".into(),
            carrier: structural_type,
            content_projection: None,
        },
        psi_terminal::StructuralDomainDeclaration {
            id: unrelated_domain,
            semantic_domain: psi_core::DomainSemanticId::new(1_912).unwrap(),
            identity: "test::UnrelatedQualification".into(),
            carrier: structural_type,
            content_projection: None,
        },
    ];
    module.boundary_machines = vec![psi_terminal::BoundaryMachineDeclaration {
        id: boundary,
        identity: "test::consume_qualification".into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: vec![parameter(boundary_place, Vec::new())],
        result: None,
        requires: vec![psi_terminal::StructuralDomainRequirement {
            argument_index: 0,
            domain: required_domain,
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    }];
    let caller = &mut module.machines[0];
    caller.structural_parameters = vec![parameter(caller_place, vec![required_domain])];
    caller.structural_places = vec![psi_terminal::StructuralPlaceDeclaration {
        id: caller_place,
        kind: psi_core::StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    }];

    verified(module, ProofBundle::default())
}
