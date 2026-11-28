//! Generic wrappers retain type, const and selected machine arguments.

#[path = "support/program.rs"]
mod program;
mod support;

fn fixture_path(tier: &str, sample: &str) -> std::path::PathBuf {
    support::repository_root()
        .join("source-rust/application/tests/fixtures/calls")
        .join(tier)
        .join(sample)
}

#[test]
fn mixed_static_arguments_run_when_interpreted_and_built() {
    program::assert_fixture_silent_return(&fixture_path("run", "mixed_static_arguments"));
}

#[test]
fn a_const_argument_still_owes_its_declared_requirement() {
    let (output, report) = program::check_fixture(&fixture_path("fail", "mixed_static_arguments"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove requires contract for call keep from forward")
            && !report.contains("not supported yet"),
        "{report}"
    );
}

#[test]
fn nominal_callback_parameters_keep_their_distinct_selections() {
    program::assert_fixture_silent_return(&fixture_path("run", "nominal_static_arguments"));
}

#[test]
fn a_generic_trait_callback_is_judged_at_the_call_type_arguments() {
    program::assert_fixture_silent_return(&fixture_path("run", "generic_nominal_static_arguments"));
}

#[test]
fn a_callback_for_another_trait_argument_is_refused() {
    let (output, report) =
        program::check_fixture(&fixture_path("fail", "generic_nominal_static_arguments"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("callback `left_step` must satisfy the exact requirement of `Step`"),
        "{report}"
    );
}

#[test]
fn a_matching_signature_does_not_bind_a_callback_to_the_requirement() {
    let (output, report) =
        program::check_fixture(&fixture_path("fail", "nominal_static_arguments"));
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("callback `same_shape` must satisfy the exact requirement of `Right`"),
        "{report}"
    );
}

#[test]
fn enum_static_arguments_retain_their_value_across_a_forwarding_call() {
    let source = r#"
use {omega}/language/core/task.omg;
const split_plan: |task|::ActivationStorage = |task|::ActivationStorage::Split;

machine choose<const Plan: |task|::ActivationStorage>() -> u32 {
    match Plan {
        |task|::ActivationStorage::Fixed -> 1u32,
        |task|::ActivationStorage::Split -> 2u32
    }
}
machine forward<const Plan: |task|::ActivationStorage>() -> u32 { choose::<Plan>() }

machine main() crashes Trap {
    let fixed: u32 = forward::<|task|::ActivationStorage::Fixed>();
    let split: u32 = forward::<split_plan>();
    transition fixed == 1u32 && split == 2u32 { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;
    program::assert_silent_return(source);
}

#[test]
fn an_ordinary_caller_runs_the_core_worker_with_distinct_job_and_outcome_types() {
    let source = r#"
use {omega}/language/core/task.omg;

machine call<Arguments, T, const Target: |task|::TaskWorker<Arguments, T>>(job: Arguments) -> T
suspends blocks {
    suspend block Target(job)
}
machine positive(arguments: u32) -> bool { arguments > 0u32 }
machine main() crashes Trap {
    let zero: bool = call::<u32, bool, positive>(0u32);
    let one: bool = call::<u32, bool, positive>(1u32);
    transition !zero && one { true -> done(), false -> wrong() }
    machine done() {}
    machine wrong() crashes Trap { crash Trap; }
}
"#;
    let directory = platform_custody::temporary_directory::TemporaryDirectory::create(
        "closed-worker-effects", false,
    ).unwrap();
    let path = directory.path().join("worker.psi");
    let (output, report) = program::invoke(source, &["--emit-psi", path.to_str().unwrap()]);
    assert!(output.status.success(), "{report}");
    let saved = terminal_codec::CanonicalTerminalArtifact::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let module = saved.semantic_module();
    let owners = module.machines.iter().filter(|machine| {
        machine.closed_reach_application.as_ref().is_some_and(|application| {
            application.suspension_dependency.parameters == [2]
                && application.blocking_dependency.parameters == [2]
                && !application.suspension_dependency.concrete
                && !application.blocking_dependency.concrete
        })
    }).collect::<Vec<_>>();
    assert_eq!(owners.len(), 1, "the worker owns both operational dependencies");
    assert!(!owners[0].published_may_suspend && !owners[0].published_may_block);
    let mut missing = module.clone();
    let owner = missing.machines.iter_mut().find(|machine| machine.id == owners[0].id).unwrap();
    let application = owner.closed_reach_application.as_mut().unwrap();
    application.suspension_dependency.parameters.clear();
    application.blocking_dependency.parameters.clear();
    application.template_commitment = terminal_psi::closed_reach_template_commitment(
        application.template_base_commitment,
        application.parameters(),
        &application.suspension_dependency,
        &application.blocking_dependency,
        |_| None,
    ).unwrap();
    assert_eq!(
        terminal_semantics::closed_reach::validate_closed_reach_applications(&missing),
        Err("closed callback dependency omits an operational effect of its body"),
    );
    program::assert_silent_return(source);
}

#[test]
fn a_selected_worker_and_an_independent_call_each_keep_their_own_waits() {
    let source = r#"
use {omega}/language/core/task.omg;
machine fixed(arguments: u32) -> u32 suspends { arguments }
machine work(arguments: u32) -> bool blocks { arguments == 1u32 }
machine call<const Target: |task|::TaskWorker<u32, bool>>(job: u32) -> bool suspends blocks {
    let value: u32 = suspend fixed(job);
    suspend block Target(value)
}
machine main() suspends blocks {
    let answer: bool = MARK call::<work>(1u32);
}
"#;
    for (marker, missing) in [("suspend", "block acknowledgement"), ("block", "suspend acknowledgement")] {
        let (output, report) = program::invoke(&source.replace("MARK", marker), &["--check"]);
        assert!(!output.status.success(), "{report}");
        assert!(report.contains(missing), "{report}");
    }
}

#[test]
fn choosing_activation_storage_alone_does_not_establish_custody() {
    for (entry, outcome) in [("start", "Task<T>"), ("try_start", "StartOutcome<T, T>")] {
        let source = format!(
            r#"
use {{omega}}/language/core/task.omg;
machine schedule<T, const Worker: |task|::TaskWorker<T, T>, const Plan: |task|::ActivationStorage>(job: T) -> |task|::{outcome}
reaches |task|::TaskRuntime invokes |task|::TaskRuntime crashes Trap crashes Abort {{
    |task|::TaskRuntime::{entry}::<T, T, Worker, Plan>(job)
}}
machine main() {{}}
"#
        );
        let (output, report) = program::invoke(&source, &["--check"]);
        assert!(!output.status.success(), "{entry}: {report}");
        assert!(report.contains("execution custody"), "{entry}: {report}");
    }
}

#[test]
fn equally_named_static_cases_from_distinct_carriers_are_rejected() {
    let source = r#"
data Storage [copy] { case Fixed; case Split; }
data Other [copy] { case Fixed; case Split; }
machine choose<const Plan: Storage>() {}
machine main() { choose::<Other::Fixed>(); }
"#;
    let (output, report) = program::invoke(source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("carrier"), "{report}");
}

#[test]
fn a_structural_static_parameter_retains_the_fields_of_its_named_constant() {
    program::assert_silent_return(
        r#"
data Size [copy] { width: u32; }
const tiny: Size = Size { width: 7 };
machine read<const Plan: Size>() -> u32 { Plan.width }
machine main() crashes Trap {
    let width: u32 = read::<tiny>();
    transition width == 7 { true -> done(), false -> bad() }
    machine done() {}
    machine bad() crashes Trap { crash Trap; }
}
"#,
    );
}

#[test]
fn checking_one_structural_instance_does_not_discharge_the_generic_body() {
    let source = r#"
data Storage [copy] { case Fixed; case Split; }
machine choose<const Plan: Storage>() -> u32 ensures result == 1u32 {
    match Plan { Storage::Fixed -> 1u32, Storage::Split -> 2u32 }
}
machine main() { let fixed: u32 = choose::<Storage::Fixed>(); }
"#;
    let (output, report) = program::invoke(source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(
        report.contains("cannot prove ensures contract for exit from choose"),
        "{report}"
    );
}

#[test]
fn a_selected_provider_uses_the_callers_worker_and_static_value() {
    program::assert_fixture_silent_return(&fixture_path("run", "selected_callback"));
}

#[test]
fn a_provider_cannot_call_the_worker_outside_its_public_contract() {
    let source = r#"
data Pick [copy] { case First; case Second; }
data Allowed<T> = machine(value: T, flag: u32) -> T requires flag == 7u32 reaches none crashes none;
data Unrestricted<T> = machine(value: T, flag: u32) -> T reaches none crashes none;
boundary trait Dispatch {
    machine apply<T, const Worker: Allowed<T>, const Plan: Pick>(value: T) -> T reaches none crashes none;
}
LocalDispatcher satisfies Dispatch {
    machine apply<T, const Action: Unrestricted<T>, const Plan: Pick>(value: T) -> T
    reaches none crashes none { Action(value, 1u32) }
}
machine main() {}
"#;
    let (output, report) = program::invoke(source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("different callback contract"), "{report}");
}

#[test]
fn selecting_a_provider_preserves_an_explicit_empty_worker_reach() {
    let source = r#"
data Step<T> = machine(value: T) -> T reaches none crashes none;
boundary trait Dispatch {
    machine apply<T, const Worker: Step<T>>(input: T) -> T reaches none crashes none;
}
LocalDispatcher satisfies Dispatch {
    machine apply<T, const Action: Step<T>>(input: T) -> T reaches none crashes none {
        Action(input)
    }
}
boundary trait Output { machine write(value: u32) reaches none crashes none; }
machine emits(value: u32) -> u32 reaches Output invokes Output crashes none {
    Output::write(value);
    value
}
machine main() reaches Dispatch + Output invokes Dispatch crashes none {
    let observed: u32 = Dispatch::apply::<u32, emits>(1u32);
}
"#;
    let (output, report) = program::invoke(source, &["--check"]);
    assert!(!output.status.success(), "{report}");
    assert!(report.contains("authored service ceiling"), "{report}");
}
