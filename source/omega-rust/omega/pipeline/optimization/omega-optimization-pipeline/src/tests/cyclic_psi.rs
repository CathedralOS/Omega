//! Optimizer module role: stage group. Real source-produced ranked countdown admission through optimizer analyses.

use omega_abstract_operations::AbstractOperation;
use omega_optimization_core::AnalysisKind;
use omega_optimization_validation::{
    OptimizationUnitValidationError, OptimizerCycleComponentSnapshot,
    OptimizerRankingCertificateSnapshot, validate_psi_cycle_component_snapshot,
    validate_psi_optimization_unit, validate_psi_ranking_certificate_snapshot,
    validate_transformed_psi_optimization_unit, validate_verified_psi_cycle_components,
};
use omega_psi_optimizer::{AnalysisManager, AnalysisProduct, VerifiedPsiOptimizationSession};
use omega_psi_to_abstract_operations::{
    VerifiedPsiOptimizationInput, build_verified_psi_optimization_unit,
    lower_artifact_sections_for_optimization,
};
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const COUNTDOWN_SOURCE: &str = r#"
    data Token { value: i32; }
    data Root {}

    machine Root::countdown(token: Token, remaining: u32)
    terminates by remaining -> Nat::Descending;
    {
        transition remaining > 0 {
            true -> countdown(token, remaining - 1)
            _ -> done(token)
        }
        state done(token: Token) {}
    }
"#;

mod countdown_invariant_constant_placement;
mod countdown_invariant_constant_relocation;
mod countdown_invariant_constants;
mod counted_loop_analysis;
mod ranking_relocated_invariant_constants;

fn countdown_input() -> (psi_terminal::TerminalModule, VerifiedPsiOptimizationInput) {
    let tokens = Lexer::new(COUNTDOWN_SOURCE)
        .tokenize()
        .expect("tokenize countdown");
    let syntax = parse_syntax_trees(&tokens).expect("parse countdown");
    let resolved = lower_syntax_trees(&syntax).expect("resolve countdown");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type countdown");
    let checked = lower_typed_trees(typed).expect("check countdown");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::countdown")
        .expect("lower countdown");
    let semantic = psi_terminal_codec::encode_module(&lowered.semantic_module)
        .expect("encode countdown semantics");
    let proof = psi_terminal_codec::encode_proof_bundle(&lowered.proof_bundle)
        .expect("encode countdown proof");
    let input = lower_artifact_sections_for_optimization(
        &semantic,
        &proof,
        &psi_proof_admission::AdmissionProfile::default(),
    )
    .expect("optimizer-only ranked admission");
    (lowered.semantic_module, input)
}

fn countdown_unit() -> (
    psi_terminal::TerminalModule,
    omega_psi_to_abstract_operations::VerifiedPsiOptimizationUnit,
) {
    let (module, input) = countdown_input();
    let verified = build_verified_psi_optimization_unit(
        input,
        psi_terminal_fuel::TerminalFuelSchedule::CURRENT.identity(),
    )
    .expect("build ranked optimizer unit");
    (module, verified)
}

#[test]
fn source_countdown_reaches_all_loop_prerequisite_analyses_without_rewrites() {
    let (module, verified) = countdown_unit();
    assert!(matches!(
        validate_psi_optimization_unit(verified.unit()),
        Err(OptimizationUnitValidationError::ControlCycle { machine, .. })
            if machine == module.entry
    ));

    let session = VerifiedPsiOptimizationSession::new(verified)
        .expect("verified context admits only its exact ranked cycle");
    let unit = session.unit();
    assert_eq!(unit.functions[0].blocks.len(), 4);
    let ranked = module.machines[0]
        .ranked_scc
        .as_ref()
        .expect("source countdown rank");
    let decrement = ranked.covered_cyclic_edges[0].source;
    let [component] = session.cycle_components().components() else {
        panic!("one optimizer cycle component")
    };
    assert_eq!(component.id.machine, module.entry);
    assert_eq!(component.members, vec![ranked.header, decrement]);
    assert_eq!(component.id.internal_edges.len(), 2);
    assert_eq!(component.entries.len(), 1);
    assert_eq!(component.exits.len(), 1);
    assert!(component.id.internal_edges.iter().any(|edge| {
        edge.edge == ranked.covered_cyclic_edges[0].edge
            && edge.source == decrement
            && edge.target == ranked.header
    }));
    let mut analyses = AnalysisManager::new(unit);

    let AnalysisProduct::ControlFlowGraph(cfg) = analyses
        .require(unit, AnalysisKind::ControlFlowGraph)
        .expect("countdown CFG")
    else {
        panic!("CFG product")
    };
    assert_eq!(cfg.functions[0].blocks.len(), 4);

    let AnalysisProduct::Dominators(dominators) = analyses
        .require(unit, AnalysisKind::Dominators)
        .expect("countdown dominators")
    else {
        panic!("dominator product")
    };
    for (block, set) in &dominators.functions[0].1 {
        if *block == ranked.header || *block == decrement {
            assert!(set.contains(&ranked.header));
        }
    }

    let AnalysisProduct::StronglyConnectedComponents(components) = analyses
        .require(unit, AnalysisKind::StronglyConnectedComponents)
        .expect("countdown SCCs")
    else {
        panic!("SCC product")
    };
    assert!(
        components.functions[0]
            .1
            .contains(&vec![ranked.header, decrement])
    );

    let AnalysisProduct::LoopForest(loops) = analyses
        .require(unit, AnalysisKind::LoopForest)
        .expect("countdown loops")
    else {
        panic!("loop product")
    };
    assert!(loops.functions[0].1.iter().any(|region| {
        region.header == Some(ranked.header)
            && region.blocks == vec![ranked.header, decrement]
            && !region.irreducible
    }));

    let AnalysisProduct::ValueLiveness(liveness) = analyses
        .require(unit, AnalysisKind::ValueLiveness)
        .expect("countdown liveness")
    else {
        panic!("liveness product")
    };
    assert!(liveness.blocks.iter().any(|block| {
        block.machine == module.entry
            && block.block == ranked.header
            && block
                .nodes
                .iter()
                .any(|node| node.entry.contains(&ranked.rank_parameter))
    }));
}

