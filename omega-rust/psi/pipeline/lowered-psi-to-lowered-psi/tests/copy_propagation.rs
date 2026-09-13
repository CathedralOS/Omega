//! Stage-level coverage for the exact `CopyPropagation` rule through the
//! public `run_psi_optimization` entrance. Internal machine-level boundary
//! cases that a validated module cannot express live beside the rewrite in
//! `src/copy_propagation/copies.rs`.

mod common;

use common::{block_id, copy_fixture, dead_scalar_fixture, value};
use lowered_psi_to_lowered_psi::{PsiOptimizationStageError, run_psi_optimization};
use optimization::{PsiOptimization, PsiOptimizationSelections};
use terminal_psi::Terminator;

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
