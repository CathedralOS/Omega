//! Register allocation (Omega 05) allocates every program through one
//! entrance, whatever selected-instruction optimization (Omega 04) did first.
//!
//! Each test compiles one of two kept programs, one with few live values and
//! one with more live values than allocation has registers, natively for
//! linux_x86_64. Their builds select one rule from each Omega 04 phase and both
//! allocation-recovery rules; a test rolls back every rule except the ones it
//! checks. The tests then read the allocation manifest of the emitted object:
//! the Omega 04 record it carries, the recovery rewrites allocation applied,
//! and whether it spilled. With validators requested the object also keeps
//! Omega 04's completed run, which the independent replay validator checks.

#[path = "support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use fixture_package_inputs::repo_root;
use register_homes::{
    PostAllocationOptimizationManifest, PostAllocationSelectedTransformation,
    PostAllocationSpillStatus,
};
use selected_instruction_analyses::RetainedSelectedOptimizationRun as _;
use target::TargetProfile;

// Compilations share process-wide phase counters; run them one at a time.
static COMPILATIONS: std::sync::Mutex<()> = std::sync::Mutex::new(());

const LOW_PRESSURE: &str = "tests/fixtures/run/optimizer/selected_allocation_low_pressure_exit";
const HIGH_PRESSURE: &str = "tests/fixtures/run/optimizer/selected_allocation_high_pressure_exit";

const SELECTED_LOWERING: &str = "SelectedIncomingWrappingAddZeroIdentityCopy";
const PRE_ALLOCATION: &str = "SelectedSameBlockCopyI64RemovalV1";
const FIXED_VIEW_RECOVERY: &str = "SharedEntryFixedViewCopyAfterCompareBeforeBranchV1";
const REMATERIALIZATION_RECOVERY: &str = "ActiveResidentImmediateU64MultiUseRematerializationV1";
const BUILD_RULES: [&str; 4] = [
    SELECTED_LOWERING,
    PRE_ALLOCATION,
    FIXED_VIEW_RECOVERY,
    REMATERIALIZATION_RECOVERY,
];

/// Compile `fixture` keeping only the build rules in `keep`, and hand the
/// emitted object (or the joined diagnostics) to `inspect`.
fn compile<R>(
    fixture: &str,
    keep: &[&str],
    retain_for_validators: bool,
    inspect: impl FnOnce(Result<&image_emission::ObjectArtifact, String>) -> R,
) -> R {
    let root = repo_root().join(fixture).join("main.omg");
    let rollback = compiler::OptimizationRollback::from_exact_names(
        BUILD_RULES.into_iter().filter(|rule| !keep.contains(rule)),
    )
    .expect("the rolled-back rules are known names");
    let request = compiler::CompileRequest::for_targets(
        root.clone(),
        root.with_file_name("build"),
        vec![Some(TargetProfile::LinuxX64)],
    )
    .with_requested_product(compiler::RequestedCompileProduct::NativeArtifact)
    .with_retained_native_stage_inputs(retain_for_validators)
    .with_optimization_rollback(rollback);
    let outcomes =
        orchestration::compile_sources(&mut compiler::CompilationSession::default(), request);
    let outcomes = match outcomes {
        Ok(outcomes) => outcomes,
        Err(errors) => return inspect(Err(format!("{errors:?}"))),
    };
    let outcome = &outcomes.outcomes()[0];
    match outcome.report() {
        Some(report) => inspect(Ok(report
            .retained_native_artifact()
            .expect("a native compile emits an artifact")
            .object())),
        None => inspect(Err(format!("{:?}", outcome.diagnostics()))),
    }
}

fn emission(
    object: &image_emission::ObjectArtifact,
) -> &machine_emission::StagedOptimizedFunctionFragmentEmissionSource {
    object
        .retained_fragment_source()
        .expect("the object keeps its fragment producer")
        .source()
        .source()
        .source()
        .source()
}

fn manifest(object: &image_emission::ObjectArtifact) -> PostAllocationOptimizationManifest {
    emission(object).post_allocation_manifest().record().clone()
}

fn is_runtime_recovery(transformation: &PostAllocationSelectedTransformation) -> bool {
    matches!(
        transformation,
        PostAllocationSelectedTransformation::RuntimeSpill(_)
            | PostAllocationSelectedTransformation::RuntimeSplit(_)
            | PostAllocationSelectedTransformation::RuntimeRematerialization(_)
            | PostAllocationSelectedTransformation::FixedOccurrences(_)
    )
}

/// Under pressure allocation spilled; without it, it applied nothing of its own.
fn check_pressure(manifest: &PostAllocationOptimizationManifest, prefix: usize, pressure: bool) {
    let recovery = &manifest.selected_transformations[prefix..];
    if pressure {
        assert_eq!(
            manifest.spills,
            PostAllocationSpillStatus::RealizedInSelectedProgram
        );
        assert!(!recovery.is_empty() && recovery.iter().all(is_runtime_recovery));
    } else {
        assert_eq!(
            manifest.spills,
            PostAllocationSpillStatus::NotRequiredForValidatedHomePlan
        );
        assert!(recovery.is_empty(), "{recovery:?}");
    }
}

#[test]
fn identity_input_allocates_with_and_without_pressure() {
    let _compilation = COMPILATIONS.lock().unwrap();
    for (fixture, pressure) in [(LOW_PRESSURE, false), (HIGH_PRESSURE, true)] {
        compile(fixture, &[], true, |object| {
            let object = object.unwrap_or_else(|error| panic!("{fixture}: {error}"));
            let manifest = manifest(object);
            assert_eq!(manifest.selected_lowering_completion, None);
            assert_eq!(manifest.pre_allocation_completion, None);
            check_pressure(&manifest, 0, pressure);
            assert!(
                matches!(
                    image_emission::verify_retained_selected_optimization(object),
                    Ok(false)
                ),
                "no Omega 04 run produced an identity program"
            );
        });
    }
}

