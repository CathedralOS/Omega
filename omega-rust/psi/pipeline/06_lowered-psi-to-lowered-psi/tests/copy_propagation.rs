//! Stage-level coverage for the exact `CopyPropagation` rule through the
//! public `run_psi_optimization` entrance. Internal machine-level boundary
//! cases that a validated module cannot express live beside the rewrite in
//! `src/copy_propagation/copies.rs`.

use crate::common;

use common::{block_id, copy_fixture, dead_scalar_fixture, machine_id, value};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::Terminator;
use terminal_verifier::{CopyPropagationRewriteError, validate_copy_propagation};

fn selections() -> PsiOptimizationSelections {
    PsiOptimizationSelections::new([PsiOptimization::CopyPropagation]).unwrap()
}

#[test]
fn selected_copy_propagation_collapses_uniform_and_single_edge_parameters() {
    let lowered = copy_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("copy propagation executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    let merge = &machine.blocks[3];
    assert_eq!(
        merge.parameters.iter().map(|p| p.id).collect::<Vec<_>>(),
        vec![value(42)],
        "the uniform v41 copy collapses; the divergent v42 stays"
    );
    assert_eq!(
        machine.blocks[4].parameters.len(),
        0,
        "the single-edge v51 copy collapses to the add result"
    );
    let Terminator::Jump { arguments, .. } = &merge.terminator else {
        panic!("merge keeps its jump")
    };
    assert_eq!(arguments, &[], "the collapsed edge argument drops out");
    let Terminator::Jump { arguments, .. } = &machine.blocks[1].terminator else {
        panic!("b2 keeps its jump")
    };
    assert_eq!(arguments, &[value(20)]);
    let Terminator::Jump { arguments, .. } = &machine.blocks[2].terminator else {
        panic!("b3 keeps its jump")
    };
    assert_eq!(arguments, &[value(30)]);
    let Terminator::Return {
        value: returned, ..
    } = &machine.blocks[4].terminator
    else {
        panic!("b5 keeps its return")
    };
    assert_eq!(*returned, value(44), "the return reads the resolved source");
    let add = &merge.operations[0].kind;
    assert_eq!(
        add,
        &terminal_psi::OperationKind::WrappingIntegerAdd {
            left: value(2),
            right: value(42)
        },
        "direct uses substitute the resolved source"
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
}

#[test]
fn divergent_bindings_and_effects_are_retained() {
    let lowered = copy_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("copy propagation executes");
    let machine = &optimized.lowered().semantic_module.machines[0];
    let merge = &machine.blocks[3];
    // Negative: a parameter bound to a different resolved value per edge is
    // not a copy and survives with its argument positions.
    assert_eq!(merge.parameters.len(), 1);
    assert_eq!(merge.parameters[0].id, value(42));
    // The rewrite removes no operations: v20/v30 still feed the retained v42.
    assert!(machine.blocks[1].operations.len() == 1 && machine.blocks[2].operations.len() == 1);
}

#[test]
fn dead_code_survives_when_dead_scalar_elimination_is_not_selected() {
    // Disabled coverage for the sibling rule on the same input: a dead
    // parameter/operation workload under a CopyPropagation-only selection
    // collapses the copy but retains every dead producer.
    let lowered = dead_scalar_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("copy propagation executes");
    let machine = &optimized.lowered().semantic_module.machines[0];
    assert_eq!(
        machine.blocks[0].operations.len(),
        1,
        "the dead v10 constant survives: elimination is not selected"
    );
    assert_eq!(
        machine.blocks[1].operations.len() + machine.blocks[2].operations.len(),
        4,
        "every constant producer survives"
    );
    assert_eq!(
        machine.blocks[3]
            .parameters
            .iter()
            .map(|p| p.id)
            .collect::<Vec<_>>(),
        vec![value(42), value(43)],
        "v41 collapsed; the divergent v42 and dead v43 remain parameters"
    );
}

#[test]
fn optimized_output_is_a_legal_second_input_and_reaches_a_fixed_point() {
    let lowered = copy_fixture();
    let first = run_psi_optimization(lowered, selections()).expect("first run executes");
    // The published artifact is a validated `LoweredPsi`, a legal second
    // input — this is fixed-point coverage, not repeated reconstruction.
    let second = run_psi_optimization(first.lowered().clone(), selections())
        .expect("the published artifact re-enters the stage");
    assert_eq!(
        second.lowered(),
        first.lowered(),
        "a second selection changes nothing: no copy parameter remains"
    );
    assert_eq!(
        second.execution().input_semantic(),
        second.execution().output_semantic(),
        "the fixed-point run records an identity"
    );
}

#[test]
fn execution_is_deterministic_across_runs() {
    let first = run_psi_optimization(copy_fixture(), selections()).unwrap();
    let second = run_psi_optimization(copy_fixture(), selections()).unwrap();
    assert_eq!(first, second, "identical input and selection must agree");
}

#[test]
fn copy_propagation_prunes_removed_value_debug_sites() {
    let mut lowered = copy_fixture();
    common::with_debug_sites(
        &mut lowered,
        &[
            terminal_psi::DebugSubject::Value(value(41)),
            terminal_psi::DebugSubject::Value(value(42)),
            terminal_psi::DebugSubject::Value(value(51)),
            terminal_psi::DebugSubject::Machine(common::machine_id(1)),
        ],
    );
    let optimized = run_psi_optimization(lowered, selections()).unwrap();
    let debug = optimized.lowered().debug_map.as_ref().unwrap();
    let subjects = debug
        .sites
        .iter()
        .map(|site| site.subject)
        .collect::<Vec<_>>();
    assert!(
        !subjects.contains(&terminal_psi::DebugSubject::Value(value(41)))
            && !subjects.contains(&terminal_psi::DebugSubject::Value(value(51))),
        "collapsed parameter sites are dropped: {subjects:?}"
    );
    assert!(subjects.contains(&terminal_psi::DebugSubject::Value(value(42))));
    assert_eq!(
        debug.semantic,
        terminal_codec::terminal_psi_identity(&optimized.lowered().semantic_module).unwrap(),
        "the sidecar semantic identity tracks the rewritten module"
    );
}

#[test]
fn structurally_invalid_inputs_fail_before_rewrite() {
    // Corruption coverage: each malformed carrier fails closed at the
    // module-validation gate before any rewrite decision.
    let mut no_machines = copy_fixture();
    no_machines.semantic_module.machines.clear();
    assert!(matches!(
        run_psi_optimization(no_machines, selections()),
        Err(PsiOptimizationStageError::InvalidModule(
            terminal_verifier::ModuleError::EmptyModule
        ))
    ));

    let mut bad_target = copy_fixture();
    let Terminator::Jump { target, .. } =
        &mut bad_target.semantic_module.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    *target = block_id(99);
    assert!(matches!(
        run_psi_optimization(bad_target, selections()),
        Err(PsiOptimizationStageError::InvalidModule(_))
    ));

    let mut bad_arity = copy_fixture();
    let Terminator::Jump { arguments, .. } =
        &mut bad_arity.semantic_module.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    arguments.pop();
    assert!(matches!(
        run_psi_optimization(bad_arity, selections()),
        Err(PsiOptimizationStageError::InvalidModule(_))
    ));

    let mut bad_debug = copy_fixture();
    common::with_debug_sites(&mut bad_debug, &[]);
    bad_debug
        .debug_map
        .as_mut()
        .unwrap()
        .semantic
        .program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes([0xff; 32]);
    assert!(matches!(
        run_psi_optimization(bad_debug, selections()),
        Err(PsiOptimizationStageError::InvalidDebugMap(_))
    ));
}

#[test]
fn proof_bearing_closure_remains_unchanged() {
    let mut lowered = copy_fixture();
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
        optimized.execution().output_semantic(),
        "the frozen run records an identity"
    );
}

#[test]
fn independent_check_rejects_unjustified_collapses() {
    let before = copy_fixture().semantic_module;

    // v42 binds a different resolved value per edge: removing it is not a
    // copy collapse no matter how the producer substitutes its uses.
    let mut removed_non_copy = before.clone();
    let merge = &mut removed_non_copy.machines[0].blocks[3];
    merge
        .parameters
        .retain(|parameter| parameter.id != value(42));
    merge.operations[0].kind = terminal_psi::OperationKind::WrappingIntegerAdd {
        left: value(41),
        right: value(2),
    };
    let Terminator::Jump { arguments, .. } = &mut removed_non_copy.machines[0].blocks[1].terminator
    else {
        panic!("b2 is a jump")
    };
    *arguments = vec![value(2)];
    let Terminator::Jump { arguments, .. } = &mut removed_non_copy.machines[0].blocks[2].terminator
    else {
        panic!("b3 is a jump")
    };
    *arguments = vec![value(2)];
    assert!(matches!(
        validate_copy_propagation(&before, &removed_non_copy),
        Err(CopyPropagationRewriteError::RemovedNonCopyParameter(block))
            if block == block_id(4)
    ));

    // Reordering the surviving parameter list is a block-shape change, not a
    // copy removal.
    let mut reordered = before.clone();
    reordered.machines[0].blocks[3].parameters.swap(0, 1);
    assert!(matches!(
        validate_copy_propagation(&before, &reordered),
        Err(CopyPropagationRewriteError::ChangedBlockParameters(block))
            if block == block_id(4)
    ));

    // Removing the copy but substituting a different source than the resolved
    // one is drift the replayed reconstruction catches.
    let mut wrong_source = before.clone();
    let merge = &mut wrong_source.machines[0].blocks[3];
    merge
        .parameters
        .retain(|parameter| parameter.id != value(41));
    merge.operations[0].kind = terminal_psi::OperationKind::WrappingIntegerAdd {
        left: value(42),
        right: value(42),
    };
    for index in [1usize, 2] {
        let Terminator::Jump { arguments, .. } =
            &mut wrong_source.machines[0].blocks[index].terminator
        else {
            panic!("b{index} is a jump")
        };
        *arguments = vec![match index {
            1 => value(20),
            _ => value(30),
        }];
    }
    assert!(matches!(
        validate_copy_propagation(&before, &wrong_source),
        Err(CopyPropagationRewriteError::ChangedMachine(machine))
            if machine == machine_id(1)
    ));

    // A machine added to the module is a structural change, not a collapse.
    let mut added = before.clone();
    added.machines.push(common::machine(
        2,
        Vec::new(),
        terminal_psi::TerminalMachineResult::Unit,
        block_id(10),
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
        validate_copy_propagation(&before, &added),
        Err(CopyPropagationRewriteError::ChangedProgramStructure)
    ));
}

#[test]
fn independent_check_rejects_a_changed_proof_question() {
    // A structurally exact collapse still refuses when the reconstructed
    // proof question cannot be carried verbatim: both sides publish the same
    // ensures clause, but the substituted uses change its reconstructed
    // axioms. This is the refusal the proof-bearing freeze above observes.
    let clause = terminal_psi::ContractClause {
        obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
        proposition: semantic_vocabulary::Proposition::Truth,
    };
    let mut before = copy_fixture();
    before.semantic_module.machines[0]
        .contract
        .ensures
        .push(clause.clone());
    let mut after = run_psi_optimization(copy_fixture(), selections())
        .unwrap()
        .into_lowered()
        .semantic_module;
    after.machines[0].contract.ensures.push(clause);
    assert!(matches!(
        validate_copy_propagation(&before.semantic_module, &after),
        Err(CopyPropagationRewriteError::ChangedProofQuestion)
    ));
}

#[test]
fn ranked_machine_collapses_outside_copies_while_covered_coordinates_stay() {
    // The validated `Natural` row covers member block b3 alone: its parameter
    // table, covered backedge arguments, and named values stay exact. The
    // merge's copy parameters are outside the covered component and collapse
    // under the ordinary rule.
    let lowered = common::ranked_cycle_fixture();
    let optimized =
        run_psi_optimization(lowered.clone(), selections()).expect("copy propagation executes");
    let machine = &optimized.lowered().semantic_module.machines[0];

    let merge = &machine.blocks[1];
    assert!(
        merge.parameters.is_empty(),
        "both merge parameters were copies and collapse"
    );
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("the entry keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[]);
    assert_eq!(when_false.arguments, &[]);
    let Terminator::Jump { arguments, .. } = &merge.terminator else {
        panic!("the merge keeps its jump")
    };
    assert_eq!(
        arguments,
        &[value(1), value(2), value(21)],
        "the member edge substitutes the resolved sources"
    );

    let member = &machine.blocks[2];
    assert_eq!(
        member
            .parameters
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>(),
        vec![value(10), value(11), value(32)],
        "the covered parameter table stays exact — copy-shaped v32 keeps its position"
    );
    let Terminator::Conditional {
        when_true,
        when_false,
        ..
    } = &member.terminator
    else {
        panic!("the member keeps its conditional")
    };
    assert_eq!(when_true.arguments, &[value(41), value(11), value(21)]);
    assert_eq!(when_false.arguments, &[]);
    assert_eq!(
        machine.ranked_scc, lowered.semantic_module.machines[0].ranked_scc,
        "the ranking row is untouched"
    );
}
