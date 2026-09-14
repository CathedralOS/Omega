//! Stage-level coverage for the exact `ControlFlowCleanup` rule through the
//! public `run_psi_optimization` entrance: literal-resolved conditional folds,
//! unreachable-block removal under the performed folds, evidence retention,
//! and the independent verifier's rewrite relation.

mod common;

use common::{
    coercion_edge_fixture, coercion_region_fixture, control_flow_fixture, copy_fixture,
    minimal_unit_lowered, value,
};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{DebugSubject, Terminator};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap()
}

#[test]
fn literal_conditionals_fold_and_stranded_regions_are_removed() {
    let lowered = control_flow_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("control flow cleanup executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    assert_eq!(
        machine
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![
            common::block_id(1),
            common::block_id(2),
            common::block_id(4)
        ],
        "the fold strands b3 and b5; both are removed with the conditional"
    );
    let Terminator::Jump {
        edge,
        target,
        arguments,
        structural_arguments,
        trivial_affine_discards,
        residual_affine_discards,
    } = &machine.blocks[0].terminator
    else {
        panic!("the entry conditional folds to the selected successor edge")
    };
    assert_eq!(*edge, common::edge(2), "the taken edge keeps its identity");
    assert_eq!(*target, common::block_id(2));
    assert!(
        arguments.is_empty()
            && structural_arguments.is_empty()
            && trivial_affine_discards.is_empty()
            && residual_affine_discards.is_empty(),
        "the fold carries the selected edge's rows verbatim and no residual"
    );
    let Terminator::Jump { edge, target, .. } = &machine.blocks[1].terminator else {
        panic!("the trivially conditional branch folds to the taken edge")
    };
    assert_eq!(*edge, common::edge(4));
    assert_eq!(*target, common::block_id(4));
    assert_eq!(
        machine
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .map(|operation| operation.id)
            .collect::<Vec<_>>(),
        vec![
            common::operation_id(10),
            common::operation_id(11),
            common::operation_id(20),
            common::operation_id(40),
        ],
        "v30 and v50 die with their blocks; the dead producers stay for the \
         dead-scalar rule"
    );
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the rewrite is recorded as a semantic change"
    );
    assert_eq!(
        optimized.execution().input_proof(),
        optimized.execution().output_proof(),
        "the proof bundle is byte-identical"
    );
    terminal_verifier::validate_control_flow_cleanup(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
}

#[test]
fn fold_keeps_the_false_arm_when_the_condition_is_false() {
    let mut lowered = control_flow_fixture();
    let entry = &mut lowered.semantic_module.machines[0].blocks[0];
    entry.operations[1].kind = terminal_psi::OperationKind::BooleanConstant { value: false };
    let Terminator::Conditional { condition, .. } = &mut entry.terminator else {
        panic!("entry is a conditional")
    };
    *condition = value(11);
    let optimized = run_psi_optimization(lowered.clone(), selections()).unwrap();
    let machine = &optimized.lowered().semantic_module.machines[0];
    let Terminator::Jump { edge, target, .. } = &machine.blocks[0].terminator else {
        panic!("the false condition selects the false edge")
    };
    assert_eq!(*edge, common::edge(3));
    assert_eq!(*target, common::block_id(3));
    assert_eq!(
        machine
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![
            common::block_id(1),
            common::block_id(3),
            common::block_id(4),
            common::block_id(5)
        ],
        "the true arm strands b2; b3, b4, and b5 stay reachable through it"
    );
    terminal_verifier::validate_control_flow_cleanup(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
}

#[test]
fn non_literal_conditionals_are_retained() {
    // The copy fixture's entry branches on a machine parameter: nothing in it
    // is a `BooleanConstant` result, so the module passes through unchanged.
    let lowered = copy_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("control flow cleanup executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
    let lowered = minimal_unit_lowered();
    let optimized = run_psi_optimization(lowered.clone(), selections()).unwrap();
    assert_eq!(optimized.lowered(), &lowered);
}

#[test]
fn an_evidence_bound_untaken_edge_keeps_the_conditional() {
    let lowered = coercion_edge_fixture();
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a coercion-pinned edge refuses the fold");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "dropping e3 would orphan its coercion: the conditional stays"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn evidence_inside_the_stranded_region_keeps_the_conditional() {
    let lowered = coercion_region_fixture();
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("evidence inside the stranded region refuses the fold");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "removing b3 would orphan the e6 coercion: the conditional stays"
    );
}

#[test]
fn proof_bearing_closure_remains_unchanged() {
    let mut lowered = control_flow_fixture();
    lowered.semantic_module.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            proposition: semantic_vocabulary::Proposition::Truth,
        });
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a proof-bearing closure remains unchanged");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "reconstructed obligations freeze the whole closure until \
         proof-context transport is implemented"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn independent_check_rejects_unjustified_rewrites() {
    let before = control_flow_fixture().semantic_module;

    // A conditional rewritten to the untaken edge is not the fold the literal
    // justifies. b2 strands under that jump, so it leaves with it.
    let mut wrong_arm = before.clone();
    wrong_arm.machines[0].blocks[0].terminator = Terminator::Jump {
        edge: common::edge(3),
        target: common::block_id(3),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    wrong_arm.machines[0].blocks.remove(1);
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&before, &wrong_arm),
        Err(
            terminal_verifier::ControlFlowCleanupRewriteError::UnjustifiedFold(block)
        ) if block == common::block_id(1)
    ));

    // A Jump rewritten to a different Jump is outside this rule entirely.
    let mut changed_jump = before.clone();
    changed_jump.machines[0].blocks[4].terminator = Terminator::Jump {
        edge: common::edge(9),
        target: common::block_id(4),
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        trivial_affine_discards: Vec::new(),
        residual_affine_discards: Vec::new(),
    };
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&before, &changed_jump),
        Err(
            terminal_verifier::ControlFlowCleanupRewriteError::UnjustifiedFold(block)
        ) if block == common::block_id(5)
    ));

    // A surviving block whose operations changed is not a fold.
    let mut changed_body = before.clone();
    changed_body.machines[0].blocks[0].operations[0].kind =
        terminal_psi::OperationKind::BooleanConstant { value: false };
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&before, &changed_body),
        Err(
            terminal_verifier::ControlFlowCleanupRewriteError::ChangedSurvivingBlock(block)
        ) if block == common::block_id(1)
    ));

    // A machine added to the module is a structural change, not cleanup.
    let mut added = before.clone();
    added.machines.push(common::machine(
        2,
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
        common::block_id(10),
        vec![common::block(
            10,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: common::edge(10),
                trivial_affine_discards: Vec::new(),
            },
        )],
    ));
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&before, &added),
        Err(terminal_verifier::ControlFlowCleanupRewriteError::ChangedProgramStructure)
    ));
}

