//! Stage-level coverage for the exact `ControlFlowCleanup` rule through the
//! public `run_psi_optimization` entrance: literal-resolved conditional folds,
//! unreachable-block removal under the performed folds, evidence retention,
//! and the independent verifier's rewrite relation.

use crate::common;

use common::{
    coercion_edge_fixture, coercion_region_fixture, control_flow_fixture, copy_fixture,
    dead_machine_coercion_fixture, minimal_unit_lowered, suspension_rows, two_machine_fixture,
    unit_call, value,
};
use lowered_psi_to_lowered_psi::run_psi_optimization;
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::{DebugSubject, Terminator};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::ControlFlowCleanup]).unwrap()
}

/// Install one valid operation crash-contract row naming `operation` inside
/// `machine`, publishing the matching unconditional Trap route the caller
/// coverage check requires.
fn crash_contract(lowered: &mut lowered_psi::LoweredPsi, machine_index: usize, operation: u64) {
    let unconditional_trap = terminal_psi::CrashRouteBucket {
        cause: terminal_psi::CrashCause::Trap,
        alternatives: vec![terminal_psi::CrashRouteGuard::Truth],
    };
    let machine = &mut lowered.semantic_module.machines[machine_index];
    machine.contract.crash_routes = vec![unconditional_trap.clone()];
    let machine_id = machine.id;
    lowered.semantic_module.operation_crash_contracts =
        vec![terminal_psi::TerminalOperationCrashContract {
            machine: machine_id,
            operation: common::operation_id(operation),
            published_routes: vec![unconditional_trap.clone()],
            crash_continuations: vec![unconditional_trap],
        }];
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
fn foldable_conditionals_survive_when_cleanup_is_not_selected() {
    // Disabled coverage: the literal-conditional workload under an
    // SCCP-only selection keeps every branch — cleanup is not selected. The
    // `v10`/`v11` rows are already literals, so the selected rule folds no
    // leaf either.
    let lowered = control_flow_fixture();
    let optimized = run_psi_optimization(
        lowered.clone(),
        PsiOptimizationSelections::new([PsiOptimization::SparseConditionalConstantPropagation])
            .unwrap(),
    )
    .expect("the stage executes");
    assert_eq!(optimized.lowered(), &lowered);
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
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
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
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
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
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
fn a_crash_contract_inside_the_stranded_region_keeps_the_conditional() {
    // The contracted operation sits in the block the literal fold strands:
    // removing b3 would orphan the row, so the conditional stays authored.
    let mut lowered = control_flow_fixture();
    lowered.semantic_module.machines[0].parameters = vec![common::i32(90)];
    lowered.semantic_module.machines[0].blocks[2].operations[0].kind =
        terminal_psi::OperationKind::WrappingIntegerAdd {
            left: value(90),
            right: value(90),
        };
    crash_contract(&mut lowered, 0, 30);
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("the contracted operation pins its block");
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
            common::block_id(3),
            common::block_id(4),
            common::block_id(5),
        ],
        "removing b3 would orphan the crash contract: every block stays"
    );
    assert!(
        matches!(machine.blocks[0].terminator, Terminator::Conditional { .. }),
        "the entry conditional keeps its untaken edge"
    );
    assert!(
        matches!(machine.blocks[1].terminator, Terminator::Jump { .. }),
        "the unrelated conditional still folds: it strands nothing"
    );
    terminal_verifier::validate_control_flow_cleanup(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the partial rewrite");
}

#[test]
fn an_evidence_pinned_machine_keeps_its_own_call_targets() {
    // The suspension rows name machine 2's call operation without making the
    // machine reachable, so they keep it authored rather than retained. The
    // kept machine's own call to machine 3 is still a surviving transition:
    // machine 3 stays too.
    let mut lowered = two_machine_fixture();
    lowered.semantic_module.machines[1].blocks[0]
        .operations
        .push(unit_call(111, common::machine_id(1)));
    lowered.semantic_module.machines[1].blocks[0]
        .operations
        .push(unit_call(112, common::machine_id(3)));
    lowered.semantic_module.machines.push(common::machine(
        3,
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
        common::block_id(21),
        vec![common::block(
            21,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: common::edge(21),
                trivial_affine_discards: Vec::new(),
            },
        )],
    ));
    suspension_rows(
        &mut lowered,
        common::operation_id(111),
        terminal_psi::TerminalSuspensionCallTarget::Machine(common::machine_id(1)),
    );
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("the pinned machine's callee stays alive");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "machine 2's kept call transition retains machine 3"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn a_ranked_machine_is_retained() {
    // Machine 2 is unreachable, but its ranked SCC row is execution-position
    // evidence no cleanup may drop: it stays authored. The member block shape
    // mirrors `ranked_cycle_fixture`: an entry hop binds the rank and guard,
    // and the covered self-edge passes the decremented rank back.
    let mut lowered = two_machine_fixture();
    let member = common::block_id(11);
    let (rank, guard, one, successor) = (value(110), value(111), value(112), value(113));
    lowered.semantic_module.machines[1].parameters = vec![common::u32(120), common::boolean(121)];
    lowered.semantic_module.machines[1].entry = common::block_id(10);
    lowered.semantic_module.machines[1].blocks = vec![
        common::block(
            10,
            Vec::new(),
            Vec::new(),
            common::jump(10, member, vec![value(120), value(121)]),
        ),
        common::block(
            11,
            vec![common::u32(110), common::boolean(111)],
            vec![
                common::unsigned_constant(112, common::u32(112), 1),
                common::operation(
                    113,
                    common::u32(113),
                    terminal_psi::OperationKind::WrappingIntegerSubtract {
                        left: rank,
                        right: one,
                    },
                ),
            ],
            Terminator::Conditional {
                condition: guard,
                when_true: common::successor(11, member, vec![successor, guard]),
                when_false: common::successor(12, common::block_id(12), vec![]),
            },
        ),
        common::block(
            12,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: common::edge(13),
                trivial_affine_discards: Vec::new(),
            },
        ),
    ];
    lowered.semantic_module.machines[1].ranked_scc = Some(common::natural_self_loop(
        member,
        rank,
        common::edge(11),
        successor,
    ));
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a ranked machine refuses removal");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "machine 2's ranking evidence keeps it authored"
    );
}

