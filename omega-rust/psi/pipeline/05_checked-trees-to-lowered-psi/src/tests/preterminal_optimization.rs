//! Pre-Terminal Psi optimization entrance regressions.
use super::{ScalarType, TerminalMachineResult, hard_root_checked_fixture, lower_machine};
use crate::TerminalMachineSelection;
use lowered_psi::LoweredPsi;
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use lowered_psi_to_terminal_psi::finalize_terminal_artifact;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use semantic_vocabulary::{IntegerValue, ObligationId, OperationId, Proposition, ValueId};
use std::collections::BTreeSet;
use terminal_codec::{DebugSite, DebugSubject, terminal_psi_identity};
use terminal_psi::{
    ContractClause, Operation, OperationKind, OperationResult, Terminator, ValueDeclaration,
};

#[test]
fn empty_selection_executes_validated_identity_before_publication() {
    let lowered = lower_machine(
        &hard_root_checked_fixture(),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("fixture lowers");
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
fn every_nonempty_selection_executes_before_publication() {
    // The selection catalog is fully ported: every named rule executes over a
    // real lowered module and records itself in the execution it returns.
    let lowered = lower_machine(
        &hard_root_checked_fixture(),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("fixture lowers");

    for optimization in PsiOptimization::ALL {
        let selections = PsiOptimizationSelections::new([optimization]).unwrap();
        let optimized = run_psi_optimization(lowered.clone(), selections)
            .unwrap_or_else(|error| panic!("{optimization:?} must execute: {error:?}"));
        assert_eq!(
            optimized.execution().selections().as_slice(),
            &[optimization],
            "{optimization:?} must be the recorded selection"
        );
    }
}

fn dead_scalar_fixture() -> LoweredPsi {
    let checked = crate::front_end::checked_program("data Main {} machine Main::answer() {}");
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::answer"))
        .expect("Unit source lowers");
    let first = ValueId::new(2001).unwrap();
    let second = ValueId::new(2002).unwrap();
    lowered.semantic_module.machines[0].blocks[0]
        .operations
        .extend([
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
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
                suspension_crossing: None,
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
fn selected_sccp_folds_literal_leaves_before_portable_publication() {
    let lowered = dead_scalar_fixture();
    let input =
        run_psi_optimization(lowered.clone(), PsiOptimizationSelections::default()).unwrap();
    let original = finalize_terminal_artifact(&input).unwrap();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::SparseConditionalConstantPropagation])
            .unwrap(),
    )
    .expect("selected constant folding executes");
    let operations = &optimized.lowered().semantic_module.machines[0].blocks[0].operations;
    assert_eq!(
        operations.len(),
        lowered.semantic_module.machines[0].blocks[0]
            .operations
            .len(),
        "folding rewrites in place: every operation row survives"
    );
    assert!(
        operations.iter().any(|operation| matches!(
            operation.kind,
            OperationKind::BooleanConstant { value: false }
        ) && operation
            .result
            .scalar()
            .is_some_and(|result| result.id == ValueId::new(2002).unwrap())),
        "the BooleanNot over a literal folds to its BooleanConstant denotation"
    );
    assert_eq!(optimized.lowered().proof_bundle, lowered.proof_bundle);
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    terminal_verifier::validate_sparse_conditional_constant_propagation(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
    let published = finalize_terminal_artifact(&optimized).unwrap();
    assert_eq!(
        published.manifest().semantic(),
        optimized.execution().output_semantic()
    );
    let profile = proof_admission::AdmissionProfile::default();
    let expected = terminal_interpreter::interpret_terminal_artifact(
        original.semantic_bytes(),
        original.proof_bytes(),
        &profile,
        &[],
    )
    .unwrap();
    let actual = terminal_interpreter::interpret_terminal_artifact(
        published.semantic_bytes(),
        published.proof_bytes(),
        &profile,
        &[],
    )
    .unwrap();
    assert_eq!(
        actual, expected,
        "the folded artifact interprets identically from published bytes"
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
fn dead_scalar_selection_rewrites_where_the_reconstructed_question_does_not_reach() {
    // The refusal boundary is the reconstructed question, not obligation
    // presence: an obligation owned by the callee keeps that machine's exit
    // axioms, so a dead chain in the caller still leaves while the clause and
    // the bundle survive verbatim.
    let mut lowered = lower_machine(
        &hard_root_checked_fixture(),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("fixture lowers");
    lowered.semantic_module.machines[1]
        .contract
        .ensures
        .push(ContractClause {
            obligation: ObligationId::new(4242).unwrap(),
            proposition: Proposition::Truth,
        });
    let dead = Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: OperationId::new(4242).unwrap(),
        result: OperationResult::Scalar(ValueDeclaration {
            qualifications: Default::default(),
            id: ValueId::new(4242).unwrap(),
            scalar_type: ScalarType::Boolean,
        }),
        kind: OperationKind::BooleanConstant { value: true },
    };
    lowered.semantic_module.machines[0].blocks[0]
        .operations
        .push(dead.clone());
    if let Some(debug) = lowered.debug_map.as_mut() {
        debug.semantic = terminal_psi_identity(&lowered.semantic_module).unwrap();
    }
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::DeadPureScalarElimination]).unwrap(),
    )
    .expect("a rewrite the reconstructed question cannot see still executes");
    assert!(
        !optimized.lowered().semantic_module.machines[0].blocks[0]
            .operations
            .iter()
            .any(|operation| operation.id == dead.id),
        "the proof-disjoint dead operation leaves the caller"
    );
    assert_eq!(
        optimized.lowered().semantic_module.machines[1]
            .contract
            .ensures,
        lowered.semantic_module.machines[1].contract.ensures,
        "the obligation survives verbatim"
    );
    assert_eq!(
        optimized.lowered().proof_bundle,
        lowered.proof_bundle,
        "the proof bundle is byte-identical"
    );
}

#[test]
fn invalid_input_fails_before_selected_rule_dispatch() {
    let mut lowered = lower_machine(
        &hard_root_checked_fixture(),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("fixture lowers");
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
    let lowered = lower_machine(
        &hard_root_checked_fixture(),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .unwrap();
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

/// A dead comparison, a dead constant, four forwarded copies per arm, and a
/// merge parameter bound to a different constant on each incoming edge: the
/// shape the rewrite tests below consume.
///
/// The arm constants are the declared type's endpoints on purpose. A join whose
/// actual scalar arrivals are all same-type literals proposes their interval
/// hull as a scalar block invariant, and a retained invariant makes the module
/// proof-bearing: both rewrites then refuse, because removing a header
/// parameter or its arrival arguments cannot carry the reconstructed question
/// verbatim. A hull equal to the whole declared type is no narrower than the
/// parameter's own type, so no invariant is proposed and this fixture stays in
/// the non-proof-bearing lane these tests exercise. The refusal itself has its
/// own coverage in `dead_scalar_selection_preserves_proof_questions_and_rejects_unchecked_context_changes`
/// and `copy_propagation_preserves_proof_questions_and_keeps_proof_bearing_identities`.
fn dead_block_parameter_fixture() -> LoweredPsi {
    let checked = crate::front_end::checked_program(
        "data Main { value: i32; }\n\
         machine Main::compute(a: i32, b: i32) -> i32 {\n\
             let unused: bool = a < b;\n\
             let dead_const: i32 = 7;\n\
             transition {\n\
                 a == b -> (-2147483648)\n\
                 _ -> (2147483647)\n\
             }\n\
         }\n\
         machine Main::main(&mut self) {\n\
             self.value = Main::compute(1, 2);\n\
         }\n",
    );
    lower_machine(&checked, TerminalMachineSelection::Name("Main::compute"))
        .expect("transition source lowers")
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
fn selected_copy_propagation_collapses_forwarded_copies_and_edge_arguments() {
    let lowered = dead_block_parameter_fixture();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::CopyPropagation]).unwrap(),
    )
    .expect("selected copy propagation executes");
    let before = &lowered.semantic_module;
    let after = &optimized.lowered().semantic_module;
    terminal_verifier::validate_copy_propagation(before, after)
        .expect("the independent check accepts the rewrite");
    let old_machine = &before.machines[0];
    let machine = &after.machines[0];
    // Every forwarded copy collapses transitively: the merge block keeps only
    // the parameter bound to a different constant on each arm, and the
    // dispatch block's equality reads the machine parameters directly.
    let conditional = machine
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Conditional { .. }))
        .expect("the equality dispatch survives");
    assert!(conditional.parameters.is_empty());
    let equal = conditional
        .operations
        .iter()
        .find_map(|operation| match &operation.kind {
            OperationKind::IntegerEqual { left, right } => Some((*left, *right)),
            _ => None,
        })
        .expect("the dispatch condition survives");
    assert_eq!(equal, (machine.parameters[0].id, machine.parameters[1].id));
    let merge = machine
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Return { .. }))
        .expect("merge block");
    let old_merge = old_machine
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Return { .. }))
        .unwrap();
    assert_eq!(merge.parameters.len() + 4, old_merge.parameters.len());
    let Terminator::Return { value, .. } = &merge.terminator else {
        unreachable!()
    };
    assert!(
        merge
            .parameters
            .iter()
            .any(|parameter| parameter.id == *value),
        "the non-uniform result parameter is retained"
    );
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
    assert_eq!(new_parameters + 16, old_parameters);
    assert_eq!(optimized.lowered().proof_bundle, lowered.proof_bundle);
    assert_eq!(
        optimized.lowered().source_call_occurrences,
        lowered.source_call_occurrences
    );
    if let Some(debug) = optimized.lowered().debug_map.as_ref() {
        let removed = old_machine
            .blocks
            .iter()
            .flat_map(|old_block| {
                let retained = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == old_block.id)
                    .unwrap()
                    .parameters
                    .iter()
                    .map(|parameter| parameter.id)
                    .collect::<BTreeSet<_>>();
                old_block
                    .parameters
                    .iter()
                    .filter(move |parameter| !retained.contains(&parameter.id))
                    .map(|parameter| parameter.id)
                    .collect::<Vec<_>>()
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(removed.len(), 16);
        for site in &debug.sites {
            if let DebugSubject::Value(value) = site.subject {
                assert!(!removed.contains(&value));
            }
        }
        assert_eq!(
            debug.semantic,
            terminal_psi_identity(&optimized.lowered().semantic_module).unwrap()
        );
    }
    let published = finalize_terminal_artifact(&optimized).expect("optimized result publishes");
    assert_eq!(
        published.manifest().semantic(),
        optimized.execution().output_semantic()
    );
    let reconverged = run_psi_optimization(
        optimized.lowered().clone(),
        PsiOptimizationSelections::new([PsiOptimization::CopyPropagation]).unwrap(),
    )
    .expect("the optimized artifact is a legal second input");
    assert_eq!(
        reconverged.lowered(),
        optimized.lowered(),
        "a second selection is the identity: no copy parameter remains"
    );
}

