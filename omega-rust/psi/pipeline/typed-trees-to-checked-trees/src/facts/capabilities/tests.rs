//! Capability fact tests.

use super::{CapabilityFlowKind, CapabilityFlowPlan, SymbolHandle, TypedTrees};
use crate::facts::build_capability_facts;
use crate::tests::front_end::typed_program;
use checked_trees::FlowFacts;

fn capability_plan(source: &str) -> (TypedTrees, CapabilityFlowPlan) {
    let typed = typed_program(source);
    let operations = validation::infer_operational_may(&typed);
    let service_reaches = validation::infer_service_reaches(&typed, &operations);
    // Capability-flow verbs derive from normalized call topology,
    // independent of raw control-flow facts, so an empty FlowFacts
    // exercises the nested-propagation logic.
    let plan = build_capability_facts(&typed, &service_reaches, &FlowFacts::default());
    (typed, plan)
}

fn state_symbol(program: &TypedTrees, state_name: &str) -> SymbolHandle {
    program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .find(|state| state.name.as_str() == state_name)
        .map(|state| state.symbol)
        .unwrap_or_else(|| panic!("state {state_name} should exist"))
}

fn has_flow(plan: &CapabilityFlowPlan, kind: CapabilityFlowKind, state: SymbolHandle) -> bool {
    plan.flows()
        .any(|flow| flow.kind == kind && flow.state_symbol == state)
}

fn has_flow_via(
    plan: &CapabilityFlowPlan,
    kind: CapabilityFlowKind,
    state: SymbolHandle,
    via: SymbolHandle,
) -> bool {
    plan.flows()
        .any(|flow| flow.kind == kind && flow.state_symbol == state && flow.via_state_symbol == via)
}

#[test]
fn returns_propagates_to_nested_caller() {
    // RootDir::open mints a Folder. Vault::open_folder calls it and returns
    // the Folder (direct `returns`). Vault::expose only calls open_folder and
    // returns the Folder, so `returns` must follow the call graph up to it.
    let (program, plan) = capability_plan(
        r#"
            boundary trait Folder {
                machine write_line(text: String);
            }

            boundary trait RootDir {
                machine open() -> Folder;
            }

            data Vault<'s> {
                root: &'s mut RootDir;
            }

            machine Vault::open_folder(&self) -> Folder {
                self.root.open();
            }

            machine Vault::expose(&self) -> Folder {
                self.open_folder();
            }

            data Main {
                vault: Vault;
            }

            machine Main::main(&mut self) {
                self.vault.expose();
            }
            "#,
    );

    let open_folder = state_symbol(&program, "open_folder");
    let expose = state_symbol(&program, "expose");

    assert!(
        has_flow(&plan, CapabilityFlowKind::Returns, open_folder),
        "nested helper that returns a minted capability should record `returns`"
    );
    assert!(
        has_flow(&plan, CapabilityFlowKind::Returns, expose),
        "`returns` must propagate to the caller that only reaches the boundary \
             through the nested helper"
    );
}

#[test]
fn derives_propagates_to_nested_caller() {
    // Workspace::subfolder derives a SubFolder from a parent Folder argument.
    // Broker::narrow calls it with a caller-provided Folder and returns the
    // derived SubFolder (direct `derives`). Broker::delegate forwards its own
    // Folder to narrow, so `derives` must follow the call graph up to it.
    let (program, plan) = capability_plan(
        r#"
            boundary trait Folder {
                machine write_line(text: String)
                reaches
                    Folder;
            }

            boundary trait SubFolder {
                machine write_line(text: String)
                reaches
                    SubFolder;
            }

            boundary trait Workspace {
                machine subfolder<'s>(parent: &'s mut Folder) -> SubFolder
                reaches
                    Workspace;
            }

            data Broker<'s> {
                workspace: &'s mut Workspace;
            }

            machine Broker::narrow<'s>(&self, folder: &'s mut Folder) -> SubFolder {
                self.workspace.subfolder(folder);
            }

            machine Broker::delegate<'s>(&self, folder: &'s mut Folder) -> SubFolder {
                self.narrow(folder);
            }

            data Main<'s> {
                broker: Broker;
                folder: &'s mut Folder;
            }

            machine Main::main(&mut self) {
                self.broker.delegate(self.folder);
            }
            "#,
    );

    let narrow = state_symbol(&program, "narrow");
    let delegate = state_symbol(&program, "delegate");

    assert!(
        has_flow(&plan, CapabilityFlowKind::Derives, narrow),
        "nested helper that derives a capability should record `derives`"
    );
    assert!(
        has_flow(&plan, CapabilityFlowKind::Derives, delegate),
        "`derives` must propagate to the caller that only reaches the boundary \
             through the nested helper"
    );
}

