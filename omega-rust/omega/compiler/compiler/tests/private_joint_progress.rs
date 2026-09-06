use checked_trees::CheckedTrees;
use compiler::compile_to_checked;
use language_semantics::{
    MachineTerminationPlan, ProgressPremise, ProgressSubject, TerminationGuarantee,
    TerminationInterface,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use typed_trees::machine::Machine;

const LEXICOGRAPHIC: &str = include_str!(
    "../../../../../tests/omega/pass/termination/joint_lexicographic_machine_call_cycle_compile/main.omg"
);
// Same forwarding/descent shape as joint_call_rankings.rs, without any
// authored public termination guarantee to seed the checked summaries.
const SCALAR: &str = r#"
data Main {}
machine Main::main(&mut self) -> u64 { transition { _ -> self.a(4) } }
machine Main::a(&mut self, remaining: u64)
terminates by remaining;
-> u64 { transition { _ -> self.b(remaining) } }
machine Main::b(&mut self, count: u64)
terminates by count -> Nat::Descending;
-> u64 { transition count { 0 -> 0 _ -> self.a(count - 1) } }
"#;

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

struct Project(PathBuf);

impl Project {
    fn new(source: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-private-joint-progress-{}-{}",
            std::process::id(),
            NEXT_PROJECT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create test project");
        fs::write(path.join("main.omg"), source).expect("write progress regression");
        Self(path)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test project");
    }
}

fn checked(source: &str) -> CheckedTrees {
    let project = Project::new(source);
    compile_to_checked(&project.0.join("main.omg"), None)
        .expect("progress fixture must reach checked trees")
        .into_program()
}

fn machine<'program>(program: &'program CheckedTrees, name: &str) -> &'program Machine {
    program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap_or_else(|| panic!("missing machine {name}"))
}

fn private_plan<'program>(
    program: &'program CheckedTrees,
    name: &str,
) -> &'program MachineTerminationPlan {
    let machine = machine(program, name);
    assert!(!machine.is_public, "{name} must remain private");
    let plan = program
        .facts
        .termination
        .for_machine(machine.symbol)
        .expect("checked termination plan");
    assert_eq!(plan.interface, TerminationInterface::InternalDerived);
    assert_eq!(machine.termination_plan.interface, plan.interface);
    assert_eq!(
        plan.interface.published(),
        None,
        "{name}: checked progress must not author a public promise"
    );
    plan
}

fn assert_unconditional_cycle(source: &str, members: &[&str]) {
    let program = checked(
        &source
            .replace("data Main", "pub data Main")
            .replace("machine Main::main", "pub machine Main::main"),
    );
    for name in members {
        let plan = private_plan(&program, name);
        assert!(plan.implementation_witness.is_some());
        assert_eq!(
            plan.checked_summary,
            TerminationGuarantee::Terminates {
                premises: Vec::new()
            },
            "{name}: validated joint ranking must derive checked progress"
        );
    }
    let entry = machine(&program, "Main::main");
    let entry_plan = program
        .facts
        .termination
        .for_machine(entry.symbol)
        .expect("exported caller termination plan");
    assert!(
        entry_plan
            .interface
            .published()
            .is_none_or(|guarantee| !guarantee.promises_termination()),
        "the exported caller must not publish its inferred progress"
    );
    assert_eq!(
        entry_plan.checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        },
        "the caller's body must consume the private cycle's checked summary"
    );
}

#[test]
fn private_scalar_cycle_derives_termination_without_publishing_it() {
    assert_unconditional_cycle(SCALAR, &["Main::a", "Main::b"]);
}

#[test]
fn private_lexicographic_cycle_derives_termination_without_publishing_it() {
    assert_unconditional_cycle(LEXICOGRAPHIC, &["Main::scan_a", "Main::scan_b"]);
}

#[test]
fn unknown_external_progress_prevents_private_cycle_inference() {
    // An implementation outside the cycle inherits a requirement with no
    // progress promise. Only one cycle member invokes it.
    for (source, exit, members) in [
        (SCALAR, "0 -> 0", ["Main::a", "Main::b"]),
        (
            LEXICOGRAPHIC,
            "false -> 0",
            ["Main::scan_a", "Main::scan_b"],
        ),
    ] {
        let source = format!(
            "trait External {{ machine unknown(&mut self) -> u64; }}
             machine Main::unknown(&mut self) -> u64 satisfies External::unknown {{ 0 }}\n{}",
            source.replacen(exit, &exit.replace("-> 0", "-> self.unknown()"), 1)
        );
        let program = checked(&source);
        let external = machine(&program, "Main::unknown");
        let external_plan = program
            .facts
            .termination
            .for_machine(external.symbol)
            .expect("external callee termination plan");
        assert_eq!(
            external_plan.interface,
            TerminationInterface::Published(TerminationGuarantee::NoGuarantee)
        );
        assert!(external_plan.checked_summary.promises_termination());
        for name in members.into_iter().chain(["Main::main"]) {
            assert_eq!(
                private_plan(&program, name).checked_summary,
                TerminationGuarantee::NoGuarantee,
                "{name}: unknown external progress must propagate through the cycle"
            );
        }
    }
}

const QUALIFIED_CYCLE: &str = r#"
pub data SchedulerHandle {}
pub data Context { scheduler: SchedulerHandle; }
pub domain SchedulerHandle::WeakFair
satisfies ProgressProfile
established by SchedulerAdmission::grant;
pub boundary trait SchedulerAdmission {
    machine grant(scheduler: SchedulerHandle) -> SchedulerHandle in WeakFair;
}
pub machine wait(context: Context)
requires context.scheduler in WeakFair
terminates;
-> u64 { 0 }