#[test]
fn independent_copy_propagation_check_rejects_noncopy_removal_and_substitution() {
    let before = dead_block_parameter_fixture().semantic_module;
    let machine = &before.machines[0];
    let merge = machine
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Return { .. }))
        .expect("merge block");
    // The merge block's result parameter binds a different constant on each
    // incoming edge. Removing it while keeping a valid module is still not a
    // copy rewrite.
    let mut after = before.clone();
    let position = after.machines[0]
        .blocks
        .iter()
        .find(|block| block.id == merge.id)
        .unwrap()
        .parameters
        .len()
        - 1;
    let machine = &mut after.machines[0];
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == merge.id)
        .unwrap()
        .parameters
        .remove(position);
    let substitute = machine.parameters[0].id;
    for block in &mut machine.blocks {
        block.terminator.map_scalar_uses(&mut |value| {
            if value == merge.parameters[position].id {
                substitute
            } else {
                value
            }
        });
        if let Terminator::Jump {
            target, arguments, ..
        } = &mut block.terminator
            && *target == merge.id
        {
            arguments.remove(position);
        }
    }
    assert!(matches!(
        terminal_verifier::validate_copy_propagation(&before, &after),
        Err(terminal_verifier::CopyPropagationRewriteError::RemovedNonCopyParameter(
            block
        )) if block == merge.id
    ));
    // A removed copy parameter must also substitute the exact resolved source:
    // replacing it with any other live value is not the declared relation.
    let dispatch = before.machines[0]
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Conditional { .. }))
        .expect("dispatch block");
    let mut after = before.clone();
    let machine = &mut after.machines[0];
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == dispatch.id)
        .unwrap()
        .parameters
        .remove(0);
    let wrong = machine.parameters[1].id;
    let removed = dispatch.parameters[0].id;
    for block in &mut machine.blocks {
        for operation in &mut block.operations {
            operation.kind.map_scalar_uses(&mut |value| {
                if value == removed { wrong } else { value }
            });
        }
        block.terminator.map_scalar_uses(&mut |value| {
            if value == removed { wrong } else { value }
        });
        match &mut block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == dispatch.id => {
                arguments.remove(0);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    if edge.target == dispatch.id {
                        edge.arguments.remove(0);
                    }
                }
            }
            _ => {}
        }
    }
    assert!(matches!(
        terminal_verifier::validate_copy_propagation(&before, &after),
        Err(terminal_verifier::CopyPropagationRewriteError::ChangedMachine(
            machine_id
        )) if machine_id == before.machines[0].id
    ));
}

