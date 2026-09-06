//! Pre-Terminal Psi optimization entrance regressions.

use super::*;
use optimization::{PsiOptimization, PsiOptimizationSelections};

#[test]
fn empty_selection_executes_validated_identity_before_publication() {
    let lowered =
        lower_machine(&hard_root_checked_fixture(), "Root::enter").expect("fixture lowers");
    let expected = lowered.clone();
    let selections = PsiOptimizationSelections::default();
    let selection_identity = selections.identity();

    let optimized = run_psi_optimization(lowered, selections).expect("identity stage executes");

    assert_eq!(optimized.lowered(), &expected);
    assert!(optimized.selections().is_empty());
    assert_eq!(optimized.execution().selection(), selection_identity);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof()
    );
    let artifact = finalize_terminal_artifact(&optimized).expect("optimized result publishes");
    let direct = terminal_codec::CanonicalTerminalArtifact::from_parts(
        &expected.semantic_module,
        &expected.proof_bundle,
        optimized.execution(),
        expected.debug_map.as_ref(),
    )
    .expect("direct canonical encoding");
    assert_eq!(
        artifact, direct,
        "stage extraction preserves every artifact byte"
    );
    assert_eq!(
        artifact.manifest().semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn every_unported_nonempty_selection_fails_closed() {
    let lowered =
        lower_machine(&hard_root_checked_fixture(), "Root::enter").expect("fixture lowers");

    for optimization in PsiOptimization::ALL {
        if optimization == PsiOptimization::DeadPureScalarElimination {
            continue;
        }
        let selections = PsiOptimizationSelections::new([optimization]).unwrap();
        assert!(matches!(
            run_psi_optimization(lowered.clone(), selections),
            Err(PsiOptimizationStageError::UnsupportedSelection(actual))
                if actual == optimization
        ));
    }
}

fn dead_scalar_fixture() -> LoweredPsi {
    let checked = checked_source("data Main {} machine Main::answer() {}");
    let mut lowered = lower_machine(&checked, "Main::answer").expect("Unit source lowers");
    let first = ValueId::new(2001).unwrap();
    let second = ValueId::new(2002).unwrap();
    lowered.semantic_module.machines[0].blocks[0]
        .operations
        .extend([
            Operation {
                id: OperationId::new(2001).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: first,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
            Operation {
                id: OperationId::new(2002).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    id: second,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand: first },
            },
        ]);
    let identity = terminal_psi_identity(&lowered.semantic_module).unwrap();
    if let Some(debug) = lowered.debug_map.as_mut() {
        debug.semantic = identity;
        let span = debug.sites[0].span;
        debug.sites.extend([
            DebugSite {
                subject: DebugSubject::Operation(OperationId::new(2001).unwrap()),
                span,
            },
            DebugSite {
                subject: DebugSubject::Value(first),
                span,
            },
            DebugSite {
                subject: DebugSubject::Operation(OperationId::new(2002).unwrap()),
                span,
            },
            DebugSite {
                subject: DebugSubject::Value(second),
                span,
            },
        ]);
        debug.sites.sort_by_key(|site| site.subject);
    }
    lowered
}

#[test]
fn selected_dead_scalar_elimination_removes_a_chain_before_portable_publication() {
    let lowered = dead_scalar_fixture();
    let input =
        run_psi_optimization(lowered.clone(), PsiOptimizationSelections::default()).unwrap();
    let original = finalize_terminal_artifact(&input).unwrap();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap(),
    )
    .expect("selected elimination executes");
    assert_eq!(
        optimized.lowered().semantic_module.machines[0].blocks[0]
            .operations
            .len()
            + 2,
        lowered.semantic_module.machines[0].blocks[0]
            .operations
            .len()
    );
    assert_eq!(optimized.lowered().proof_bundle, lowered.proof_bundle);
    assert_eq!(
        optimized.lowered().source_call_occurrences,
        lowered.source_call_occurrences
    );
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    let published = finalize_terminal_artifact(&optimized).unwrap();
    let profile = proof_admission::AdmissionProfile::default();
    let expected = terminal_interpreter::interpret_terminal_artifact(
        original.semantic_bytes(),
        original.proof_bytes(),
        &profile,
        &[],
    )
    .unwrap();
    drop(lowered);
    drop(optimized);
    let actual = terminal_interpreter::interpret_terminal_artifact(
        published.semantic_bytes(),
        published.proof_bytes(),
        &profile,
        &[],
    )
    .unwrap();
    assert_eq!(
        actual, expected,
        "fresh interpretation consumes only published bytes"
    );
}