#[test]
fn ranked_component_snapshot_replay_rejects_every_topology_axis() {
    let (_, verified) = countdown_unit();
    let validated = validate_verified_psi_cycle_components(&verified)
        .expect("derive canonical optimizer component snapshot");
    let baseline = validated.snapshot().clone();
    let (input, unit) = verified.into_parts();
    validate_psi_cycle_component_snapshot(&input, &unit, &baseline)
        .expect("exact component snapshot replays");

    let corruptions: Vec<Box<dyn Fn(&mut OptimizerCycleComponentSnapshot)>> = vec![
        Box::new(|snapshot| {
            snapshot.terminal_psi.program_fingerprint =
                psi_terminal::SemanticFingerprint::from_bytes([0xA5; 32]);
        }),
        Box::new(|snapshot| {
            snapshot.components[0].id.machine = psi_core::MachineId::new(91_001).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.components[0].id.internal_edges[0].edge =
                snapshot.components[0].entries[0].edge
        }),
        Box::new(|snapshot| {
            snapshot.components[0].id.internal_edges[0].source =
                snapshot.components[0].entries[0].source
        }),
        Box::new(|snapshot| {
            snapshot.components[0].id.internal_edges[0].target =
                snapshot.components[0].exits[0].target
        }),
        Box::new(|snapshot| {
            snapshot.components[0].id.internal_edges.pop();
        }),
        Box::new(|snapshot| snapshot.components[0].id.internal_edges.reverse()),
        Box::new(|snapshot| {
            snapshot.components[0].members.pop();
        }),
        Box::new(|snapshot| snapshot.components[0].members.reverse()),
        Box::new(|snapshot| {
            snapshot.components[0].entries[0].edge =
                snapshot.components[0].id.internal_edges[0].edge
        }),
        Box::new(|snapshot| {
            snapshot.components[0].entries[0].source = snapshot.components[0].members[0]
        }),
        Box::new(|snapshot| {
            snapshot.components[0].entries[0].target = snapshot.components[0].exits[0].target
        }),
        Box::new(|snapshot| snapshot.components[0].entries.clear()),
        Box::new(|snapshot| {
            snapshot.components[0].exits[0].edge = snapshot.components[0].id.internal_edges[0].edge
        }),
        Box::new(|snapshot| {
            snapshot.components[0].exits[0].source = snapshot.components[0].entries[0].source
        }),
        Box::new(|snapshot| {
            snapshot.components[0].exits[0].target = snapshot.components[0].members[0]
        }),
        Box::new(|snapshot| snapshot.components[0].exits.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.components[0].clone();
            snapshot.components.push(duplicate);
        }),
    ];
    for mutate in corruptions {
        let mut corrupted = baseline.clone();
        mutate(&mut corrupted);
        assert_eq!(
            validate_psi_cycle_component_snapshot(&input, &unit, &corrupted),
            Err(OptimizationUnitValidationError::RankedCycleComponentSnapshotMismatch)
        );
    }
}