#[test]
fn copy_propagation_preserves_proof_questions_and_keeps_proof_bearing_identities() {
    let mut lowered = dead_block_parameter_fixture();
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
        PsiOptimizationSelections::new([PsiOptimization::CopyPropagation]).unwrap(),
    )
    .expect("proof-bearing closure remains unchanged");
    assert_eq!(optimized.lowered(), &lowered);
    // Removing a copy parameter inside a proof-bearing machine is rejected
    // independently even when the resulting module is internally consistent.
    let before = dead_block_parameter_fixture().semantic_module;
    let mut after = before.clone();
    after.machines[0].contract.ensures.push(ContractClause {
        obligation: ObligationId::new(2001).unwrap(),
        proposition: Proposition::Truth,
    });
    let dispatch = after.machines[0]
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Terminator::Conditional { .. }))
        .unwrap()
        .id;
    let machine = &mut after.machines[0];
    machine
        .blocks
        .iter_mut()
        .find(|block| block.id == dispatch)
        .unwrap()
        .parameters
        .remove(0);
    let source = machine.parameters[0].id;
    let removed = before.machines[0]
        .blocks
        .iter()
        .find(|block| block.id == dispatch)
        .unwrap()
        .parameters[0]
        .id;
    for block in &mut machine.blocks {
        for operation in &mut block.operations {
            operation.kind.map_scalar_uses(&mut |value| {
                if value == removed { source } else { value }
            });
        }
        block.terminator.map_scalar_uses(&mut |value| {
            if value == removed { source } else { value }
        });
        match &mut block.terminator {
            Terminator::Jump {
                target, arguments, ..
            } if *target == dispatch => {
                arguments.remove(0);
            }
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for edge in [when_true, when_false] {
                    if edge.target == dispatch {
                        edge.arguments.remove(0);
                    }
                }
            }
            _ => {}
        }
    }
    assert!(
        terminal_verifier::validate_copy_propagation(&before, &after).is_err(),
        "a context change outside the copy relation must reject"
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

fn global_value_numbering_fixture() -> LoweredPsi {
    let checked = crate::front_end::checked_program("data Main {} machine Main::answer() {}");
    let mut lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::answer"))
        .expect("Unit source lowers");
    let operand = ValueId::new(2001).unwrap();
    let first = ValueId::new(2002).unwrap();
    let duplicate = ValueId::new(2003).unwrap();
    let consumer = ValueId::new(2004).unwrap();
    lowered.semantic_module.machines[0].blocks[0]
        .operations
        .extend([
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(2001).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: operand,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanConstant { value: true },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(2002).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: first,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(2003).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: duplicate,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand },
            },
            Operation {
                static_reach_binding: None,
                suspension_crossing: None,
                id: OperationId::new(2004).unwrap(),
                result: OperationResult::Scalar(ValueDeclaration {
                    qualifications: Default::default(),
                    id: consumer,
                    scalar_type: ScalarType::Boolean,
                }),
                kind: OperationKind::BooleanNot { operand: duplicate },
            },
        ]);
    let identity = terminal_psi_identity(&lowered.semantic_module).unwrap();
    if let Some(debug) = lowered.debug_map.as_mut() {
        debug.semantic = identity;
        let span = debug.sites[0].span;
        debug.sites.extend([
            DebugSite {
                subject: DebugSubject::Operation(OperationId::new(2003).unwrap()),
                span,
            },
            DebugSite {
                subject: DebugSubject::Value(duplicate),
                span,
            },
        ]);
        debug.sites.sort_by_key(|site| site.subject);
    }
    lowered
}

