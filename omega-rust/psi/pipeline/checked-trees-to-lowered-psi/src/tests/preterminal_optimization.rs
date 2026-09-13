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
                static_reach_binding: None,
                id: OperationId::new(2001).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: first,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
            Operation {
                static_reach_binding: None,
                id: OperationId::new(2002).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
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
        qualifications: Default::default(),
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

fn dead_block_parameter_fixture() -> LoweredPsi {
    let checked = checked_source(
        "data Main { value: i32; }\n\
         machine Main::compute(a: i32, b: i32) -> i32 {\n\
             let unused: bool = a < b;\n\
             let dead_const: i32 = 7;\n\
             transition {\n\
                 a == b -> (1)\n\
                 _ -> (2)\n\
             }\n\
         }\n\
         machine Main::main(&mut self) {\n\
             self.value = Main::compute(1, 2);\n\
         }\n",
    );
    lower_machine(&checked, "Main::compute").expect("transition source lowers")
}

#[test]
fn selected_dead_scalar_elimination_removes_unused_block_parameters_and_edge_arguments() {
    let lowered = dead_block_parameter_fixture();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap(),
    )
    .expect("selected elimination executes");
    let before = &lowered.semantic_module;
    let after = &optimized.lowered().semantic_module;
    terminal_verifier::validate_dead_scalar_elimination(before, after)
        .expect("the independent check accepts the rewrite");
    let machine = &after.machines[0];
    for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
        assert!(
            !matches!(operation.kind, OperationKind::IntegerLessThan { .. }),
            "the dead comparison is removed"
        );
        assert!(
            !matches!(
                operation.kind,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Signed(7)
                }
            ),
            "the dead constant is removed"
        );
    }
    let conditional = machine
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Conditional { .. }))
        .expect("the equality dispatch survives");
    let Terminator::Conditional {
        condition,
        when_true,
        when_false,
    } = &conditional.terminator
    else {
        unreachable!()
    };
    assert!(
        conditional.operations.iter().any(|operation| matches!(
            operation.kind,
            OperationKind::IntegerEqual { .. }
        ) && operation
            .result
            .scalar()
            .is_some_and(|result| result.id == *condition)),
        "the conditional still reads its IntegerEqual condition"
    );
    let old_machine = &before.machines[0];
    for edge in [when_true, when_false] {
        let old_block = old_machine
            .blocks
            .iter()
            .find(|block| block.id == edge.target)
            .unwrap();
        let new_block = machine
            .blocks
            .iter()
            .find(|block| block.id == edge.target)
            .unwrap();
        // The forwarded `unused` and `dead_const` bindings die, and the
        // forwarded `a`/`b` copies die transitively through the merge block.
        assert_eq!(new_block.parameters.len() + 4, old_block.parameters.len());
        assert_eq!(edge.arguments.len(), new_block.parameters.len());
        let old_edge_arguments = old_machine
            .blocks
            .iter()
            .find_map(|block| match &block.terminator {
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => [when_true, when_false]
                    .into_iter()
                    .find(|successor| successor.edge == edge.edge)
                    .map(|successor| successor.arguments.clone()),
                _ => None,
            })
            .expect("the same edge exists before the rewrite");
        assert_eq!(edge.arguments.len() + 4, old_edge_arguments.len());
    }
    let old_parameters: usize = old_machine
        .blocks
        .iter()
        .map(|block| block.parameters.len())
        .sum();
    let new_parameters: usize = machine
        .blocks
        .iter()
        .map(|block| block.parameters.len())
        .sum();
    assert!(
        new_parameters < old_parameters,
        "dead block parameters are removed module-wide"
    );
}

#[test]
fn dead_scalar_check_rejects_mismatched_parameter_and_edge_argument_removal() {
    let before = dead_block_parameter_fixture().semantic_module;
    let machine = &before.machines[0];
    // The merge block declares five parameters and is reached by two jumps.
    let merge = machine
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Return { .. }))
        .expect("merge block");
    // Removing an unused parameter while dropping a different argument
    // position keeps arity and scalar types consistent (both neighbors are
    // i32), so the independent check itself must reject the misaligned edge.
    let mut after = before.clone();
    after.machines[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == merge.id)
        .unwrap()
        .parameters
        .remove(0);
    for block in &mut after.machines[0].blocks {
        if let Terminator::Jump {
            target, arguments, ..
        } = &mut block.terminator
            && *target == merge.id
        {
            arguments.remove(1);
        }
    }
    let result = terminal_verifier::validate_dead_scalar_elimination(&before, &after);
    assert!(
        matches!(
            result,
            Err(terminal_verifier::DeadScalarRewriteError::ChangedEdgeArguments(_))
        ),
        "expected ChangedEdgeArguments, observed {result:?}"
    );
    // Declaring a surviving parameter with a different declaration is also
    // rejected even when every edge stays aligned.
    let mut after = before.clone();
    let retained = after.machines[0]
        .blocks
        .iter_mut()
        .find(|block| block.id == merge.id)
        .unwrap();
    retained.parameters[0].scalar_type = ScalarType::Boolean;
    let result = terminal_verifier::validate_dead_scalar_elimination(&before, &after);
    assert!(
        result.is_err(),
        "a mutated surviving parameter declaration must reject: {result:?}"
    );
}
