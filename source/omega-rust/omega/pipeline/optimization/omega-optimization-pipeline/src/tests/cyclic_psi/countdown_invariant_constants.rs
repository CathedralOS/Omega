//! Optimizer module role: test leaf. Exact ranked-loop invariant constant custody.

use super::*;

use omega_optimization_unit::{ValueDefinitionSite, recompute_psi_optimization_unit_identity};
use omega_optimization_validation::validate_transformed_psi_optimization_unit;
use omega_psi_optimizer::{
    CountdownInvariantConstantAnalysisError, CountdownInvariantConstantAnalysisSnapshot,
    CountdownInvariantConstantRole,
};
use psi_core::{IntegerSign, IntegerType, IntegerValue, MachineId};

#[test]
fn source_countdown_yields_exact_certificate_owned_zero_and_one() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let counted = session
        .counted_loop_analysis()
        .expect("authenticated counted loop");
    let analysis = session
        .countdown_invariant_constant_analysis()
        .expect("authenticated countdown constants");
    let [loop_constants] = analysis.loops() else {
        panic!("one countdown invariant-constant row")
    };
    let [zero, one] = loop_constants.constants.as_slice() else {
        panic!("the ranking relation owns exactly zero and one")
    };
    let [summary] = counted.loops() else {
        panic!("one counted loop")
    };

    assert_eq!(&loop_constants.counted_loop, summary);
    assert_eq!(
        loop_constants.prospective_preheader,
        summary.preheader_edge.source
    );
    assert_eq!(zero.role, CountdownInvariantConstantRole::PositiveGuardZero);
    assert_eq!(zero.psi_operation, summary.certificate.guard.zero_operation);
    assert_eq!(zero.result, summary.certificate.guard.zero);
    assert_eq!(zero.scalar_type, summary.certificate.rank_type);
    assert_eq!(zero.value, IntegerValue::Unsigned(0));
    assert_eq!(zero.location.block, summary.certificate.header);
    assert_eq!(zero.definition.value, zero.result);
    assert_eq!(
        zero.definition.site,
        ValueDefinitionSite::Node {
            block: zero.location.block,
            node: zero.location.node,
        }
    );
    assert_eq!(
        one.role,
        CountdownInvariantConstantRole::BackedgeDecrementOne
    );
    assert_eq!(one.psi_operation, summary.certificate.descent.one_operation);
    assert_eq!(one.result, summary.certificate.descent.one);
    assert_eq!(one.scalar_type, summary.certificate.rank_type);
    assert_eq!(one.value, IntegerValue::Unsigned(1));
    assert_eq!(
        one.location.block,
        summary.certificate.descent.backedge.source
    );
    assert_ne!(zero.effect, one.effect);
}

