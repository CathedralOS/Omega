//! Provider-attachment specialization replay tests.

use super::super::*;

#[test]
fn provider_attachment_specialization_replays_exact_roots_calls_and_nonuse() {
    let baseline = provider_attachment_specialization_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("repeated calls share one canonical provider requirement root");
    let machine = baseline.functions[0].machine;
    let invalid = OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(machine);
    let attachment = baseline.functions[0]
        .attachment
        .expect("provider fixture attachment");
    let first_boundary = baseline.boundary_machines[0].id;
    let second_boundary = baseline.boundary_machines[1].id;
    let unused_boundary = baseline.boundary_machines[2].id;
    let first_provider_place = baseline.functions[0].structural_places[0].id;

    let assert_invalid = |mut unit: PsiOptimizationUnit| {
        refresh_identity(&mut unit);
        assert_eq!(validate_psi_optimization_unit(&unit), Err(invalid.clone()));
    };

    let mut missing_root = baseline.clone();
    missing_root.functions[0].structural_places.pop();
    assert_invalid(missing_root);

    let mut extra_root = baseline.clone();
    extra_root.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: id(1, psi_core::StructuralFieldId::new),
                boundary: unused_boundary,
            },
        });
    assert_invalid(extra_root);

    let mut reordered_roots = baseline.clone();
    reordered_roots.functions[0].structural_places.swap(0, 1);
    assert_invalid(reordered_roots);

    let mut duplicate_root = baseline.clone();
    let duplicate_kind = duplicate_root.functions[0].structural_places[1].kind;
    duplicate_root.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: duplicate_kind,
        });
    assert_invalid(duplicate_root);

    let mut wrong_field = baseline.clone();
    let StructuralPlaceKind::ProviderAttachment { field, .. } =
        &mut wrong_field.functions[0].structural_places[1].kind
    else {
        panic!("provider fixture root")
    };
    *field = id(2, psi_core::StructuralFieldId::new);
    assert_invalid(wrong_field);

    let mut unknown_boundary = baseline.clone();
    let StructuralPlaceKind::ProviderAttachment { boundary, .. } =
        &mut unknown_boundary.functions[0].structural_places[1].kind
    else {
        panic!("provider fixture root")
    };
    *boundary = id(999, BoundaryMachineId::new);
    assert_invalid(unknown_boundary);

    let mut attached_boundary = baseline.clone();
    attached_boundary.boundary_machines[1].attachment = Some(attachment);
    assert_invalid(attached_boundary);

    let mut self_parameter = baseline.clone();
    let parameter_place = id(454, PlaceId::new);
    self_parameter.functions[0].structural_parameters.push(
        psi_terminal::StructuralParameterDeclaration {
            place: parameter_place,
            position: 0,
            is_self: true,
            structural_type: attachment,
            multiplicity: psi_terminal::StructuralMultiplicity::Unrestricted,
            access: psi_terminal::StructuralAccess::Owned,
            qualifications: Vec::new(),
        },
    );
    self_parameter.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: parameter_place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    assert_invalid(self_parameter);

    let mut missing_call = baseline.clone();
    let AbstractOperation::BoundaryCall { boundary, .. } =
        &mut missing_call.functions[0].blocks[0].nodes[2].operation
    else {
        panic!("provider fixture call")
    };
    *boundary = first_boundary;
    assert_invalid(missing_call);

    let mut extra_call = baseline.clone();
    let AbstractOperation::BoundaryCall { boundary, .. } =
        &mut extra_call.functions[0].blocks[0].nodes[1].operation
    else {
        panic!("provider fixture call")
    };
    *boundary = unused_boundary;
    assert_invalid(extra_call);

    let provider_argument = psi_terminal::StructuralArgument {
        place: first_provider_place,
        path: Vec::new(),
        access: psi_terminal::StructuralAccess::Owned,
    };
    let mut boundary_use = baseline.clone();
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut boundary_use.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("provider fixture call")
    };
    structural_arguments.push(provider_argument.clone());
    assert_invalid(boundary_use);

    let mut unit_use = baseline.clone();
    let psi_operation = match unit_use.functions[0].blocks[0].nodes[0].operation {
        AbstractOperation::BoundaryCall { psi_operation, .. } => psi_operation,
        _ => panic!("provider fixture call"),
    };
    unit_use.functions[0].blocks[0].nodes[0].operation = AbstractOperation::CallUnit {
        psi_operation,
        callee: machine,
        structural_arguments: vec![provider_argument],
        claim_transfers: Vec::new(),
    };
    refresh_node_derivatives(&mut unit_use, 0, 0, 0);
    assert_invalid(unit_use);

    let mut multiple_fields = baseline;
    let psi_terminal::StructuralTypeShape::Record { fields } =
        &mut multiple_fields.structural_types[0].shape
    else {
        panic!("provider fixture attachment record")
    };
    fields.push(structural_leaf_field(
        2,
        psi_terminal::BindingRelevance::Relevant,
        psi_terminal::StructuralFieldType::Erased {
            type_identity: "validation::second-provider".into(),
        },
    ));
    multiple_fields.functions[0]
        .structural_places
        .push(psi_terminal::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: id(2, psi_core::StructuralFieldId::new),
                boundary: unused_boundary,
            },
        });
    assert_invalid(multiple_fields);

    assert!(first_boundary < second_boundary);
}
