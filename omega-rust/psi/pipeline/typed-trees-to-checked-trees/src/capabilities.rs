//! Checked capability-flow fact population.
//!
//! Capability-flow facts describe how authority-bearing values move through a
//! package (chapter 18, "Capabilities And Authority Flow"). Each boundary call
//! is classified into one or more verbs, inferred from boundary provenance,
//! receiver provenance, signature service reach, and capability-typed returns — no new
//! source keywords are required:
//!
//! - `uses`     — a boundary capability is reached by a call.
//! - `stores`   — the capability is retained in a machine-owned field, either as
//!   the receiver of the boundary call or as the target of a `self.field = <call>`
//!   assignment.
//! - `acquires` — the package obtains authority it holds itself (a boundary call
//!   through a machine-owned capability, or a boundary that returns a fresh
//!   capability value).
//! - `returns`  — a capability value leaves through the enclosing state's
//!   capability-typed return.
//! - `derives`  — a narrower capability is produced from an existing capability
//!   passed into the boundary call.

use arena::Arena;
use checked_trees::FlowFacts;
use flow_effects::{
    CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan, ServiceReachInferencePlan,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::machine::Machine;
use typed_trees::signature::StateSignature;
use typed_trees::state::State;
use typed_trees::statement::StatementNode;

/// Provenance of a boundary call's receiver: where the capability came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReceiverProvenance {
    /// The capability entered through a caller-provided parameter.
    CallerProvided,
    /// The capability is retained in a machine-owned field.
    MachineOwned,
    /// Provenance could not be determined.
    Unknown,
}

pub(crate) fn build_capability_facts(
    program: &TypedTrees,
    service_reaches: &ServiceReachInferencePlan,
    flow: &FlowFacts,
) -> CapabilityFlowPlan {
    let mut flows = Arena::with_capacity(flow.boundaries.edges.len());

    // `uses`: every boundary-trait edge recorded by the flow analysis for calls
    // that resolve to an in-package boundary implementation.
    for (_, state) in flow.control.states.iter() {
        for call in flow.control.calls.span_or_empty(state.calls) {
            for edge in flow.boundaries.edges.span_or_empty(call.boundary_edges) {
                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Uses,
                    edge.boundary_trait_symbol,
                    state.machine_symbol,
                    state.state_symbol,
                    edge.statement_index,
                    edge.call_ordinal,
                    SymbolHandle::invalid(),
                );
            }
        }
    }

    // Derive every authority-flow verb from normalized call topology, including
    // abstract boundary-trait calls that raw control-flow facts elide because
    // they have no in-package implementation state.
    for machine_reach in service_reaches.machines() {
        let machine = machine_for_symbol(program, machine_reach.machine);
        for state_reach in service_reaches.states_for(machine_reach) {
            let enclosing =
                machine.and_then(|machine| enclosing_state(program, machine, state_reach.state));
            for call in service_reaches.calls_for(state_reach) {
                let Some((capability_symbol, signature)) =
                    boundary_capability(program, call.target_state)
                else {
                    continue;
                };

                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Uses,
                    capability_symbol,
                    machine_reach.machine,
                    state_reach.state,
                    call.statement_index,
                    call.call_ordinal,
                    SymbolHandle::invalid(),
                );

                let returns_capability = signature
                    .map(|signature| is_capability_type(program, signature.return_type))
                    .unwrap_or(false);
                let derives_argument = signature
                    .map(|signature| passes_capability_argument(program, signature))
                    .unwrap_or(false);

                let provenance = enclosing
                    .map(|state| call_receiver_provenance(program, state, call.statement_index))
                    .unwrap_or(ReceiverProvenance::Unknown);

                // `stores`: the capability is retained in a machine-owned field,
                // either as the call receiver or as a `self.field = <call>`
                // assignment target.
                let stored_into_field = matches!(provenance, ReceiverProvenance::MachineOwned)
                    || enclosing
                        .map(|state| call_stored_into_field(program, state, call.statement_index))
                        .unwrap_or(false);
                if stored_into_field {
                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Stores,
                        capability_symbol,
                        machine_reach.machine,
                        state_reach.state,
                        call.statement_index,
                        call.call_ordinal,
                        SymbolHandle::invalid(),
                    );
                }

                // `acquires`: the package obtains authority it holds itself —
                // a boundary call through a machine-owned capability, or a
                // boundary that yields a fresh capability value.
                if returns_capability || matches!(provenance, ReceiverProvenance::MachineOwned) {
                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Acquires,
                        capability_symbol,
                        machine_reach.machine,
                        state_reach.state,
                        call.statement_index,
                        call.call_ordinal,
                        SymbolHandle::invalid(),
                    );
                }

                // `derives`: a narrower capability produced from an existing
                // capability passed into the boundary call.
                if derives_argument
                    && (returns_capability
                        || matches!(provenance, ReceiverProvenance::CallerProvided))
                {
                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Derives,
                        capability_symbol,
                        machine_reach.machine,
                        state_reach.state,
                        call.statement_index,
                        call.call_ordinal,
                        SymbolHandle::invalid(),
                    );
                }

                // `returns`: a capability leaves through the enclosing state's
                // capability-typed return.
                if returns_capability
                    && enclosing
                        .map(|state| is_capability_type(program, state.return_type))
                        .unwrap_or(false)
                {
                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Returns,
                        capability_symbol,
                        machine_reach.machine,
                        state_reach.state,
                        call.statement_index,
                        call.call_ordinal,
                        SymbolHandle::invalid(),
                    );
                }
            }
        }
    }

    // Propagate `returns`/`derives`/`acquires` across nested, in-package calls.
    // A boundary capability that a nested (non-boundary) call returns, derives,
    // or acquires-and-returns flows the same verb up to its caller, following
    // the call graph one or more levels.
    propagate_nested_capability_flows(program, service_reaches, &mut flows);

    CapabilityFlowPlan::with_roots(flows)
}

