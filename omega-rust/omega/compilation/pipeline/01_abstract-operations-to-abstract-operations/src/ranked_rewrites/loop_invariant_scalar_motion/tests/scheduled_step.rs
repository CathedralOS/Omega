//! The relocation boundary as one scheduled pass step: selection drives
//! `run_psi_pipeline` through propose/validate/apply, and the retained
//! commits, manifest, and ledger survive independent publication replay.

use optimization_core::{
    Optimization, OptimizationPassIdentity, OptimizationSelections, OptimizationWorkBudget,
};
use optimization_unit::PsiCandidateDeclaration;

use crate::ranked_rewrites::loop_invariant_scalar_motion::{
    LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME, rule_identity, validator_identity,
};
use crate::{optimize_abstract_operations, publish_optimization_run, run_psi_pipeline};

use super::{TRANSITIVE_MEMBER_SOURCE, lowered_unit};

/// A straight-line machine: no cyclic component means the step proposes no
/// relocation at all.
const ACYCLIC_SOURCE: &str = r#"
    data Root {}

    machine Root::scan(scale: u32 in Wrapping)
    {
        transition { _ -> finish(scale) }
        state finish(r: u32 in Wrapping) {}
    }
"#;

fn selections_of(optimizations: &[Optimization]) -> OptimizationSelections {
    OptimizationSelections::new(optimizations.iter().copied()).expect("exact selection")
}

fn budget() -> OptimizationWorkBudget {
    OptimizationWorkBudget::new(96, 64, 64, 64, 64).expect("test budget")
}

/// Selecting the relocation boundary schedules the session-level step: the
/// run relocates the member computation, retains one commit whose declaration
/// is the relocation roster, and publishes one pass manifest and ledger row
/// that independent replay re-derives during `publish_optimization_run`.
#[test]
fn scheduled_step_relocates_and_publishes() {
    let unit = lowered_unit(TRANSITIVE_MEMBER_SOURCE, "scheduled relocation");
    let input_identity = unit.unit().identity;
    let selections = selections_of(&[Optimization::LoopInvariantScalarMotion]);

    let run = run_psi_pipeline(unit.clone(), &selections, budget())
        .expect("the scheduled relocation step runs");
    let [commit] = run.commits() else {
        panic!("the scheduled step commits one relocation")
    };
    let PsiCandidateDeclaration::LoopInvariantScalarMotion(candidate) = commit.declaration() else {
        panic!("the commit declaration is a relocation roster")
    };
    assert_eq!(commit.rule, rule_identity());
    assert_eq!(commit.validator, validator_identity());
    assert_eq!(commit.input, input_identity);
    assert_eq!(commit.output, run.session().unit().identity);
    assert_ne!(commit.output, commit.input);
    assert!(
        !candidate.relocations().is_empty(),
        "the committed roster relocates the transitive member computation"
    );

    let [manifest] = run.pass_manifests() else {
        panic!("the step publishes exactly one pass manifest")
    };
    assert_eq!(
        manifest.pass(),
        OptimizationPassIdentity::from_canonical_bytes(LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME)
    );
    assert_eq!(manifest.ordered_rules(), [rule_identity()].as_slice());
    assert_eq!(manifest.input(), input_identity);
    assert_eq!(manifest.output(), commit.output);

    let output_identity = commit.output;
    let ledger = run.transformation_ledger();
    assert_eq!(ledger.records().len(), 1);
    assert_eq!(ledger.input(), input_identity);
    assert_eq!(ledger.output(), output_identity);

    // Publication independently replays the retained declaration against the
    // recorded schedule: a forged or stale roster cannot reach this point.
    let plan = publish_optimization_run(run).expect("replayed plan publishes");
    assert_eq!(plan.unit().identity, output_identity);
}

/// With no cyclic component the step still runs and publishes its empty
/// manifest: the schedule itself is the pass's evidence, not the commit.
#[test]
fn scheduled_step_without_components_still_publishes() {
    let unit = lowered_unit(ACYCLIC_SOURCE, "acyclic relocation schedule");
    let input_identity = unit.unit().identity;
    let selections = selections_of(&[Optimization::LoopInvariantScalarMotion]);

    let run = run_psi_pipeline(unit.clone(), &selections, budget())
        .expect("the scheduled step runs without components");
    assert!(
        run.commits().is_empty(),
        "an acyclic unit offers no relocation"
    );
    let [manifest] = run.pass_manifests() else {
        panic!("the empty step still publishes its manifest")
    };
    assert_eq!(
        manifest.pass(),
        OptimizationPassIdentity::from_canonical_bytes(LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME)
    );
    assert_eq!(manifest.input(), input_identity);
    assert_eq!(manifest.output(), input_identity);
    let plan = publish_optimization_run(run).expect("the empty run publishes");
    assert_eq!(plan.unit().identity, input_identity);
}

/// The full phase entrance resolves the same schedule: selection projection
/// admits the relocation selection and the chained run reaches publication.
#[test]
fn optimize_abstract_operations_schedules_the_relocation_step() {
    let unit = lowered_unit(TRANSITIVE_MEMBER_SOURCE, "phase-entrance relocation");
    let selections = selections_of(&[Optimization::LoopInvariantScalarMotion]);
    let plan = optimize_abstract_operations(
        unit.input().clone(),
        &selections,
        &selections.project_psi(),
        budget(),
    )
    .expect("the phase runs the scheduled relocation step");
    let [commit] = plan.commits() else {
        panic!("the phase commits one relocation")
    };
    assert!(
        matches!(
            commit.declaration(),
            PsiCandidateDeclaration::LoopInvariantScalarMotion(_)
        ),
        "the retained commit is a relocation roster"
    );
}
