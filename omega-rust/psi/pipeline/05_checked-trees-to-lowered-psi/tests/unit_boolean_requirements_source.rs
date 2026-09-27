//! Boolean preconditions retain their actual scalar identity across Unit calls.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

fn roundtrip(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("Boolean requirements survive mixed Unit lowering");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent Boolean call requirement verification");
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("Boolean requirement source publishes Terminal")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        module
    );
    lowered
}

const SOURCE: &str = r#"
    data Metrics { current: u64; }
    boundary trait Sink { machine record(flag: bool); }
    data Helper {}
    machine Helper::consume(flag: bool, metrics: Metrics)
    reaches Sink requires flag
    { Sink::record(flag); }
    data Main {}
    machine Main::main(metrics: Metrics, flag: bool)
    requires flag
    { Helper::consume(flag, metrics); }
"#;

#[test]
fn write_only_forwarding_proves_the_callee_field_requirement() {
    let lowered = roundtrip(
        r#"
        data Cell { flag: bool; }
        data Helper {}
        machine Helper::consume(destination: &write Cell)
        requires destination.flag
        { destination.flag = false; }
        data Main {}
        machine Main::main(cell: &write Cell)
        requires cell.flag
        { Helper::consume(&write cell); }
    "#,
    );
    assert_call_requirement_certificates(&lowered);
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .all(|operation| !matches!(
                operation.kind,
                terminal_psi::OperationKind::BooleanStructuralField { .. }
            ))
    );
}

fn assert_call_requirement_certificates(lowered: &lowered_psi::LoweredPsi) {
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let (operation, callee, requirements) = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit {
                callee,
                requirement_obligations,
                ..
            } => Some((operation.id, *callee, requirement_obligations)),
            _ => None,
        })
        .unwrap();
    let callee = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == callee)
        .unwrap();
    assert!(
        !callee.contract.requires.is_empty(),
        "authored Boolean requirements are not discarded"
    );
    assert_eq!(requirements.len(), callee.contract.requires.len());
    let reconstructed =
        terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
    for (position, obligation) in requirements.iter().enumerate() {
        let site = reconstructed
            .iter()
            .find(|site| site.obligation.id == *obligation)
            .unwrap();
        assert_eq!(
            site.owner,
            terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires {
                machine: root.id,
                operation,
                requirement_position: position as u32,
            }
        );
        let evidence = lowered
            .proof_bundle
            .evidence
            .iter()
            .find(|evidence| evidence.obligation == *obligation)
            .unwrap();
        let proof_admission::EvidenceRoute::CertificateDerived(certificate) = &evidence.route
        else {
            panic!("Boolean call requirement uses independently checked evidence");
        };
        assert_eq!(certificate.proof.conclusion, site.obligation.proposition);
    }
}

#[test]
fn boolean_requirement_formulas_preserve_scalar_and_structural_subjects() {
    for requirement in [
        "metrics.enabled",
        "!flag",
        "!metrics.enabled",
        "flag && metrics.enabled",
        "flag || metrics.enabled",
        "flag == metrics.enabled",
        "!(flag && metrics.enabled)",
        "!(flag || metrics.enabled)",
        "(flag || metrics.enabled) == !flag",
        "true",
        "!false",
    ] {
        let source = SOURCE
            .replace("current: u64", "enabled: bool")
            .replace("requires flag", &format!("requires {requirement}"));
        let lowered = roundtrip(&source);
        assert_call_requirement_certificates(&lowered);
        let root = lowered
            .semantic_module
            .machines
            .iter()
            .find(|machine| machine.id == lowered.semantic_module.entry)
            .unwrap();
        let reconstructed =
            terminal_verifier::reconstruct_operation_obligations(&lowered.semantic_module).unwrap();
        for site in reconstructed.iter().filter(|site| matches!(site.owner,
            terminal_verifier::ReconstructedTerminalObligationOwner::CallRequires { machine, .. } if machine == root.id)) {
            assert!(root.contract.requires.contains(&site.obligation.proposition),
                "forwarded requirement rejoins the caller's exact scalar/place namespace: {requirement}: {:?}", site.obligation.proposition);
        }
    }
}

fn computed_boolean_actual(actual: &str, caller_requirement: &str) -> lowered_psi::LoweredPsi {
    let source = SOURCE.replace(
        "requires flag\n    { Helper::consume(flag, metrics); }",
        &format!("{caller_requirement}\n    {{ Helper::consume({actual}, metrics); }}"),
    );
    let lowered = roundtrip(&source);
    assert_call_requirement_certificates(&lowered);
    let root = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert_eq!(
        root.contract.requires.is_empty(),
        caller_requirement.is_empty()
    );
    let argument = root
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match &operation.kind {
            terminal_psi::OperationKind::CallUnit { arguments, .. } => Some(arguments[0]),
            _ => None,
        })
        .unwrap();
    let definition = root.blocks.iter().flat_map(|block| &block.operations).find(|operation|
        matches!(operation.result, terminal_psi::OperationResult::Scalar(value) if value.id == argument)).unwrap();
    assert!(
        matches!(
            definition.kind,
            terminal_psi::OperationKind::BooleanNot { .. }
                | terminal_psi::OperationKind::BooleanEqual { .. }
                | terminal_psi::OperationKind::IntegerEqual { .. }
                | terminal_psi::OperationKind::IntegerLessThan { .. }
                | terminal_psi::OperationKind::IntegerLessOrEqual { .. }
        ),
        "{actual}: call retains the evaluated operation result, not a substituted literal: {:?}",
        definition.kind
    );
    lowered
}

#[test]
fn computed_boolean_equality_actual_retains_its_evaluated_result() {
    for actual in ["true == true", "false == false"] {
        computed_boolean_actual(actual, "");
    }
}