#[test]
fn selected_global_value_numbering_collapses_dominating_duplicates_before_publication() {
    let lowered = global_value_numbering_fixture();
    let input =
        run_psi_optimization(lowered.clone(), PsiOptimizationSelections::default()).unwrap();
    let original = finalize_terminal_artifact(&input).unwrap();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::GlobalValueNumbering]).unwrap(),
    )
    .expect("selected numbering executes");
    let before = &lowered.semantic_module;
    let after = &optimized.lowered().semantic_module;
    terminal_verifier::validate_global_value_numbering(before, after)
        .expect("the independent check accepts the rewrite");
    let block = &after.machines[0].blocks[0];
    assert!(
        block
            .operations
            .iter()
            .all(|operation| operation.id != OperationId::new(2003).unwrap()),
        "the same-block duplicate is removed"
    );
    let consumer = block
        .operations
        .iter()
        .find(|operation| operation.id == OperationId::new(2004).unwrap())
        .expect("the duplicate's consumer survives");
    assert_eq!(
        consumer.kind,
        OperationKind::BooleanNot {
            operand: ValueId::new(2002).unwrap()
        },
        "the canonical survivor substitutes the duplicate's uses"
    );
    assert_eq!(optimized.lowered().proof_bundle, lowered.proof_bundle);
    assert_eq!(
        optimized.lowered().source_call_occurrences,
        lowered.source_call_occurrences
    );
    if let Some(debug) = optimized.lowered().debug_map.as_ref() {
        for site in &debug.sites {
            assert_ne!(
                site.subject,
                DebugSubject::Operation(OperationId::new(2003).unwrap())
            );
            assert_ne!(
                site.subject,
                DebugSubject::Value(ValueId::new(2003).unwrap())
            );
        }
        assert_eq!(
            debug.semantic,
            terminal_psi_identity(&optimized.lowered().semantic_module).unwrap()
        );
    }
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    let published = finalize_terminal_artifact(&optimized).expect("optimized result publishes");
    assert_eq!(
        published.manifest().semantic(),
        optimized.execution().output_semantic()
    );
    let profile = proof_admission::AdmissionProfile::default();
    let expected = terminal_interpreter::interpret_terminal_artifact(
        original.semantic_bytes(),
        original.proof_bytes(),
        &profile,
        &[],
    )
    .unwrap();
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
    let reconverged = run_psi_optimization(
        optimized.lowered().clone(),
        PsiOptimizationSelections::new([PsiOptimization::GlobalValueNumbering]).unwrap(),
    )
    .expect("the optimized artifact is a legal second input");
    assert_eq!(
        reconverged.lowered(),
        optimized.lowered(),
        "a second selection is the identity: no duplicate remains"
    );
}