#[test]
fn independent_dead_scalar_check_rejects_live_removal_and_value_substitution() {
    let before = dead_scalar_fixture().semantic_module;
    let mut after = before.clone();
    after.machines[0].blocks[0].operations.remove(0);
    assert!(terminal_verifier::validate_dead_scalar_elimination(&before, &after).is_err());
    let mut after = before.clone();
    after.machines[0].blocks[0].operations[1].kind =
        OperationKind::BooleanConstant { value: false };
    assert!(matches!(
        terminal_verifier::validate_dead_scalar_elimination(&before, &after),
        Err(terminal_verifier::DeadScalarRewriteError::ChangedSurvivingOperation(_))
    ));
}

#[test]
fn dead_scalar_selection_preserves_proof_questions_and_rejects_unchecked_context_changes() {
    let mut lowered = dead_scalar_fixture();
    lowered.semantic_module.machines[0]
        .contract
        .ensures
        .push(ContractClause {
            obligation: ObligationId::new(2001).unwrap(),
            proposition: Proposition::Truth,
        });
    if let Some(debug) = lowered.debug_map.as_mut() {
        debug.semantic = terminal_psi_identity(&lowered.semantic_module).unwrap();
    }
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap(),
    )
    .expect("proof-bearing closure remains unchanged");
    assert_eq!(optimized.lowered(), &lowered);
    let mut after = lowered.semantic_module.clone();
    after.machines[0].blocks[0].operations.truncate(1);
    assert!(matches!(
        terminal_verifier::validate_dead_scalar_elimination(&lowered.semantic_module, &after),
        Err(terminal_verifier::DeadScalarRewriteError::ChangedProofQuestion)
    ));
}

#[test]
fn invalid_input_fails_before_selected_rule_dispatch() {
    let mut lowered =
        lower_machine(&hard_root_checked_fixture(), "Root::enter").expect("fixture lowers");
    lowered.semantic_module.machines.clear();
    let selections = PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap();

    assert!(matches!(
        run_psi_optimization(lowered, selections),
        Err(PsiOptimizationStageError::InvalidModule(
            terminal_verifier::ModuleError::EmptyModule
        ))
    ));
}

#[test]
fn dead_scalar_check_keeps_effects_and_rejects_their_removal_or_reordering() {
    let lowered = lower_machine(&hard_root_checked_fixture(), "Root::enter").unwrap();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap(),
    )
    .unwrap();
    assert_eq!(optimized.lowered(), &lowered);
    let mut removed = lowered.semantic_module.clone();
    let port = removed.machines[1].blocks[0].operations.remove(0);
    assert!(
        matches!(terminal_verifier::validate_dead_scalar_elimination(
        &lowered.semantic_module, &removed),
        Err(terminal_verifier::DeadScalarRewriteError::RemovedNonTotalOperation(id)) if id == port.id)
    );
    let mut reordered = lowered.semantic_module.clone();
    reordered.machines[1].blocks[0].operations.swap(0, 1);
    assert!(
        terminal_verifier::validate_dead_scalar_elimination(&lowered.semantic_module, &reordered)
            .is_err()
    );
}

#[test]
fn dead_scalar_elimination_keeps_the_transitive_returned_value_chain() {
    let mut lowered = dead_scalar_fixture();
    let machine = &mut lowered.semantic_module.machines[0];
    machine.result = TerminalMachineResult::Scalar(ValueDeclaration {
        id: ValueId::new(2003).unwrap(),
        scalar_type: ScalarType::Boolean,
    });
    machine.blocks[0].terminator = Terminator::Return {
        edge: machine.blocks[0].terminator.edge(),
        value: ValueId::new(2002).unwrap(),
        cleanup_actions: Vec::new(),
    };
    if let Some(debug) = lowered.debug_map.as_mut() {
        debug.semantic = terminal_psi_identity(&lowered.semantic_module).unwrap();
    }
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap(),
    )
    .unwrap();
    assert_eq!(optimized.lowered(), &lowered);
}
