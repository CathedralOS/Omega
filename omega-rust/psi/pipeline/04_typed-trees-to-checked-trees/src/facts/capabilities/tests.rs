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