#[test]
fn independent_global_value_numbering_check_rejects_unjustified_removal_and_substitution() {
    let before = global_value_numbering_fixture().semantic_module;
    // Removing a unique computation has no dominating equivalent to justify it.
    let mut after = before.clone();
    after.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id != OperationId::new(2004).unwrap());
    assert!(matches!(
        terminal_verifier::validate_global_value_numbering(&before, &after),
        Err(
            terminal_verifier::GlobalValueNumberingRewriteError::MissingDominatingEquivalent(id)
        ) if id == OperationId::new(2004).unwrap()
    ));
    // A justified removal must substitute the re-derived canonical survivor:
    // replacing the duplicate with any other live value is a different program.
    let mut after = before.clone();
    after.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id != OperationId::new(2003).unwrap());
    let removed = ValueId::new(2003).unwrap();
    let wrong = ValueId::new(2001).unwrap();
    for block in &mut after.machines[0].blocks {
        for operation in &mut block.operations {
            operation.kind.map_scalar_uses(&mut |value| {
                if value == removed { wrong } else { value }
            });
        }
        block.terminator.map_scalar_uses(&mut |value| {
            if value == removed { wrong } else { value }
        });
    }
    assert!(matches!(
        terminal_verifier::validate_global_value_numbering(&before, &after),
        Err(terminal_verifier::GlobalValueNumberingRewriteError::ChangedMachine(id))
            if id == before.machines[0].id
    ));
}

#[test]
fn global_value_numbering_preserves_proof_questions_and_keeps_proof_bearing_identities() {
    let mut lowered = global_value_numbering_fixture();
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
        PsiOptimizationSelections::new([PsiOptimization::GlobalValueNumbering]).unwrap(),
    )
    .expect("proof-bearing closure remains unchanged");
    assert_eq!(optimized.lowered(), &lowered);
    // The independent check still rejects relation violations inside a
    // proof-bearing module: a removed unique computation is not a duplicate
    // rewrite even when every identity is otherwise consistent.
    let mut after = lowered.semantic_module.clone();
    after.machines[0].blocks[0]
        .operations
        .retain(|operation| operation.id != OperationId::new(2004).unwrap());
    assert!(
        terminal_verifier::validate_global_value_numbering(&lowered.semantic_module, &after)
            .is_err()
    );
}
