use psi_proof_kernel::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, StructuralPathSegment, Terminator};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    data Token { value: u64; }
    data Quintet {
        first: Token;
        second: Token;
        third: Token;
        fourth: Token;
        fifth: Token;
    }

    data Sink {}
    machine Sink::take(token: Token) {}

    data Root {}
    machine Root::enter(value: Quintet) {
        Sink::take(value.third);
    }
"#;

#[test]
fn direct_field_partial_affine_cleanup_crosses_source_codec_verifier_and_interpreter() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct field transfer plus residual affine cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("partial affine source slice has one structural root")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("partial affine source slice has one block")
    };
    let [call] = block.operations.as_slice() else {
        panic!("partial affine source slice has one projected call")
    };
    let OperationKind::CallUnit {
        structural_arguments,
        claim_transfers,
        ..
    } = &call.kind
    else {
        panic!("expected direct Unit call")
    };
    assert!(claim_transfers.is_empty());
    assert!(matches!(
        structural_arguments.as_slice(),
        [argument]
            if argument.place == root.place
                && argument.path == [StructuralPathSegment::Field("third".into())]
    ));

    let Terminator::ReturnUnitPartialAffine {
        edge,
        trivial_affine_discards,
        residual_affine_discards,
    } = &block.terminator
    else {
        panic!("expected path-sensitive Unit return")
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|residual| {
                assert_eq!(residual.place, root.place);
                residual.path.clone()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("fifth".into())],
            vec![StructuralPathSegment::Field("fourth".into())],
            vec![StructuralPathSegment::Field("second".into())],
            vec![StructuralPathSegment::Field("first".into())],
        ]
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs moved field plus residual root exhaustion");
    let semantic = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&semantic).expect("semantic module decodes"),
        lowered.semantic_module,
        "the partial frontier is canonical artifact identity"
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");

    let argument = TerminalStructuralValue {
        opaque_identity: 0x5041_4952,
        structural_type: root.structural_type,
        qualifications: root.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified source artifact starts");

    // The projected call and callee return commit first. With no remaining
    // allowance, the caller suspends at its return edge while every residual
    // remains live; cleanup commits only after replenishment.
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execution suspends cleanly"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(*edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    let live_residuals = execution
        .live_affine_frontier()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(live_residuals.len(), residual_affine_discards.len());
    assert!(
        residual_affine_discards
            .iter()
            .all(|residual| live_residuals.contains(residual))
    );

    meter.replenish(1).expect("replenish return-edge fuel");
    assert_eq!(
        execution.resume(&mut meter).expect("execution completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 3);
}
