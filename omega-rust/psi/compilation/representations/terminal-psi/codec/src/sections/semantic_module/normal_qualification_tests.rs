//! Normal preservation is path-scoped metadata, not a root-only promise.

use super::module_wire::encode_raw;
use super::target_family_tests::{identity_table, unit_machine};
use crate::{CodecError, decode_module, encode_module, semantic_fingerprint};
use semantic_vocabulary::{
    BoundaryMachineId, DomainSemanticId, MachineId, OperationId, PlaceId, StructuralDomainId,
    StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, BoundaryCompletion, BoundaryLenderPreservationDemand,
    BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryMachineResult,
    BoundaryParameterKind, BoundaryReferenceDestination, BoundaryReferenceOutput,
    BoundaryRequirementKind, NominalDeclarationIdentity, StructuralAccess,
    StructuralDomainDeclaration, StructuralDomainRequirement, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
    TerminalIdentityIndex, TerminalModule,
};

fn type_id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}

fn field(value: u32) -> StructuralPathSegment {
    StructuralPathSegment::Field(TerminalIdentityIndex(value))
}

pub(super) fn module() -> TerminalModule {
    let machine = MachineId::new(1).unwrap();
    let mut module = TerminalModule::for_entry(machine);
    module.identities = identity_table(12);
    module.machines.push(unit_machine(machine));
    let shapes = [
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: Vec::new(),
        },
        StructuralTypeShape::FixedArray {
            element: type_id(1),
            length: terminal_psi::ArrayExtent::Literal(3),
        },
        StructuralTypeShape::Reference {
            referent: type_id(2),
            access: StructuralAccess::MutableBorrow,
        },
        StructuralTypeShape::Record {
            storage: StructuralRecordStorage::Packed,
            fields: [8, 9]
                .into_iter()
                .enumerate()
                .map(|(position, identity)| StructuralFieldDeclaration {
                    inline_carrier: None,
                    id: StructuralFieldId::new(position as u64 + 1).unwrap(),
                    identity: TerminalIdentityIndex(identity),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(type_id(1)),
                    storage: None,
                })
                .collect(),
        },
    ];
    for (position, shape) in shapes.into_iter().enumerate() {
        if let StructuralTypeShape::Record { fields, .. } = &shape {
            module.native_layouts.insert(
                type_id(position as u64 + 1),
                TerminalIdentityIndex(position as u32 + 4),
                TerminalIdentityIndex::ABSENT,
                TerminalIdentityIndex::ABSENT,
                fields
                    .iter()
                    .map(|field| terminal_psi::NativeLayoutFieldIdentity {
                        field: field.id,
                        declaration_identity: field.identity,
                    }),
                [],
            );
        }
        module.structural_types.push(StructuralTypeDeclaration {
            id: type_id(position as u64 + 1),
            identity: TerminalIdentityIndex(position as u32 + 4),
            quotient_carrier: None,
            owns_linear_claim: false,
            no_code_affine_cleanup: position != 2,
            copyable: false,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        });
    }
    let domain = StructuralDomainId::new(1).unwrap();
    module.structural_domains.push(StructuralDomainDeclaration {
        id: domain,
        semantic_domain: DomainSemanticId::new(1).unwrap(),
        identity: TerminalIdentityIndex(10),
        carrier: type_id(1),
        required_memberships: Some(Vec::new()),
        complete_predicate_theory: false,
        predicate: None,
        content_projection: None,
        byte_predicate: None,
        establishment_routes: Vec::new(),
    });
    module.boundary_machines.push(BoundaryMachineDeclaration {
        id: BoundaryMachineId::new(1).unwrap(),
        declaration: None,
        role: terminal_psi::BoundaryDeclarationRole::Callable,
        kind: BoundaryRequirementKind::Machine,
        completion: BoundaryCompletion::NormalReturn,
        numeric_intrinsic: None,
        unconditional_termination: false,
        identity: TerminalIdentityIndex(11),
        owner: NominalDeclarationIdentity::new(TerminalIdentityIndex(1)),
        attachment: None,
        scalar_parameters: Vec::new(),
        parameter_order: vec![BoundaryParameterKind::Structural; 3],
        crash_routes: Vec::new(),
        structural_parameters: [1, 4, 3]
            .into_iter()
            .enumerate()
            .map(|(position, carrier)| StructuralParameterDeclaration {
                place: PlaceId::new(position as u64 + 1).unwrap(),
                position: position as u32,
                structural_type: type_id(carrier),
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::MutableBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            })
            .collect(),
        result: BoundaryMachineResult::Unit,
        requires: vec![StructuralDomainRequirement {
            argument_index: 1,
            path: vec![field(8)],
            domain,
        }],
        normal_qualifications: [
            (0, Vec::new()),
            (1, vec![field(8)]),
            (
                2,
                vec![
                    StructuralPathSegment::Referent,
                    StructuralPathSegment::AllElements,
                ],
            ),
        ]
        .into_iter()
        .map(|(argument_index, path)| StructuralDomainRequirement {
            argument_index,
            path,
            domain,
        })
        .collect(),
        scalar_domain_requirements: Vec::new(),
        scalar_requires: Vec::new(),
        scalar_ensures: Vec::new(),
        // Parameter two exposes a writable reference descriptor. Its complete
        // opaque completion contract retains the original stored loan as a
        // lifetime anchor; it does not promise the old pointer's identity.
        result_reference_regions: Vec::new(),
        reference_outputs: vec![BoundaryReferenceOutput {
            destination: BoundaryReferenceDestination::Parameter(2),
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
            dependencies: vec![BoundaryLifetimeDependency::Stored {
                argument_index: 2,
                path: Vec::new(),
                access: StructuralAccess::MutableBorrow,
            }],
        }],
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        owning_service: None,
        published_service_ceiling: Vec::new(),
    });
    module
}