#[test]
fn pre_allocation_output_allocates_with_and_without_pressure() {
    let _compilation = COMPILATIONS.lock().unwrap();
    for (fixture, pressure) in [(LOW_PRESSURE, false), (HIGH_PRESSURE, true)] {
        compile(fixture, &[PRE_ALLOCATION], true, |object| {
            let object = object.unwrap_or_else(|error| panic!("{fixture}: {error}"));
            let manifest = manifest(object);
            assert!(manifest.pre_allocation_completion.is_some());
            assert_eq!(manifest.selected_lowering_completion, None);
            let (run, record) = emission(object)
                .selected_optimization_evidence()
                .expect("validators keep the evidence");
            assert!(run.is_some(), "the run is kept for the validator");
            assert_eq!(
                record.pre_allocation_completion,
                manifest.pre_allocation_completion
            );
            assert!(
                manifest
                    .selected_transformations
                    .starts_with(&record.transformations),
                "allocation keeps the Omega 04 record first"
            );
            check_pressure(&manifest, record.transformations.len(), pressure);
            assert!(matches!(
                image_emission::verify_retained_selected_optimization(object),
                Ok(true)
            ));
        });
    }
}

#[test]
fn selected_lowering_output_allocates_through_the_same_entrance() {
    let _compilation = COMPILATIONS.lock().unwrap();
    compile(LOW_PRESSURE, &[SELECTED_LOWERING], true, |object| {
        let object = object.unwrap_or_else(|error| panic!("{error}"));
        let manifest = manifest(object);
        assert!(manifest.selected_lowering_completion.is_some());
        assert_eq!(manifest.pre_allocation_completion, None);
        let (_, record) = emission(object).selected_optimization_evidence().unwrap();
        assert!(record.transformations.iter().all(|transformation| matches!(
            transformation,
            PostAllocationSelectedTransformation::LiteralFold(_)
        )));
        assert!(
            manifest
                .selected_transformations
                .starts_with(&record.transformations)
        );
        check_pressure(&manifest, record.transformations.len(), false);
        assert!(matches!(
            image_emission::verify_retained_selected_optimization(object),
            Ok(true)
        ));
    });
}

#[test]
fn a_default_compile_keeps_no_omega_04_run() {
    let _compilation = COMPILATIONS.lock().unwrap();
    compile(LOW_PRESSURE, &[PRE_ALLOCATION], false, |object| {
        let object = object.unwrap_or_else(|error| panic!("{error}"));
        assert!(manifest(object).pre_allocation_completion.is_some());
        let error = image_emission::verify_retained_selected_optimization(object)
            .expect_err("a default compile retains nothing to replay");
        assert!(error.to_string().contains("ReplayUnavailable"), "{error}");
    });
}

#[test]
fn a_retained_run_refuses_another_programs_record() {
    let _compilation = COMPILATIONS.lock().unwrap();
    compile(LOW_PRESSURE, &[PRE_ALLOCATION], true, |low| {
        let low = low.unwrap_or_else(|error| panic!("{error}"));
        compile(HIGH_PRESSURE, &[PRE_ALLOCATION], true, |high| {
            let high = high.unwrap_or_else(|error| panic!("{error}"));
            let (low_run, low_record) = emission(low).selected_optimization_evidence().unwrap();
            let (_, high_record) = emission(high).selected_optimization_evidence().unwrap();
            let low_run = low_run.expect("the low-pressure run is kept");
            low_run
                .validate(low_record)
                .expect("a run validates against its own program");
            let error = low_run
                .validate(high_record)
                .expect_err("a run cannot vouch for another program");
            assert!(error.contains("OtherProgram"), "{error}");
        });
    });
}

#[test]
fn each_recovery_entry_runs_its_own_procedure() {
    let _compilation = COMPILATIONS.lock().unwrap();
    // The fixed-view entry splits fixed-register conflicts with copies; this
    // program has none, so the copy step applies none and assignment follows.
    compile(LOW_PRESSURE, &[FIXED_VIEW_RECOVERY], true, |object| {
        let object = object.unwrap_or_else(|error| panic!("{error}"));
        let manifest = manifest(object);
        assert!(matches!(
            manifest.selected_transformations.as_slice(),
            [PostAllocationSelectedTransformation::FixedViewCopy(_)]
        ));
    });
    // The rematerialization entry runs its own procedure under its own
    // allocator availability (two registers), and that procedure's spill
    // choice refuses this program's pressure shape, as it does on main.
    compile(
        LOW_PRESSURE,
        &[REMATERIALIZATION_RECOVERY],
        false,
        |object| {
            let error = object.expect_err("the rematerialization procedure refuses this shape");
            assert!(
                error.contains("RegisterAllocation(Rematerialization(SpillChoice("),
                "{error}"
            );
        },
    );
}

#[test]
fn unsupported_compositions_still_refuse() {
    let _compilation = COMPILATIONS.lock().unwrap();
    for keep in [
        [SELECTED_LOWERING, PRE_ALLOCATION],
        [PRE_ALLOCATION, FIXED_VIEW_RECOVERY],
        [FIXED_VIEW_RECOVERY, REMATERIALIZATION_RECOVERY],
    ] {
        compile(LOW_PRESSURE, &keep, false, |object| {
            let error = object.expect_err("a composition nothing serves is refused");
            assert!(
                error.contains("UnsupportedComposition"),
                "{keep:?}: {error}"
            );
        });
    }
}