#[test]
fn acquires_propagates_through_helper_return() {
    // RootDir::open mints fresh authority and returns the Folder, so
    // Vault::open_folder records a direct `acquires`. Vault::expose obtains
    // the same minted authority through open_folder's capability return, and
    // Main::main through expose's — `acquires` must follow the call graph up
    // with the helper recorded as provenance.
    let (program, plan) = capability_plan(
        r#"
            boundary trait Folder {
                machine write_line(text: String)
                reaches
                    Folder;
            }

            boundary trait RootDir {
                machine open() -> Folder
                reaches
                    RootDir;
            }

            data Vault<'s> {
                root: &'s mut RootDir;
            }

            machine Vault::open_folder(&self) -> Folder {
                self.root.open();
            }

            machine Vault::expose(&self) -> Folder {
                self.open_folder();
            }

            data Main {
                vault: Vault;
            }

            machine Main::main(&mut self) {
                self.vault.expose();
            }
            "#,
    );

    let open_folder = state_symbol(&program, "open_folder");
    let expose = state_symbol(&program, "expose");
    let main = state_symbol(&program, "main");

    assert!(
        has_flow_via(
            &plan,
            CapabilityFlowKind::Acquires,
            open_folder,
            SymbolHandle::invalid()
        ),
        "helper that mints authority at the boundary should record a direct `acquires`"
    );
    assert!(
        has_flow_via(&plan, CapabilityFlowKind::Acquires, expose, open_folder),
        "`acquires` must propagate to the caller receiving the minted capability, \
             recording the helper as provenance"
    );
    assert!(
        has_flow_via(&plan, CapabilityFlowKind::Acquires, main, expose),
        "`acquires` must keep flowing up across several call levels"
    );
}

#[test]
fn acquires_does_not_propagate_when_helper_keeps_authority() {
    // Archiver::flush acquires through its machine-owned Disk but returns
    // nothing, so the minted authority never reaches Main::main.
    let (program, plan) = capability_plan(
        r#"
            boundary trait Disk {
                machine write_line(text: String);
            }

            data Archiver<'s> {
                disk: &'s mut Disk;
            }

            machine Archiver::flush(&mut self) {
                self.disk.write_line("flush log");
            }

            data Main {
                archiver: Archiver;
            }

            machine Main::main(&mut self) {
                self.archiver.flush();
            }
            "#,
    );

    let flush = state_symbol(&program, "flush");
    let main = state_symbol(&program, "main");

    assert!(
        has_flow(&plan, CapabilityFlowKind::Acquires, flush),
        "helper exercising machine-owned authority should record a direct `acquires`"
    );
    assert!(
        !has_flow(&plan, CapabilityFlowKind::Acquires, main),
        "`acquires` must not propagate when the helper keeps the authority to itself"
    );
}

#[test]
fn returns_does_not_propagate_to_non_capability_caller() {
    // The caller `Vault::touch` returns nothing, so the nested `returns` verb
    // must not flow up to it.
    let (program, plan) = capability_plan(
        r#"
            boundary trait Folder {
                machine write_line(text: String)
                reaches
                    Folder;
            }

            boundary trait RootDir {
                machine open() -> Folder
                reaches
                    RootDir;
            }

            data Vault<'s> {
                root: &'s mut RootDir;
            }

            machine Vault::open_folder(&self) -> Folder {
                self.root.open();
            }

            machine Vault::touch(&self) {
                self.open_folder();
            }

            data Main {
                vault: Vault;
            }

            machine Main::main(&mut self) {
                self.vault.touch();
            }
            "#,
    );

    let open_folder = state_symbol(&program, "open_folder");
    let touch = state_symbol(&program, "touch");

    assert!(
        has_flow(&plan, CapabilityFlowKind::Returns, open_folder),
        "nested helper should still record its own direct `returns`"
    );
    assert!(
        !has_flow(&plan, CapabilityFlowKind::Returns, touch),
        "`returns` must not propagate to a caller that returns no capability"
    );
}