data Main {}
machine Main::main(&mut self) -> u64 { 0 }
machine Main::a(&mut self, remaining: u64, context: Context)
requires context.scheduler in WeakFair
terminates by remaining;
-> u64 { transition { _ -> self.b(context, remaining) } }
machine Main::b(&mut self, forwarded: Context, count: u64)
requires forwarded.scheduler in WeakFair
terminates by count;
-> u64 {
    transition count {
        0 -> wait(forwarded)
        _ -> self.a(count - 1, forwarded)
    }
}
"#;

#[test]
fn exact_external_premise_survives_private_cycle_propagation() {
    assert_exact_external_premise(&checked(QUALIFIED_CYCLE));
}

#[test]
fn private_external_wrapper_can_be_solved_after_the_cycle() {
    let source = format!(
        "{}\nmachine wait_for_scheduler(context: Context) -> u64
         requires context.scheduler in WeakFair
         {{ transition {{ _ -> wait(context) }} }}",
        QUALIFIED_CYCLE.replace("0 -> wait(forwarded)", "0 -> wait_for_scheduler(forwarded)")
    );
    assert_exact_external_premise(&checked(&source));
}

fn assert_exact_external_premise(program: &CheckedTrees) {
    let external = machine(program, "wait");
    assert!(
        validation::validated_runtime_recursive_components(&program.typed)
            .iter()
            .any(
                |component| component.contains(&machine(program, "Main::a").symbol)
                    && component.contains(&machine(program, "Main::b").symbol)
            ),
        "the source must retain its validated joint ranking"
    );
    let Some(TerminationGuarantee::Terminates { premises }) =
        external.termination_plan.interface.published()
    else {
        panic!("external contract must retain its authored progress premise");
    };
    let [premise] = premises.as_slice() else {
        panic!("external contract must contain exactly one progress premise");
    };
    assert_eq!(
        program
            .facts
            .termination
            .for_machine(external.symbol)
            .expect("external callee checked summary")
            .checked_summary,
        TerminationGuarantee::Terminates {
            premises: Vec::new()
        },
        "the cycle must consume the published premise even though wait's body is unconditional"
    );
    assert!(premise.profile.is_valid());
    assert_eq!(premise.subject.projections.len(), 1);
    assert_eq!(
        program
            .symbols
            .display_path(premise.subject.projections[0], "::"),
        "Context::scheduler"
    );
    for (name, parameter_name) in [("Main::a", "context"), ("Main::b", "forwarded")] {
        let member = machine(program, name);
        let entry = &program.machine_states(member)[0];
        let parameter = program
            .state_parameters(entry)
            .iter()
            .find(|parameter| parameter.name.as_str() == parameter_name)
            .expect("exact capability parameter");
        let plan = private_plan(program, name);
        assert!(plan.implementation_witness.is_some());
        assert_eq!(
            plan.checked_summary,
            TerminationGuarantee::Terminates {
                premises: vec![ProgressPremise {
                    profile: premise.profile,
                    subject: ProgressSubject {
                        root: parameter.symbol,
                        projections: premise.subject.projections.clone(),
                    },
                }],
            },
            "{name}: retain exactly the external premise with the caller's parameter root"
        );
    }
}

#[test]
fn independent_external_premises_converge_as_a_set() {
    let source = QUALIFIED_CYCLE
        .replace("scheduler: SchedulerHandle; }", "scheduler: SchedulerHandle; backup: SchedulerHandle; }")
        .replace("{ transition { _ -> self.b(context, remaining) } }", "{ transition remaining > 0 { true -> self.b(context, remaining) false -> wait(context) } }")
        .replace("0 -> wait(forwarded)\n        _ -> self.a(count - 1, forwarded)", "_ -> self.a(count - 1, forwarded)")
        .replace("transition count {\n        _ -> self.a(count - 1, forwarded)", "transition count > 0 {\n        true -> self.a(count - 1, forwarded)\n        false -> wait_backup(forwarded)");
    let source = format!(
        "{source}\npub machine wait_backup(context: Context)
        requires context.backup in WeakFair
        terminates;
        -> u64 {{ 0 }}"
    );
    let program = checked(&source);
    for name in ["Main::a", "Main::b"] {
        let TerminationGuarantee::Terminates { premises } =
            &private_plan(&program, name).checked_summary
        else {
            panic!("{name} must retain both external premises");
        };
        assert_eq!(premises.len(), 2);
        let paths = premises
            .iter()
            .map(|premise| {
                assert_eq!(premise.subject.projections.len(), 1);
                program
                    .symbols
                    .display_path(premise.subject.projections[0], "::")
            })
            .collect::<Vec<_>>();
        assert!(paths.iter().any(|path| path == "Context::scheduler"));
        assert!(paths.iter().any(|path| path == "Context::backup"));
    }
}

#[test]
fn recursively_projected_requirements_do_not_become_a_finite_promise() {
    let source = QUALIFIED_CYCLE
        .replace(
            "scheduler: SchedulerHandle; }",
            "scheduler: SchedulerHandle; next: &Context; }",
        )
        .replace("context: Context", "context: &Context")
        .replace("forwarded: Context", "forwarded: &Context")
        .replace(
            "self.a(count - 1, forwarded)",
            "self.a(count - 1, forwarded.next)",
        );
    let program = checked(&source);
    assert!(
        validation::validated_runtime_recursive_components(&program.typed)
            .iter()
            .any(|component| {
                component.contains(&machine(&program, "Main::a").symbol)
                    && component.contains(&machine(&program, "Main::b").symbol)
            })
    );
    for name in ["Main::a", "Main::b"] {
        assert_eq!(
            private_plan(&program, name).checked_summary,
            TerminationGuarantee::NoGuarantee
        );
    }
}
