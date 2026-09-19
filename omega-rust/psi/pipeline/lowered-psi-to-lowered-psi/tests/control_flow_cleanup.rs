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
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use std::collections::BTreeSet;
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
        erased_arguments: _,
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
fn an_unreferenced_machine_is_pruned() {
    let lowered = two_machine_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("control flow cleanup executes");
    assert_eq!(
        optimized
            .lowered()
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![common::machine_id(1)],
        "nothing names machine 2: it leaves the module"
    );
    assert_ne!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic(),
        "the removal is recorded as a semantic change"
    );
    terminal_verifier::validate_control_flow_cleanup(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed removal");
}

#[test]
fn a_call_transition_retains_its_callee() {
    let mut lowered = two_machine_fixture();
    lowered.semantic_module.machines[0].blocks[0]
        .operations
        .push(unit_call(100, common::machine_id(2)));
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("control flow cleanup executes");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "the entry machine calls machine 2: it stays"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn a_stranded_call_site_drops_its_callee() {
    // The only call to machine 2 sits in the block the literal fold strands:
    // once b3 leaves, machine 2 is unreachable and leaves with it.
    let mut lowered = control_flow_fixture();
    lowered.semantic_module.machines.push(common::machine(
        2,
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
        common::block_id(11),
        vec![common::block(
            11,
            Vec::new(),
            Vec::new(),
            Terminator::ReturnUnit {
                edge: common::edge(11),
                trivial_affine_discards: Vec::new(),
            },
        )],
    ));
    lowered.semantic_module.machines[0].blocks[2]
        .operations
        .push(unit_call(300, common::machine_id(2)));
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("control flow cleanup executes");
    assert_eq!(
        optimized
            .lowered()
            .semantic_module
            .machines
            .iter()
            .map(|machine| machine.id)
            .collect::<Vec<_>>(),
        vec![common::machine_id(1)],
        "the fold strands b3 and its call; machine 2 leaves with it"
    );
    terminal_verifier::validate_control_flow_cleanup(
        &lowered.semantic_module,
        &optimized.lowered().semantic_module,
    )
    .expect("the independent check accepts the executed rewrite");
}

#[test]
fn a_coercion_row_retains_the_machine_it_names() {
    let lowered = dead_machine_coercion_fixture();
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a coercion-named machine refuses removal");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "the coercion names machine 2 and its exact edge and values: it stays"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn a_suspension_row_retains_the_operation_owner() {
    // Machine 2 holds a call to machine 1; the module's suspension rows name
    // that call operation but target machine 1, so they pin machine 2's
    // contents without making it reachable.
    let mut lowered = two_machine_fixture();
    lowered.semantic_module.machines[1].blocks[0]
        .operations
        .push(unit_call(111, common::machine_id(1)));
    suspension_rows(
        &mut lowered,
        common::operation_id(111),
        terminal_psi::TerminalSuspensionCallTarget::Machine(common::machine_id(1)),
    );
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a suspension-named operation refuses removal");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "the suspension plan names an operation inside machine 2: it stays"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
    );
}

#[test]
fn a_crash_contract_row_retains_its_machine_and_operation() {
    // Machine 2 is unreachable, but the module's operation crash-contract row
    // names it and its add operation: while the row survives, both stay
    // authored.
    let mut lowered = two_machine_fixture();
    let machine = &mut lowered.semantic_module.machines[1];
    machine.parameters = vec![common::i32(111)];
    machine.blocks[0].operations.push(common::operation(
        112,
        common::i32(112),
        terminal_psi::OperationKind::WrappingIntegerAdd {
            left: value(111),
            right: value(111),
        },
    ));
    crash_contract(&mut lowered, 1, 112);
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("a crash-contracted operation refuses removal");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "the crash contract names machine 2's operation: machine 2 stays"
    );
    assert_eq!(
        optimized.execution().input_semantic(),
        optimized.execution().output_semantic()
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
fn an_attached_machine_is_retained() {
    // An attached machine answers nominal-type custody rather than call
    // reachability, so it is a retention root even when nothing calls it.
    let mut lowered = two_machine_fixture();
    lowered.semantic_module.structural_types = vec![terminal_psi::StructuralTypeDeclaration {
        id: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
        identity: "attached".to_string(),
        shape: terminal_psi::StructuralTypeShape::PrimitiveScalar(common::i32_type()),
    }];
    lowered.semantic_module.machines[1].attachment =
        Some(semantic_vocabulary::StructuralTypeId::new(1).unwrap());
    let optimized = run_psi_optimization(lowered.clone(), selections())
        .expect("an attached machine refuses removal");
    assert_eq!(
        optimized.lowered(),
        &lowered,
        "machine 2's attachment keeps it authored"
    );
}

#[test]
fn retained_machine_closure_covers_every_naming_row() {
    // The entry machine is a root; an unreferenced machine is not retained.
    let plain = two_machine_fixture();
    assert_eq!(
        terminal_verifier::retained_machines(&plain.semantic_module),
        BTreeSet::from([common::machine_id(1)]),
    );

    // A direct call transition retains the callee.
    let mut called = two_machine_fixture();
    called.semantic_module.machines[0].blocks[0]
        .operations
        .push(unit_call(100, common::machine_id(2)));
    assert_eq!(
        terminal_verifier::retained_machines(&called.semantic_module),
        BTreeSet::from([common::machine_id(1), common::machine_id(2)]),
    );

    // A module-level evidence row naming the machine retains it too.
    let coercion = dead_machine_coercion_fixture();
    assert_eq!(
        terminal_verifier::retained_machines(&coercion.semantic_module),
        BTreeSet::from([common::machine_id(1), common::machine_id(2)]),
    );

    // An operation crash-contract row names its machine directly: the
    // contracted machine is a retention root even with no caller left.
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
    assert_eq!(
        terminal_verifier::retained_machines(&contracted.semantic_module),
        BTreeSet::from([common::machine_id(1), common::machine_id(2)]),
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

#[test]
fn machine_removal_prunes_machine_scoped_debug_sites() {
    let mut lowered = two_machine_fixture();
    common::with_debug_sites(
        &mut lowered,
        &[
            DebugSubject::Machine(common::machine_id(1)),
            DebugSubject::Machine(common::machine_id(2)),
            DebugSubject::Block(common::block_id(11)),
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
        vec![DebugSubject::Machine(common::machine_id(1))],
        "sites naming the removed machine or its contents are pruned"
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
