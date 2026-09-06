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
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: id(1, semantic_vocabulary::StructuralFieldId::new),
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
        .push(terminal_psi::StructuralPlaceDeclaration {
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
    *field = id(2, semantic_vocabulary::StructuralFieldId::new);
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
        terminal_psi::StructuralParameterDeclaration {
            place: parameter_place,
            position: 0,
            is_self: true,
            structural_type: attachment,
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
    );
    self_parameter.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: parameter_place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: true,
            },
        });
    self_parameter.functions[0]
        .declared_places
        .insert(parameter_place);
    refresh_identity(&mut self_parameter);
    validate_psi_optimization_unit(&self_parameter)
        .expect("one exact self parameter remains separate from provider requirement roots");

    let mut borrowed_self = self_parameter.clone();
    borrowed_self.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Affine;
    borrowed_self.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::MutableBorrow;
    refresh_identity(&mut borrowed_self);
    validate_psi_optimization_unit(&borrowed_self)
        .expect("a borrowed attachment receiver does not become a provider requirement root");

    borrowed_self.functions[0].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Unrestricted;
    borrowed_self.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::SharedBorrow;
    refresh_identity(&mut borrowed_self);
    validate_psi_optimization_unit(&borrowed_self)
        .expect("a shared receiver retains the same exact provider requirements");

    let mut displaced_self = self_parameter;
    let mut ordinary_parameter = displaced_self.functions[0].structural_parameters[0].clone();
    ordinary_parameter.place = id(455, PlaceId::new);
    ordinary_parameter.is_self = false;
    displaced_self.functions[0].structural_parameters[0].position = 1;
    displaced_self.functions[0]
        .structural_parameters
        .insert(0, ordinary_parameter.clone());
    let self_place = displaced_self.functions[0]
        .structural_places
        .last_mut()
        .unwrap();
    self_place.kind = StructuralPlaceKind::Parameter {
        position: 1,
        is_self: true,
    };
    displaced_self.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: ordinary_parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: 0,
                is_self: false,
            },
        });
    displaced_self.functions[0]
        .declared_places
        .insert(ordinary_parameter.place);
    assert_invalid(displaced_self);

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

    let provider_argument = terminal_psi::StructuralArgument {
        place: first_provider_place,
        path: Vec::new(),
        access: terminal_psi::StructuralAccess::Owned,
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
        arguments: Vec::new(),
        structural_arguments: vec![provider_argument],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    refresh_node_derivatives(&mut unit_use, 0, 0, 0);
    assert_invalid(unit_use);

    let mut multiple_fields = baseline;
    let terminal_psi::StructuralTypeShape::Record { fields } =
        &mut multiple_fields.structural_types[0].shape
    else {
        panic!("provider fixture attachment record")
    };
    fields.push(structural_leaf_field(
        2,
        terminal_psi::BindingRelevance::Relevant,
        terminal_psi::StructuralFieldType::Erased {
            type_identity: "validation::second-provider".into(),
        },
    ));
    multiple_fields.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: id(453, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: id(2, semantic_vocabulary::StructuralFieldId::new),
                boundary: unused_boundary,
            },
        });
    assert_invalid(multiple_fields);

    assert!(first_boundary < second_boundary);
}