#[test]
fn independent_check_rejects_unjustified_machine_removals() {
    // Every machine-naming row is validated against the machine table, so an
    // `after` that drops a still-named machine fails `InvalidModule` before
    // the removal relation is even consulted.
    let mut before = two_machine_fixture();
    before.semantic_module.machines[0].blocks[0]
        .operations
        .push(unit_call(100, common::machine_id(2)));
    let mut after = before.semantic_module.clone();
    after.machines.pop();
    assert!(
        matches!(
            terminal_verifier::validate_control_flow_cleanup(&before.semantic_module, &after),
            Err(terminal_verifier::ControlFlowCleanupRewriteError::InvalidModule(_))
        ),
        "machine 1 still calls the removed machine 2"
    );

    // The module entry machine can never be removed.
    let mut no_entry = before.semantic_module.clone();
    no_entry.machines.remove(0);
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&before.semantic_module, &no_entry),
        Err(terminal_verifier::ControlFlowCleanupRewriteError::InvalidModule(_)),
    ));

    // A machine a surviving coercion row names is retained.
    let coercion = dead_machine_coercion_fixture();
    let mut after = coercion.semantic_module.clone();
    after.machines.pop();
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&coercion.semantic_module, &after),
        Err(terminal_verifier::ControlFlowCleanupRewriteError::InvalidModule(_)),
    ));

    // Dropping a machine whose operation a surviving crash-contract row
    // names leaves the row dangling: module validation rejects the rewrite
    // before the removal relation is consulted.
    let mut contracted = two_machine_fixture();
    contracted.semantic_module.machines[1].parameters = vec![common::i32(111)];
    contracted.semantic_module.machines[1].blocks[0]
        .operations
        .push(common::operation(
            112,
            common::i32(112),
            terminal_psi::OperationKind::WrappingIntegerAdd {
                left: value(111),
                right: value(111),
            },
        ));
    crash_contract(&mut contracted, 1, 112);
    let mut after = contracted.semantic_module.clone();
    after.machines.pop();
    assert!(matches!(
        terminal_verifier::validate_control_flow_cleanup(&contracted.semantic_module, &after),
        Err(
            terminal_verifier::ControlFlowCleanupRewriteError::InvalidModule(
                terminal_verifier::ModuleError::InvalidOperationCrashContract { .. }
            )
        ),
    ));
}

#[test]
fn machine_pruning_is_deterministic_and_idempotent() {
    let first = run_psi_optimization(two_machine_fixture(), selections()).unwrap();
    let second = run_psi_optimization(two_machine_fixture(), selections()).unwrap();
    assert_eq!(first, second, "identical input and selection must agree");
    let third = run_psi_optimization(first.lowered().clone(), selections()).unwrap();
    assert_eq!(
        third.lowered(),
        first.lowered(),
        "the pruned module re-enters the stage unchanged"
    );
    assert_eq!(
        third.execution().input_semantic(),
        third.execution().output_semantic(),
        "the fixed-point run records an identity"
    );
}