#[test]
fn independent_check_rejects_a_changed_proof_question() {
    let mut lowered = control_flow_fixture();
    lowered.semantic_module.machines[0]
        .contract
        .ensures
        .push(terminal_psi::ContractClause {
            obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
            proposition: semantic_vocabulary::Proposition::Truth,
        });
    let mut after = lowered.semantic_module.clone();
    after.machines[0].contract.ensures.clear();
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&lowered.semantic_module, &after),
        Err(terminal_verifier::ControlFlowCleanupRewriteError::ChangedProofQuestion)
    ));
}

#[test]
fn optimized_output_is_a_legal_second_input_and_reaches_a_fixed_point() {
    let lowered = control_flow_fixture();
    let first = run_psi_optimization(lowered, selections()).expect("first run executes");
    let second = run_psi_optimization(first.lowered().clone(), selections())
        .expect("the published artifact re-enters the stage");
    assert_eq!(
        second.lowered(),
        first.lowered(),
        "a second selection changes nothing: every literal branch is folded"
    );
    assert_eq!(
        second.execution().input_semantic(),
        second.execution().output_semantic(),
        "the fixed-point run records an identity"
    );
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(control_flow_fixture(), selections()).unwrap();
    let second = run_psi_optimization(control_flow_fixture(), selections()).unwrap();
    assert_eq!(first, second, "identical input and selection must agree");
}

#[test]
fn cleanup_prunes_removed_subjects_and_updates_semantic_identity() {
    let mut lowered = control_flow_fixture();
    common::with_debug_sites(
        &mut lowered,
        &[
            DebugSubject::Block(common::block_id(3)),
            DebugSubject::Block(common::block_id(5)),
            DebugSubject::Edge(common::edge(3)),
            DebugSubject::Edge(common::edge(6)),
            DebugSubject::Operation(common::operation_id(30)),
            DebugSubject::Value(value(30)),
            DebugSubject::Value(value(50)),
            DebugSubject::Block(common::block_id(4)),
            DebugSubject::Machine(common::machine_id(1)),
        ],
    );
    let optimized = run_psi_optimization(lowered, selections()).unwrap();
    let debug = optimized.lowered().debug_map.as_ref().unwrap();
    assert_eq!(
        debug
            .sites
            .iter()
            .map(|site| site.subject)
            .collect::<Vec<_>>(),
        vec![
            DebugSubject::Machine(common::machine_id(1)),
            DebugSubject::Block(common::block_id(4)),
        ],
        "sites naming removed blocks, edges, operations, and values are pruned"
    );
    assert_eq!(
        debug.semantic,
        terminal_codec::terminal_psi_identity(&optimized.lowered().semantic_module).unwrap(),
        "the sidecar semantic identity tracks the rewritten module"
    );
}

#[test]
fn structurally_invalid_inputs_fail_before_rewrite() {
    let mut bad_target = control_flow_fixture();
    let Terminator::Jump { target, .. } =
        &mut bad_target.semantic_module.machines[0].blocks[2].terminator
    else {
        panic!("b3 is a jump")
    };
    *target = common::block_id(99);
    assert!(matches!(
        run_psi_optimization(bad_target, selections()),
        Err(PsiOptimizationStageError::InvalidModule(_))
    ));
}