/// A nested-call edge in the service-reach plan's call graph: a caller state
/// calling a target state, with the source location of the call.
struct CallEdge {
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_state_symbol: SymbolHandle,
}

/// Flows `returns`/`derives`/`acquires` capability verbs from nested in-package
/// callees up to their callers. If a call's target state itself returns (resp.
/// derives, acquires) a boundary capability and the authority value reaches the
/// caller, the caller records the same verb for that capability with the helper
/// state as provenance. Iterated to a fixpoint so the verb can travel several
/// call levels.
fn propagate_nested_capability_flows(
    program: &TypedTrees,
    service_reaches: &ServiceReachInferencePlan,
    flows: &mut Arena<CapabilityFlowFact>,
) {
    let edges = collect_call_edges(service_reaches);

    loop {
        let mut added = false;

        for edge in &edges {
            for kind in [
                CapabilityFlowKind::Returns,
                CapabilityFlowKind::Derives,
                CapabilityFlowKind::Acquires,
            ] {
                // Capabilities the callee state already carries with this verb.
                let propagated: Vec<SymbolHandle> = flows
                    .iter()
                    .filter(|(_, fact)| {
                        fact.kind == kind && fact.state_symbol == edge.target_state_symbol
                    })
                    .map(|(_, fact)| fact.capability_symbol)
                    .collect();

                if propagated.is_empty() {
                    continue;
                }

                if !nested_flow_reaches_caller(program, edge, kind) {
                    continue;
                }

                for capability_symbol in propagated {
                    if push_unique(
                        flows,
                        kind,
                        capability_symbol,
                        edge.caller_machine_symbol,
                        edge.caller_state_symbol,
                        edge.statement_index,
                        edge.call_ordinal,
                        edge.target_state_symbol,
                    ) {
                        added = true;
                    }
                }
            }
        }

        if !added {
            break;
        }
    }
}

fn collect_call_edges(service_reaches: &ServiceReachInferencePlan) -> Vec<CallEdge> {
    let mut edges = Vec::new();
    for machine_reach in service_reaches.machines() {
        for state_reach in service_reaches.states_for(machine_reach) {
            for call in service_reaches.calls_for(state_reach) {
                if !call.target_state.is_valid() {
                    continue;
                }
                edges.push(CallEdge {
                    caller_machine_symbol: machine_reach.machine,
                    caller_state_symbol: state_reach.state,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_state_symbol: call.target_state,
                });
            }
        }
    }
    edges
}

