use omega_compiler::compile_to_checked;
use psi_language_semantics::{
    BlockingInterface, ServiceReachInterface, SuspensionInterface, SynchronousInvocationInterface,
    TerminationGuarantee, TerminationInterface,
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn write_program(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "omega-service-operational-{name}-{}-{}",
        std::process::id(),
        NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed),
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).expect("create service/operational test directory");
    let main_path = directory.join("main.omg");
    fs::write(&main_path, source).expect("write service/operational test program");
    main_path
}

fn compile_error(name: &str, source: &str) -> String {
    let main_path = write_program(name, source);
    let diagnostics = compile_to_checked(&main_path, None)
        .expect_err("the service/operational contract violation must reject compilation");
    let rendered = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
    rendered
}

const CONTRACT_PROGRAM: &str = r#"
boundary trait Clock {
    machine wait()
    reaches Clock
    suspends;
}

boundary trait Storage {
    machine flush()
    reaches Storage
    blocks;
}

trait Worker {
    machine run(
        clock: &mut Clock,
        storage: &mut Storage,
        remaining: u64
    ) -> u64
    reaches Clock, Storage
    suspends;
    blocks;
    terminates;
}

machine finish(clock: &mut Clock, storage: &mut Storage) -> u64 {
    block storage.flush();
    suspend clock.wait();
    0
}

machine run_impl(
    clock: &mut Clock,
    storage: &mut Storage,
    remaining: u64
) -> u64
satisfies Worker::run
reaches Clock, Storage
suspends;
blocks;
terminates;
terminates by remaining;
{
    transition remaining > 0 {
        true -> run_impl(clock, storage, remaining - 1)
        false -> finish(clock, storage)
    }
}

data Main { }
machine Main::main(&mut self) { }
"#;

fn contract_report_fingerprint_for(source: &str, machine_name: &str) -> u64 {
    let main_path = write_program("fingerprint", source);
    let checked = compile_to_checked(&main_path, None).expect("contract program should compile");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == machine_name)
        .unwrap_or_else(|| panic!("machine {machine_name}"));
    let report_fingerprint = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("machine contract plan")
        .report_fingerprint;
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
    report_fingerprint
}

