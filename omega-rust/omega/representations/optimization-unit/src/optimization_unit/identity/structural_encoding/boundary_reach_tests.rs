use super::{BoundaryMachineDeclaration, CanonicalBytes, encode_boundary_machine, encode_ids};
#[test]
fn boundary_identity_binds_fixed_reach_and_formal_order() {
    let service = semantic_vocabulary::ServiceId::new(1).unwrap();
    let mut boundary = BoundaryMachineDeclaration {
        id: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
        identity: "Installer::step".into(),
        attachment: None,
        parameter_order: Vec::new(),
        scalar_parameters: Vec::new(),
        crash_routes: Vec::new(),
        structural_parameters: Vec::new(),
        result: terminal_psi::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        fixed_service_reach: Vec::new(),
        published_service_ceiling: vec![service],
    };
    let encode = |declaration: &BoundaryMachineDeclaration| {
        let mut bytes = CanonicalBytes::collect();
        encode_boundary_machine(&mut bytes, declaration);
        bytes.finish()
    };
    let original = encode(&boundary);
    boundary.fixed_service_reach.push(service);
    let changed = encode(&boundary);
    assert_ne!(original, changed);
    let mut rows = CanonicalBytes::collect();
    encode_ids(&mut rows, &boundary.fixed_service_reach);
    encode_ids(&mut rows, &boundary.published_service_ceiling);
    assert!(changed.ends_with(&rows.finish()));
    boundary
        .scalar_parameters
        .push(semantic_vocabulary::ScalarType::Boolean);
    boundary
        .structural_parameters
        .push(terminal_psi::StructuralParameterDeclaration {
            place: semantic_vocabulary::PlaceId::new(1).unwrap(),
            position: 0,
            is_self: false,
            structural_type: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            multiplicity: terminal_psi::StructuralMultiplicity::Unrestricted,
            access: terminal_psi::StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    boundary.parameter_order = vec![
        terminal_psi::BoundaryParameterKind::Scalar,
        terminal_psi::BoundaryParameterKind::Structural,
    ];
    assert!(boundary.has_valid_parameter_order());
    let original_order = encode(&boundary);
    boundary.parameter_order.reverse();
    assert!(boundary.has_valid_parameter_order());
    assert_ne!(original_order, encode(&boundary));
}
