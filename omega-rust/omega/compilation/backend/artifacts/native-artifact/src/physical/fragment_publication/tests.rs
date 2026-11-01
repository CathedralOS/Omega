use super::validate_final_plan;

#[test]
fn construction_rejects_detached_final_plan_and_object_terminal() {
    let psi = terminal_psi::TerminalPsiIdentity {
        vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
        program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([17; 32]),
    };
    let retained = abstract_operations::AbstractOperationPlan {
        psi,
        entry: semantic_vocabulary::MachineId::new(1).unwrap(),
        identities: Default::default(),
        structural_types: Vec::new().into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        dynamic_dispatch: Default::default(),
        functions: Vec::new(),
    };
    assert!(validate_final_plan(&retained, &retained, psi, psi).is_ok());
    let mut changed = retained.clone();
    changed.entry = semantic_vocabulary::MachineId::new(2).unwrap();
    assert!(validate_final_plan(&changed, &retained, psi, psi).is_err());
    let mut changed_terminal = psi;
    changed_terminal.program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([18; 32]);
    assert!(validate_final_plan(&retained, &retained, psi, changed_terminal).is_err());
}