#[test]
fn normal_qualification_round_trip_retains_root_field_and_all_referent_elements() {
    let module = module();
    let bytes = encode_module(&module).unwrap();
    let decoded = decode_module(&bytes).unwrap();
    assert_eq!(decoded, module);
    assert_eq!(encode_module(&decoded).unwrap(), bytes);
}

#[test]
fn normal_qualification_and_required_paths_change_whole_semantic_fingerprint() {
    let module = module();
    let original = semantic_fingerprint(&module).unwrap();
    let mut changed = module.clone();
    changed.boundary_machines[0].normal_qualifications.clear();
    assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    let mut changed = module.clone();
    changed.boundary_machines[0].normal_qualifications[2].path[1] =
        StructuralPathSegment::FixedIndex(0);
    assert_eq!(
        decode_module(&encode_module(&changed).unwrap()).unwrap(),
        changed
    );
    assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    let mut changed = module.clone();
    changed.boundary_machines[0].requires[0].path = vec![field(9)];
    assert_eq!(
        decode_module(&encode_module(&changed).unwrap()).unwrap(),
        changed
    );
    assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
}

#[test]
fn normal_qualification_public_decoder_rejects_wrong_carrier_and_missing_path() {
    let original = module();
    assert_eq!(
        decode_module(&encode_module(&original).unwrap()).unwrap(),
        original
    );
    for path in [
        // The array itself does not have its elements' domain.
        vec![StructuralPathSegment::Referent],
        // An array index cannot silently cross the stored reference.
        vec![StructuralPathSegment::FixedIndex(0)],
    ] {
        let mut malformed = original.clone();
        malformed.boundary_machines[0].normal_qualifications[2].path = path;
        assert_eq!(
            decode_module(&encode_raw(&malformed).unwrap()),
            Err(CodecError::MalformedStructuralFoundation(
                "boundary normal guarantee has the wrong mutable path carrier"
            )),
        );
    }
    let mut malformed = original;
    malformed.boundary_machines[0].requires[0].path = vec![field(12)];
    assert_eq!(
        decode_module(&encode_raw(&malformed).unwrap()),
        Err(CodecError::MalformedStructuralFoundation(
            "boundary requirement domain has the wrong path carrier"
        )),
    );
}

#[test]
fn lender_preservation_wire_and_identity_retain_exact_call_and_requirement() {
    let mut module = module();
    module.boundary_lender_preservation = vec![BoundaryLenderPreservationDemand {
        machine: MachineId::new(1).unwrap(),
        operation: OperationId::new(1).unwrap(),
        requirement: module.boundary_machines[0].normal_qualifications[2].clone(),
    }];
    let original = semantic_fingerprint(&module).unwrap();
    // This test isolates transport. Public admission separately checks that
    // the named operation really is a call owing this requirement.
    let bytes = encode_module(&module).unwrap();
    let mut reader = super::wire::Reader::new(&bytes);
    assert_eq!(reader.take(super::MAGIC.len()).unwrap(), super::MAGIC);
    assert_eq!(reader.u16().unwrap(), super::FORMAT_MARKER);
    let decoded = super::module_wire::decode_module_body(&mut reader).unwrap();
    assert_eq!(reader.remaining(), 0);
    assert_eq!(decoded, module);

    let mut changed = module.clone();
    changed.boundary_lender_preservation.clear();
    assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    let mut variants = vec![module.clone(); 5];
    variants[0].boundary_lender_preservation[0].machine = MachineId::new(2).unwrap();
    variants[1].boundary_lender_preservation[0].operation = OperationId::new(2).unwrap();
    variants[2].boundary_lender_preservation[0]
        .requirement
        .argument_index = 1;
    variants[3].boundary_lender_preservation[0]
        .requirement
        .path
        .pop();
    variants[4].boundary_lender_preservation[0]
        .requirement
        .domain = StructuralDomainId::new(2).unwrap();
    for changed in variants {
        assert_ne!(semantic_fingerprint(&changed).unwrap(), original);
    }
}

#[test]
fn lender_preservation_roster_refuses_duplicate_and_reordered_requests() {
    let mut module = module();
    let first = BoundaryLenderPreservationDemand {
        machine: MachineId::new(1).unwrap(),
        operation: OperationId::new(1).unwrap(),
        requirement: module.boundary_machines[0].normal_qualifications[2].clone(),
    };
    let mut second = first.clone();
    second.operation = OperationId::new(2).unwrap();
    module.boundary_lender_preservation = vec![first.clone(), second];
    assert!(encode_module(&module).is_ok());
    module.boundary_lender_preservation.reverse();
    assert_eq!(
        encode_module(&module),
        Err(CodecError::NonCanonicalOrder(
            "boundary lender preservation demands"
        )),
    );
    module.boundary_lender_preservation = vec![first.clone(), first];
    assert_eq!(
        encode_module(&module),
        Err(CodecError::NonCanonicalOrder(
            "boundary lender preservation demands"
        )),
    );
}
