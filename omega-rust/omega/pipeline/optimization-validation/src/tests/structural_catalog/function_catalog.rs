//! Function structural-signature and attachment catalog tests.

use super::super::*;

#[test]
fn structural_signatures_replay_attachment_and_unique_self_legality() {
    let mut attached = structural_call_unit();
    let structural_type = attached.structural_types[0].id;
    attached.functions[0].attachment = Some(structural_type);
    attached.functions[0].structural_parameters[0].is_self = true;
    let StructuralPlaceKind::Parameter { is_self, .. } =
        &mut attached.functions[0].structural_places[0].kind
    else {
        panic!("fixture retains its parameter root")
    };
    *is_self = true;
    refresh_identity(&mut attached);
    validate_psi_optimization_unit(&attached)
        .expect("one attachment-typed self parameter is canonical");

    let mut self_without_attachment = attached.clone();
    self_without_attachment.functions[0].attachment = None;
    refresh_identity(&mut self_without_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&self_without_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut mismatched_self = attached.clone();
    let alternate = id(4_710, StructuralTypeId::new);
    mismatched_self
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-attachment".into(),
            shape: terminal_psi::StructuralTypeShape::Record { fields: Vec::new() },
        });
    mismatched_self.functions[0].attachment = Some(alternate);
    refresh_identity(&mut mismatched_self);
    assert!(matches!(
        validate_psi_optimization_unit(&mismatched_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut duplicate_self = attached.clone();
    let mut second = duplicate_self.functions[0].structural_parameters[0].clone();
    second.place = id(4_711, PlaceId::new);
    second.position = 1;
    duplicate_self.functions[0]
        .structural_parameters
        .push(second.clone());
    duplicate_self.functions[0]
        .structural_places
        .push(terminal_psi::StructuralPlaceDeclaration {
            id: second.place,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: true,
            },
        });
    refresh_identity(&mut duplicate_self);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut unknown_function_attachment = structural_call_unit();
    unknown_function_attachment.functions[0].attachment = Some(id(4_799, StructuralTypeId::new));
    refresh_identity(&mut unknown_function_attachment);
    assert!(matches!(
        validate_psi_optimization_unit(&unknown_function_attachment),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: Some(_) })
    ));

    let mut boundary_self = byte_literal_boundary_unit();
    let boundary_type = boundary_self.boundary_machines[0].structural_parameters[0].structural_type;
    boundary_self.boundary_machines[0].attachment = Some(boundary_type);
    boundary_self.boundary_machines[0].structural_parameters[0].is_self = true;
    refresh_identity(&mut boundary_self);
    validate_psi_optimization_unit(&boundary_self)
        .expect("boundary self uses the exact known attachment type");

    boundary_self.boundary_machines[0].attachment = Some(id(4_798, StructuralTypeId::new));
    refresh_identity(&mut boundary_self);
    assert_eq!(
        validate_psi_optimization_unit(&boundary_self),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { machine: None })
    );
}