#[test]
fn invariant_constant_snapshot_replay_rejects_every_retained_axis() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let baseline = session
        .countdown_invariant_constant_analysis()
        .expect("derive invariant constants")
        .snapshot()
        .clone();
    session
        .validate_countdown_invariant_constant_analysis(&baseline)
        .expect("exact invariant-constant snapshot replays");

    let corruptions: Vec<Box<dyn Fn(&mut CountdownInvariantConstantAnalysisSnapshot)>> = vec![
        Box::new(|snapshot| {
            snapshot.revision =
                omega_optimization_core::OptimizationUnitIdentity::from_canonical_bytes(
                    b"stale invariant-constant revision",
                );
        }),
        Box::new(|snapshot| {
            snapshot.terminal_psi.program_fingerprint =
                psi_terminal::SemanticFingerprint::from_bytes([0xE1; 32]);
        }),
        Box::new(|snapshot| snapshot.loops.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.loops[0].clone();
            snapshot.loops.push(duplicate);
        }),
        Box::new(|snapshot| {
            snapshot.loops[0]
                .counted_loop
                .certificate
                .component
                .internal_edges
                .pop();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].prospective_preheader =
                snapshot.loops[0].counted_loop.certificate.header;
        }),
        Box::new(|snapshot| snapshot.loops[0].constants.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.loops[0].constants[0].clone();
            snapshot.loops[0].constants.push(duplicate);
        }),
        Box::new(|snapshot| snapshot.loops[0].constants.reverse()),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].role =
                CountdownInvariantConstantRole::BackedgeDecrementOne;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].location.machine = MachineId::new(98_001).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].location.block =
                snapshot.loops[0].constants[1].location.block;
        }),
        Box::new(|snapshot| snapshot.loops[0].constants[0].location.node += 1),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].psi_operation = snapshot.loops[0]
                .counted_loop
                .certificate
                .guard
                .comparison_operation;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].result = snapshot.loops[0].constants[1].result;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].scalar_type =
                IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].value = IntegerValue::Unsigned(1);
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].definition.value =
                snapshot.loops[0].constants[1].definition.value;
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].definition.scalar_type =
                psi_core::ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
        }),
        Box::new(|snapshot| {
            snapshot.loops[0].constants[0].definition.site =
                snapshot.loops[0].constants[1].definition.site;
        }),
        Box::new(|snapshot| snapshot.loops[0].constants[0].provenance.clear()),
        Box::new(|snapshot| snapshot.loops[0].constants[0].fuel[0].units += 1),
        Box::new(|snapshot| snapshot.loops[0].constants[0].effect.input += 1),
        Box::new(|snapshot| snapshot.loops[0].constants[0].effect.output += 1),
    ];
    assert_eq!(corruptions.len(), 24);
    for mutate in corruptions {
        let mut corrupted = baseline.clone();
        mutate(&mut corrupted);
        assert_eq!(
            session.validate_countdown_invariant_constant_analysis(&corrupted),
            Err(CountdownInvariantConstantAnalysisError::SnapshotMismatch)
        );
    }
}

#[test]
fn acyclic_session_has_no_countdown_invariant_constants() {
    let verified = acyclic_unit();
    let session = VerifiedPsiOptimizationSession::new(verified).expect("verified acyclic session");
    let analysis = session
        .countdown_invariant_constant_analysis()
        .expect("empty cyclic custody is a valid analysis input");
    assert!(analysis.loops().is_empty());
    session
        .validate_countdown_invariant_constant_analysis(analysis.snapshot())
        .expect("empty invariant snapshot replays");
}

#[test]
fn analysis_does_not_authorize_ranked_component_mutation() {
    let (_, verified) = countdown_unit();
    let session =
        VerifiedPsiOptimizationSession::new(verified).expect("verified countdown session");
    let analysis = session
        .countdown_invariant_constant_analysis()
        .expect("analysis-only invariant custody");
    let zero = analysis.loops()[0].constants[0].clone();
    let (input, mut unit) = session.into_parts();
    let node = &mut unit
        .functions
        .iter_mut()
        .find(|function| function.machine == zero.location.machine)
        .unwrap()
        .blocks
        .iter_mut()
        .find(|block| block.id == zero.location.block)
        .unwrap()
        .nodes[usize::try_from(zero.location.node).unwrap()];
    node.fuel[0].units += 1;
    unit.identity = recompute_psi_optimization_unit_identity(&unit);

    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &unit),
        Err(OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
            machine,
            block,
        }) if machine == zero.location.machine && block == zero.location.block
    ));
}

fn acyclic_unit() -> omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit {
    const SOURCE: &str = r#"
        data Root {}
        machine Root::once() {}
    "#;
    let tokens = Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize acyclic unit");
    let syntax = parse_syntax_trees(&tokens).expect("parse acyclic unit");
    let resolved = lower_syntax_trees(&syntax).expect("resolve acyclic unit");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type acyclic unit");
    let checked = lower_typed_trees(typed).expect("check acyclic unit");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::once")
        .expect("lower acyclic unit");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode acyclic semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode acyclic proof");
    let input = lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("admit acyclic optimizer unit");
    build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("build acyclic optimizer unit")
}