/// Whether a propagated `kind` verb actually reaches the caller across `edge`.
/// Mirrors the direct-boundary gating: `returns` requires the caller's enclosing
/// state to return a capability type; `derives` requires the caller to return a
/// capability or to hold a capability parameter that feeds the nested
/// derivation; `acquires` requires the callee to hand the minted authority back
/// through its capability-typed return, which places it in the caller's hands.
fn nested_flow_reaches_caller(
    program: &TypedTrees,
    edge: &CallEdge,
    kind: CapabilityFlowKind,
) -> bool {
    if kind == CapabilityFlowKind::Acquires {
        return state_for_symbol(program, edge.target_state_symbol)
            .map(|callee| is_capability_type(program, callee.return_type))
            .unwrap_or(false);
    }

    let Some(machine) = machine_for_symbol(program, edge.caller_machine_symbol) else {
        return false;
    };
    let Some(state) = enclosing_state(program, machine, edge.caller_state_symbol) else {
        return false;
    };

    let returns_capability = is_capability_type(program, state.return_type);

    match kind {
        CapabilityFlowKind::Returns => returns_capability,
        CapabilityFlowKind::Derives => {
            returns_capability || state_has_capability_parameter(program, state)
        }
        _ => false,
    }
}

/// Whether `state` declares a non-self capability-typed parameter.
fn state_has_capability_parameter(program: &TypedTrees, state: &State) -> bool {
    program.state_parameters(state).iter().any(|parameter| {
        !parameter.is_self && is_capability_type(program, parameter.type_reference)
    })
}

#[allow(clippy::too_many_arguments)]
fn push_unique(
    flows: &mut Arena<CapabilityFlowFact>,
    kind: CapabilityFlowKind,
    capability_symbol: SymbolHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    via_state_symbol: SymbolHandle,
) -> bool {
    let fact = CapabilityFlowFact {
        kind,
        capability_symbol,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
        via_state_symbol,
    };

    let exists = flows.iter().any(|(_, existing)| *existing == fact);
    if !exists {
        flows.append(fact);
    }
    !exists
}

/// Classifies the receiver of the boundary call at `statement_index` as
/// caller-provided (rooted at a state parameter) or machine-owned (rooted at
/// `self`, i.e. a retained field).
fn call_receiver_provenance(
    program: &TypedTrees,
    state: &State,
    statement_index: usize,
) -> ReceiverProvenance {
    let Some(statement) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return ReceiverProvenance::Unknown;
    };

    let head = match statement {
        StatementNode::AssemblyFact(_) => None,
        StatementNode::Call(call) => program
            .statement_table
            .name_path_members(call.receiver)
            .first()
            .map(|identifier| identifier.as_str().to_owned()),
        StatementNode::Assignment(assignment) => {
            expression_receiver_head(program, assignment.value)
        }
        StatementNode::LocalData(local_data) => {
            expression_receiver_head(program, local_data.initial_value)
        }
        StatementNode::Expression(expression) => expression_receiver_head(program, *expression),
        StatementNode::Transition(_) => None,
    };

    let Some(head) = head else {
        return ReceiverProvenance::Unknown;
    };

    if head == "self" {
        return ReceiverProvenance::MachineOwned;
    }

    if program
        .state_parameters(state)
        .iter()
        .any(|parameter| !parameter.is_self && parameter.name.as_str() == head)
    {
        return ReceiverProvenance::CallerProvided;
    }

    ReceiverProvenance::Unknown
}

/// Extracts the head identifier of the receiver of a call nested in an
/// expression (`self.x = receiver.call()` / `let y = receiver.call()`).
fn expression_receiver_head(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => expression_path_head(program, call.receiver),
        ExpressionNode::Borrow(inner) => expression_receiver_head(program, inner.target),
        ExpressionNode::Cast(cast) => expression_receiver_head(program, cast.value),
        _ => None,
    }
}

fn expression_path_head(
    program: &TypedTrees,
    receiver: typed_trees::expression::ExpressionHandle,
) -> Option<String> {
    if !receiver.is_valid() {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Member(member) => expression_path_head(program, member.receiver),
        ExpressionNode::Borrow(inner) => expression_path_head(program, inner.target),
        ExpressionNode::Name(name) => program
            .expression_table
            .name_path_members(name.members)
            .first()
            .map(|identifier| identifier.as_str().to_owned()),
        _ => None,
    }
}