#[test]
fn provider_keeps_service_and_operational_contract_axes_independent() {
    let main_path = write_program("accepted", CONTRACT_PROGRAM);
    let checked = compile_to_checked(&main_path, None).expect("contract program should compile");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "run_impl")
        .expect("run_impl machine");

    let finish = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "finish")
        .expect("finish machine");
    let finish_blocking = checked
        .facts
        .blocking
        .for_machine(finish.symbol)
        .expect("finish blocking plan");
    let finish_suspension = checked
        .facts
        .suspensions
        .for_machine(finish.symbol)
        .expect("finish suspension plan");
    assert!(finish_suspension.checked_may_suspend);
    assert!(finish_blocking.checked_may_block);
    let finish_flow = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, state)| state.machine_symbol == finish.symbol)
        .map(|(_, state)| state)
        .collect::<Vec<_>>();
    assert!(!finish_flow.is_empty());
    assert!(
        finish_flow
            .iter()
            .any(|state| state.suspension.transitive_may_suspend)
    );
    assert!(
        finish_flow
            .iter()
            .any(|state| state.blocking.transitive_may_block)
    );

    let reach = checked
        .facts
        .service_reaches
        .for_machine(finish.symbol)
        .expect("finish service reach");
    let reached_names = checked
        .facts
        .service_reaches
        .rows
        .services(reach.inferred_transitive)
        .iter()
        .map(|service| {
            checked
                .facts
                .service_reaches
                .services
                .definition(*service)
                .expect("normalized service")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(reach.interface, ServiceReachInterface::InternalInferred);
    let contract_reach = checked
        .facts
        .service_reaches
        .for_machine(machine.symbol)
        .expect("run_impl service reach");
    let service_plan = checked
        .facts
        .service_reaches
        .plan_for_machine(machine.symbol)
        .expect("run_impl service plan");
    let published_services = match contract_reach.interface {
        psi_language_semantics::ServiceReachInterface::PublishedCeiling(row) => checked
            .facts
            .service_reaches
            .rows
            .services(row)
            .iter()
            .map(|service| {
                checked
                    .facts
                    .service_reaches
                    .services
                    .definition(*service)
                    .expect("published normalized service")
                    .name
                    .as_str()
            })
            .collect::<Vec<_>>(),
        psi_language_semantics::ServiceReachInterface::InternalInferred => {
            panic!("provider contract must publish its service ceiling")
        }
    };
    assert_eq!(service_plan.interface, contract_reach.interface);
    assert_eq!(
        service_plan.checked_inferred,
        contract_reach.inferred_transitive
    );
    assert_eq!(published_services, ["Clock", "Storage"]);
    assert_eq!(reached_names, ["Clock", "Storage"]);
    let suspension = checked
        .facts
        .suspensions
        .for_machine(machine.symbol)
        .expect("run_impl suspension plan");
    let blocking = checked
        .facts
        .blocking
        .for_machine(machine.symbol)
        .expect("run_impl blocking plan");
    let termination = checked
        .facts
        .termination
        .for_machine(machine.symbol)
        .expect("run_impl termination plan");
    assert_eq!(
        suspension.interface,
        SuspensionInterface::PublishedMaySuspend(true)
    );
    assert_eq!(
        blocking.interface,
        BlockingInterface::PublishedMayBlock(true)
    );
    assert_eq!(
        termination.interface,
        TerminationInterface::Published(TerminationGuarantee::Terminates {
            premises: Vec::new(),
        })
    );

    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
}

#[test]
fn private_ranking_spelling_cannot_perturb_public_contract_identity() {
    let inferred = contract_report_fingerprint_for(CONTRACT_PROGRAM, "run_impl");
    let explicit = contract_report_fingerprint_for(
        &CONTRACT_PROGRAM.replace(
            "terminates by remaining;",
            "terminates by remaining -> Nat::Descending;",
        ),
        "run_impl",
    );
    assert_eq!(inferred, explicit);
}

#[test]
fn synchronous_invocation_edges_survive_in_checked_contract_identity() {
    let with_edge = r#"
boundary trait Handler {
    machine handle();
}

data Published {}
boundary machine Published::entry(&mut self, handler: &mut Handler)
invokes handler;
{
    handler.handle();
}

data Main {}
machine Main::main(&mut self) {}
"#;
    let main_path = write_program("invocation-artifact", with_edge);
    let checked = compile_to_checked(&main_path, None).expect("invocation contract should compile");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Published::entry")
        .expect("published entry");
    let invocation = checked
        .facts
        .synchronous_invocations
        .for_machine(machine.symbol)
        .expect("published invocation contract");
    assert_eq!(
        invocation.interface,
        SynchronousInvocationInterface::PublishedCeiling
    );
    assert_eq!(invocation.published, ["parameter:0"]);
    assert_eq!(invocation.checked_inferred, ["parameter:0"]);

    let contract = checked
        .facts
        .contract_plans
        .for_machine(machine.symbol)
        .expect("published machine contract");

    let without_edge = with_edge.replace("invokes handler;\n{\n    handler.handle();\n}", "{\n}");
    assert_ne!(
        contract.report_fingerprint,
        contract_report_fingerprint_for(&without_edge, "Published::entry")
    );
    let _ = fs::remove_dir_all(main_path.parent().expect("temporary program directory"));
}

#[test]
fn provider_cannot_widen_an_independent_operational_axis() {
    let error = compile_error(
        "provider-widens-blocking",
        r#"
trait Worker {
    machine run() -> u64 suspends;
}

machine run_impl() -> u64
satisfies Worker::run
suspends;
blocks;
{
    0
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(error.contains("`blocks;`"), "{error}");
    assert!(
        error.contains("requirement omits `blocks;`")
            || (error.contains("exceeds the trait requirement")
                && error.contains("operational ceiling")),
        "{error}"
    );
}

#[test]
fn retired_operational_effect_names_do_not_reenter_service_rows() {
    let error = compile_error(
        "retired-mixed-row",
        r#"
machine wait()
reaches Suspend
{
}

data Main { }
machine Main::main(&mut self) { }
"#,
    );

    assert!(error.contains("`reaches Suspend` is invalid"), "{error}");
    assert!(error.contains("`suspends;`"), "{error}");
}
