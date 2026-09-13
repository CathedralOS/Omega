use super::*;

#[test]
fn fixed_boundary_reach_is_encoded_before_published_ceiling() {
    let service = semantic_vocabulary::ServiceId::new(1).unwrap();
    let mut boundary = BoundaryMachineDeclaration {
        id: semantic_vocabulary::BoundaryMachineId::new(1).unwrap(),
        identity: "Installer::step".into(),
        attachment: None,
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
}