fn machine_for_symbol(program: &TypedTrees, machine_symbol: SymbolHandle) -> Option<&Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}

fn enclosing_state<'program>(
    program: &'program TypedTrees,
    machine: &Machine,
    state_symbol: SymbolHandle,
) -> Option<&'program State> {
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

/// Finds a state by symbol across every machine in the program.
fn state_for_symbol(program: &TypedTrees, state_symbol: SymbolHandle) -> Option<&State> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })
}

/// Whether the assignment at `statement_index` stores its value into a field
/// (`self.field = ...`).
fn call_stored_into_field(program: &TypedTrees, state: &State, statement_index: usize) -> bool {
    let Some(statement) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };

    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    assignment_target_is_field(program, assignment.target)
}

fn assignment_target_is_field(
    program: &TypedTrees,
    target: typed_trees::expression::ExpressionHandle,
) -> bool {
    if !target.is_valid() {
        return false;
    }

    match program.expression_table.expression(target) {
        ExpressionNode::Member(_) => true,
        ExpressionNode::Borrow(inner) => assignment_target_is_field(program, inner.target),
        ExpressionNode::Name(name) => {
            // A multi-segment name path rooted at `self` (e.g. `self.field`).
            program
                .expression_table
                .name_path_members(name.members)
                .len()
                > 1
        }
        _ => false,
    }
}

/// Whether a boundary signature takes another capability as an argument, making
/// its result a derived sub-capability.
fn passes_capability_argument(program: &TypedTrees, signature: &StateSignature) -> bool {
    program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .any(|parameter| is_capability_type(program, parameter.type_reference))
}

/// Whether a type reference resolves to a capability type: a non-primitive
/// named/data type carried through boundary surfaces (e.g. `Folder`). Primitive
/// types are never capabilities.
fn is_capability_type(
    program: &TypedTrees,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    if program.primitive_type_reference(type_reference).is_some() {
        return false;
    }
    program.type_reference_symbol(type_reference).is_valid()
}

/// Resolves a call target to the boundary trait it reaches and the boundary
/// signature, if any. The target may be a boundary trait signature directly, or
/// an in-package implementation state whose machine conforms to a boundary trait.
fn boundary_capability(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<(SymbolHandle, Option<&StateSignature>)> {
    if !target_symbol.is_valid() {
        return None;
    }

    if let Some(found) = program.traits().iter().find_map(|trait_definition| {
        if !trait_definition.is_boundary {
            return None;
        }
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
            .map(|signature| (trait_definition.symbol, Some(signature)))
    }) {
        return Some(found);
    }

    // The target is an implementation state; map it back to the boundary trait
    // signature its machine conforms to, matched by state name.
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
            .map(|state| (machine, state))
    })?;

    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == conformance.symbol)
        else {
            continue;
        };
        if !trait_definition.is_boundary {
            continue;
        }
        if let Some(signature) = program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.name == state.name)
        {
            return Some((trait_definition.symbol, Some(signature)));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use checked_trees::FlowFacts;
    use source_files_to_tokens::Lexer;
    use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use tokens_to_syntax_trees::parse_syntax_trees;

    fn capability_plan(source: &str) -> (TypedTrees, CapabilityFlowPlan) {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
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
        plan.flows().any(|flow| {
            flow.kind == kind && flow.state_symbol == state && flow.via_state_symbol == via
        })
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

            data Vault {
                root: RootDir;
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
                machine subfolder(parent: Folder) -> SubFolder
                reaches
                    Workspace;
            }

            data Broker {
                workspace: Workspace;
            }

            machine Broker::narrow(&self, folder: Folder) -> SubFolder {
                self.workspace.subfolder(folder);
            }

            machine Broker::delegate(&self, folder: Folder) -> SubFolder {
                self.narrow(folder);
            }

            data Main {
                broker: Broker;
                folder: Folder;
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

            data Vault {
                root: RootDir;
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

            data Archiver {
                disk: Disk;
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

            data Vault {
                root: RootDir;
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
}