#[test]
fn ranked_countdown_certificate_replay_rejects_every_evidence_axis() {
    let (_, verified) = countdown_unit();
    let session = VerifiedPsiOptimizationSession::new(verified)
        .expect("derive analysis-only countdown ranking certificate");
    let [certificate] = session.ranking_certificates().certificates() else {
        panic!("one countdown ranking certificate")
    };
    assert_eq!(
        certificate.component,
        session.cycle_components().components()[0].id
    );
    assert_eq!(certificate.header, certificate.guard.block);
    assert_eq!(certificate.rank_parameter, certificate.guard.parameter);
    assert_eq!(
        certificate.rank_parameter,
        certificate.descent.source_parameter
    );
    assert_eq!(
        certificate.rank_parameter,
        certificate.descent.target_parameter
    );
    assert_eq!(certificate.lower_bound, psi_core::IntegerValue::Unsigned(0));
    assert!(
        certificate
            .component
            .internal_edges
            .iter()
            .any(|edge| edge.edge == certificate.guard.edge)
    );

    let baseline = session.ranking_certificates().snapshot().clone();
    let (input, unit) = session.into_parts();
    validate_psi_ranking_certificate_snapshot(&input, &unit, &baseline)
        .expect("exact ranking certificate replays");
    let corruptions: Vec<Box<dyn Fn(&mut OptimizerRankingCertificateSnapshot)>> = vec![
        Box::new(|snapshot| {
            snapshot.terminal_psi.program_fingerprint =
                psi_terminal::SemanticFingerprint::from_bytes([0xC7; 32]);
        }),
        Box::new(|snapshot| snapshot.certificates.clear()),
        Box::new(|snapshot| {
            let duplicate = snapshot.certificates[0].clone();
            snapshot.certificates.push(duplicate);
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].component.internal_edges.pop();
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].header = snapshot.certificates[0].descent.backedge.source;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].rank_parameter = snapshot.certificates[0].guard.zero;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].rank_type =
                psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap();
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].lower_bound = psi_core::IntegerValue::Unsigned(1);
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].upper_bound = psi_core::IntegerValue::Unsigned(1);
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.block = snapshot.certificates[0].descent.backedge.source;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.edge = snapshot.certificates[0].descent.backedge.edge;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.condition = snapshot.certificates[0].guard.zero;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.parameter = snapshot.certificates[0].guard.zero;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.zero = snapshot.certificates[0].rank_parameter;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.zero_operation =
                snapshot.certificates[0].guard.comparison_operation;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].guard.comparison_operation =
                snapshot.certificates[0].guard.zero_operation;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.backedge.edge = snapshot.certificates[0].guard.edge;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.backedge.source = snapshot.certificates[0].header;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.backedge.target =
                snapshot.certificates[0].descent.backedge.source;
        }),
        Box::new(|snapshot| snapshot.certificates[0].descent.argument_index += 1),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.argument = snapshot.certificates[0].descent.one;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.source_parameter =
                snapshot.certificates[0].descent.one;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.target_parameter =
                snapshot.certificates[0].descent.one;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.one = snapshot.certificates[0].rank_parameter;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.one_operation =
                snapshot.certificates[0].descent.subtract_operation;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.subtract_operation =
                snapshot.certificates[0].descent.one_operation;
        }),
        Box::new(|snapshot| {
            snapshot.certificates[0].descent.subtract_obligation =
                psi_core::ObligationId::new(99_901).unwrap();
        }),
    ];
    for mutate in corruptions {
        let mut corrupted = baseline.clone();
        mutate(&mut corrupted);
        assert_eq!(
            validate_psi_ranking_certificate_snapshot(&input, &unit, &corrupted),
            Err(OptimizationUnitValidationError::RankedCycleRankingCertificateSnapshotMismatch)
        );
    }
}

#[test]
fn ranked_context_rejects_topology_and_frozen_body_corruption() {
    let (module, verified) = countdown_unit();
    let ranked = module.machines[0].ranked_scc.as_ref().unwrap();
    let decrement = ranked.covered_cyclic_edges[0].source;
    let preheader = module.machines[0].entry;
    let (input, original) = verified.into_parts();

    let mut topology = original.clone();
    let block = topology.functions[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == decrement)
        .unwrap();
    let node = block.nodes.last_mut().unwrap();
    let AbstractOperation::Jump { target, .. } = &mut node.operation else {
        panic!("countdown backedge is a jump")
    };
    *target = preheader;
    node.successors[0].target = preheader;
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &topology),
        Err(OptimizationUnitValidationError::RankedCycleTopologyMismatch { machine })
            if machine == module.entry
    ));

    let mut frozen = original;
    let block = frozen.functions[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == ranked.header)
        .unwrap();
    block.nodes[0].fuel[0].units += 1;
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &frozen),
        Err(OptimizationUnitValidationError::RankedCycleFrozenBlockMismatch {
            machine,
            block,
        }) if machine == module.entry && block == ranked.header
    ));
}
