//! Direct synchronous boundary-invocation inference.
//!
//! `reaches` is deliberately transitive and service-shaped. This plan keeps
//! the separate `invokes` axis precise enough to substitute callable binding
//! parameters through checked helpers: `Parameter(n)` always denotes the
//! nth non-`self` entry parameter of the machine whose summary carries it.

use crate::declarations::data_queries::data_definition_by_symbol;
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::{StateParameter, StateSignature};
use typed_trees::state::State;
use typed_trees::statement::{TableCall, TableNamePath};

use super::call_sites::{CallSiteVisitor, visit_state_calls};
use flow_effects::{InvocationInferencePlan, InvocationTarget, MachineInvocationInference};

use crate::proof_contracts::default_domains::call_summaries::{
    StateToMachine, state_to_machine_index,
};

#[derive(Debug, Clone)]
struct MachineWork {
    symbol: SymbolHandle,
    family_members: Vec<SymbolHandle>,
    published: Vec<InvocationTarget>,
    uses_published: bool,
    direct: Vec<InvocationTarget>,
    transitive: Vec<InvocationTarget>,
    calls: Vec<CallWork>,
}

#[derive(Debug, Clone)]
struct CallWork {
    target_machine: SymbolHandle,
    arguments: Vec<Option<InvocationTarget>>,
    declared: Vec<InvocationTarget>,
}

pub fn infer_synchronous_invocations(
    program: &dyn crate::CheckingInput,
) -> InvocationInferencePlan {
    let state_to_machine = state_to_machine_index(program);
    let mut work = program
        .machines()
        .iter()
        .map(|machine| build_machine_work(program, machine, &state_to_machine))
        .collect::<Vec<_>>();

    let operational = super::infer_operational_may(program);
    for (_, dependency) in operational.cleanup_dependencies.iter() {
        if let Some(caller) = work
            .iter_mut()
            .find(|machine| machine.symbol == dependency.caller_machine)
        {
            caller.calls.push(CallWork {
                target_machine: dependency.target_machine,
                // The hook borrows its nominal subject; it does not receive
                // a callable Binding parameter from an authored argument.
                arguments: Vec::new(),
                declared: Vec::new(),
            });
        }
    }

    loop {
        let previous = work
            .iter()
            .map(|machine| machine.transitive.clone())
            .collect::<Vec<_>>();
        for index in 0..work.len() {
            let mut transitive = work[index].direct.clone();
            for member in &work[index].family_members {
                if let Some(callee) = work.iter().find(|candidate| candidate.symbol == *member) {
                    let effective = if callee.uses_published {
                        &callee.published
                    } else {
                        &callee.transitive
                    };
                    // Invocation parameters are positional, so the family
                    // signature already supplies their shared identity.
                    extend_targets(&mut transitive, effective);
                }
            }
            for call in work[index].calls.clone() {
                extend_targets(&mut transitive, &call.declared);
                let Some(callee) = work
                    .iter()
                    .find(|candidate| candidate.symbol == call.target_machine)
                else {
                    continue;
                };
                let effective = if callee.uses_published {
                    &callee.published
                } else {
                    &callee.transitive
                };
                for target in effective {
                    if let Some(target) = substitute_target(*target, &call.arguments) {
                        typed_trees::invocations::insert_target(&mut transitive, target);
                    }
                }
            }
            work[index].transitive = transitive;
        }
        if work
            .iter()
            .map(|machine| &machine.transitive)
            .eq(previous.iter())
        {
            break;
        }
    }

    // Publish virtual interfaces only after the shared fixed point completes.
    // Member contracts remain unchanged, and membership adds no call edges.
    for machine in &mut work {
        if !machine.family_members.is_empty() {
            machine.published = machine.transitive.clone();
            machine.uses_published = true;
        }
    }

    InvocationInferencePlan {
        machines: work
            .into_iter()
            .map(|machine| {
                let effective = if machine.uses_published {
                    machine.published.clone()
                } else {
                    machine.transitive.clone()
                };
                MachineInvocationInference {
                    machine: machine.symbol,
                    published: machine.published,
                    inferred_direct: machine.direct,
                    inferred_transitive: machine.transitive,
                    effective,
                }
            })
            .collect(),
    }
}

pub fn declared_signature_invocations(
    program: &dyn crate::CheckingInput,
    signature: &StateSignature,
) -> Vec<InvocationTarget> {
    typed_trees::invocations::declared_signature_invocations(program.typed_program(), signature)
}

/// Rejoin the inference vocabulary's non-receiver ordinal to the full
/// positional signature used by explicit static calls and refinement.
pub(crate) fn full_parameter_invocation(
    target: InvocationTarget,
    parameters: &[StateParameter],
) -> Option<InvocationTarget> {
    match target {
        InvocationTarget::Parameter(ordinal) => parameters
            .iter()
            .enumerate()
            .nth(ordinal as usize)
            .and_then(|(position, _)| u32::try_from(position).ok())
            .map(InvocationTarget::Parameter),
        target => Some(target),
    }
}

fn explicit_signature_invocations(
    program: &dyn crate::CheckingInput,
    signature: &StateSignature,
) -> Vec<InvocationTarget> {
    declared_signature_invocations(program, signature)
        .into_iter()
        .filter_map(|target| {
            full_parameter_invocation(target, program.state_signature_parameters(signature))
        })
        .collect()
}

/// Exact normalized target set authored by one machine's `invokes` clauses.
///
/// This is the same resolver used by synchronous-invocation inference. Callers
/// that retain per-occurrence provenance may compare its cardinality with the
/// authored declaration list to reject unresolved or duplicate targets.
pub fn declared_machine_invocations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> Vec<InvocationTarget> {
    typed_trees::invocations::declared_machine_invocations(program.typed_program(), machine)
}

pub fn invocation_target_label(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    target: InvocationTarget,
) -> String {
    match target {
        InvocationTarget::Parameter(index) => {
            typed_trees::invocations::machine_entry_parameters(program.typed_program(), machine)
                .get(index as usize)
                .map(|parameter| parameter.name.as_str().to_owned())
                .unwrap_or_else(|| format!("parameter#{index}"))
        }
        InvocationTarget::Service(symbol) => program
            .trait_definition_by_symbol(symbol)
            .map(|definition| definition.name.as_str().to_owned())
            .unwrap_or_else(|| format!("service#{}", symbol.arena_index())),
    }
}

/// Whether a checked adapter uses the selected boundary receiver as its one
/// extra leading non-`self` parameter. Composition forwards that value into
/// the adapter but keeps calls through it inside the selected provider
/// artifact; it is not a component-boundary invocation target.
pub fn has_self_forwarded_boundary_parameter(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    boundary: SymbolHandle,
    requirement_parameter_count: usize,
) -> bool {
    let Some(state) = program.machine_states(machine).first() else {
        return false;
    };
    let parameters = program.state_parameters(state).iter().collect::<Vec<_>>();
    parameters.len() == requirement_parameter_count + 1
        && parameters.first().is_some_and(|parameter| {
            program
                .type_reference_table
                .type_reference(parameter.type_reference)
                .type_symbol(&program.type_reference_table)
                == boundary
        })
}

fn build_machine_work(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state_to_machine: &StateToMachine,
) -> MachineWork {
    let family_members = super::family_interface_members(program, machine)
        .iter()
        .map(|member| member.machine)
        .collect::<Vec<_>>();
    let published = if family_members.is_empty() {
        declared_machine_invocations(program, machine)
    } else {
        Vec::new()
    };
    let mut direct = Vec::new();
    let mut calls = Vec::new();
    for state in program.machine_states(machine) {
        let mut sites = InvocationCallSites {
            program,
            machine,
            state,
            state_to_machine,
            direct: &mut direct,
            calls: &mut calls,
        };
        visit_state_calls(program, machine.symbol, state, &mut sites);
    }
    MachineWork {
        symbol: machine.symbol,
        uses_published: family_members.is_empty()
            && (machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
                || machine.is_public
                || !program.machine_invokes(machine).is_empty()),
        family_members,
        published: published.clone(),
        direct: direct.clone(),
        transitive: direct,
        calls,
    }
}

/// Records the direct invocation targets of one machine. The walk itself
/// belongs to `call_sites`; this pass supplies only what it records per site.
struct InvocationCallSites<'a> {
    program: &'a dyn crate::CheckingInput,
    machine: &'a Machine,
    state: &'a State,
    state_to_machine: &'a StateToMachine,
    direct: &'a mut Vec<InvocationTarget>,
    calls: &'a mut Vec<CallWork>,
}

impl CallSiteVisitor for InvocationCallSites<'_> {
    fn statement_call(&mut self, call: &TableCall, _statement_index: usize, _ordinal: usize) {
        let receiver = origin_for_statement_receiver(self.program, self.machine, self.state, call);
        self.resolved_call(
            call.target_symbol,
            call.static_machine_parameter,
            receiver,
            self.program
                .statement_table
                .expression_handles(call.arguments),
        );
    }

    fn expression_call(
        &mut self,
        call: &TableCallExpression,
        _statement_index: usize,
        _ordinal: usize,
    ) {
        // A designated product selection executes a compiler-owned operation,
        // not a user-selected component binding. Its private host protocol is
        // explicit in Terminal, but is not a new authored `invokes` obligation.
        // The common traversal still visits every operand's ordinary calls;
        // selecting a product cannot hide a helper's actual service invocations.
        if self
            .program
            .expression_table
            .root_binding_request(call.root_binding)
            .is_some()
        {
            return;
        }
        let receiver = origin_for_expression(self.program, self.machine, self.state, call.receiver);
        self.resolved_call(
            call.target_symbol,
            call.static_machine_parameter,
            receiver,
            self.program
                .expression_table
                .expression_handles(call.arguments),
        );
    }

    fn named_transfer(
        &mut self,
        target: SymbolHandle,
        path: &TableNamePath,
        arguments: HandleSpan<ExpressionHandle>,
        static_machine_parameter: SymbolHandle,
        _acknowledgement: language_semantics::CallOperationalAcknowledgement,
        _statement_index: usize,
        _ordinal: usize,
    ) {
        let members = self.program.statement_table.name_path_members(path.members);
        let receiver = members.split_last().and_then(|(_, receiver)| {
            if receiver.is_empty() {
                return None;
            }
            origin_for_symbol(self.program, self.machine, self.state, path.head_symbol).or_else(
                || {
                    boundary_service_for_receiver_path(
                        self.program,
                        self.machine,
                        self.state,
                        path.head_symbol,
                        receiver,
                    )
                    .map(InvocationTarget::Service)
                },
            )
        });
        let arguments = self
            .program
            .statement_table
            .expression_handles(arguments)
            .iter()
            .map(|argument| {
                origin_for_expression(self.program, self.machine, self.state, *argument)
            })
            .collect();
        let direct = &mut *self.direct;
        let calls = &mut *self.calls;
        push_call(
            self.program,
            target,
            static_machine_parameter,
            receiver,
            arguments,
            direct,
            calls,
            self.state_to_machine,
        );
    }
}

impl InvocationCallSites<'_> {
    /// Shared call collection for statement and expression call sites: the
    /// receiver and every argument resolve through `origin_for_expression`,
    /// then the work item lands through `push_call`.
    fn resolved_call(
        &mut self,
        target_symbol: SymbolHandle,
        static_machine_parameter: SymbolHandle,
        receiver: Option<InvocationTarget>,
        argument_handles: &[ExpressionHandle],
    ) {
        let arguments = argument_handles
            .iter()
            .map(|argument| {
                origin_for_expression(self.program, self.machine, self.state, *argument)
            })
            .collect::<Vec<_>>();
        let direct = &mut *self.direct;
        let calls = &mut *self.calls;
        push_call(
            self.program,
            target_symbol,
            static_machine_parameter,
            receiver,
            arguments,
            direct,
            calls,
            self.state_to_machine,
        );
    }
}

fn origin_for_statement_receiver(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    call: &TableCall,
) -> Option<InvocationTarget> {
    let path = program.statement_table.name_path_members(call.receiver);
    if let Some(origin) = origin_for_symbol(program, machine, state, call.receiver_symbol) {
        return Some(origin);
    }
    if let Some(service) =
        boundary_service_for_receiver_path(program, machine, state, call.receiver_symbol, path)
    {
        return Some(InvocationTarget::Service(service));
    }
    None
}

fn boundary_service_for_receiver_path(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    head_symbol: SymbolHandle,
    path: &[typed_trees::name::Identifier],
) -> Option<SymbolHandle> {
    let mut type_reference = type_reference_for_symbol(program, machine, state, head_symbol)
        .or_else(|| {
            let head = path.first()?.as_str();
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.name.as_str() == head)
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    program
                        .machine_owned_data(machine)
                        .iter()
                        .find(|owned| owned.name.as_str() == head)
                        .map(|owned| owned.type_reference)
                })
        })?;
    for member_name in path.iter().skip(1) {
        let owner_symbol = program
            .type_reference_table
            .type_reference(type_reference)
            .type_symbol(&program.type_reference_table);
        let definition = data_definition_by_symbol(program, owner_symbol)?;
        let field = program.data_members(definition).iter().find_map(|member| {
            let field = member.as_field()?;
            (field.name.as_str() == member_name.as_str()).then_some(field)
        })?;
        type_reference = field.type_reference;
    }
    typed_trees::invocations::boundary_service_for_type(program.typed_program(), type_reference)
}

fn type_reference_for_symbol(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    program
        .state_parameter_by_symbol(state, symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .machine_owned_data_by_symbol(machine, symbol)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            let definition = data_definition_by_symbol(program, machine.attached_data_symbol)?;
            program.data_members(definition).iter().find_map(|member| {
                let field = member.as_field()?;
                (field.symbol == symbol).then_some(field.type_reference)
            })
        })
}

/// A data definition by symbol, resolved through the build-scope memo when
/// one is open — receiver-path and attached-data walks otherwise re-scan
/// the declaration table per member hop.

fn push_call(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
    static_machine_parameter: SymbolHandle,
    receiver: Option<InvocationTarget>,
    arguments: Vec<Option<InvocationTarget>>,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
    state_to_machine: &StateToMachine,
) {
    let boundary_signature = boundary_trait_for_signature(program, target);
    let crosses_boundary = receiver.is_some() || boundary_signature.is_some();
    if let Some(receiver) = receiver {
        typed_trees::invocations::insert_target(direct, receiver);
    } else if let Some(service) = boundary_signature {
        typed_trees::invocations::insert_target(direct, InvocationTarget::Service(service));
    }

    // A component-boundary call contributes only its direct edge. The
    // callee's own `invokes` ceiling describes edges originating inside that
    // component and belongs in the realized provider graph, not in the
    // caller's refinement set. Local/helper calls still substitute their
    // declared binding parameters so forwarding is inferred precisely.
    let binder_signature = program
        .retained_static_machine_contract(static_machine_parameter)
        .map(typed_trees::data::MachineParameterContractView::signature)
        .or_else(|| {
            program
                .machine_parameter_signature(target)
                .map(|(_, signature)| signature)
        });
    let declared = if crosses_boundary {
        Vec::new()
    } else if let Some(signature) = binder_signature {
        explicit_signature_invocations(program, signature)
    } else {
        signature_for_symbol(program, target)
            .map(|signature| declared_signature_invocations(program, signature))
            .unwrap_or_default()
    }
    .into_iter()
    .filter_map(|target| substitute_target(target, &arguments))
    .collect::<Vec<_>>();
    calls.push(CallWork {
        // A boundary receiver is a modular call through its requirement. Do
        // not accidentally follow whichever checked adapter happened to
        // resolve by name before provider selection; that would collapse a
        // direct A -> B edge into the adapter's B -> ... implementation.
        target_machine: if crosses_boundary {
            SymbolHandle::invalid()
        } else {
            state_to_machine
                .get(&target)
                .copied()
                .unwrap_or_else(SymbolHandle::invalid)
        },
        arguments,
        declared,
    });
}

fn signature_for_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<&StateSignature> {
    if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
        return Some(signature);
    }
    program
        .trait_machine_signature(symbol)
        .map(|(_, signature)| signature)
}

fn boundary_trait_for_signature(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .find(|definition| {
            program
                .trait_machine_signature_by_symbol(definition, symbol)
                .is_some()
        })
        .map(|definition| definition.symbol)
}

fn origin_for_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> Option<InvocationTarget> {
    if !expression.is_valid() {
        return None;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => origin_for_symbol(program, machine, state, path.symbol),
        ExpressionNode::Member(member) => {
            origin_for_symbol(program, machine, state, member.member_symbol)
                .or_else(|| origin_for_expression(program, machine, state, member.receiver))
        }
        ExpressionNode::Borrow(inner) => {
            origin_for_expression(program, machine, state, inner.target)
        }
        _ => None,
    }
}

fn origin_for_symbol(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<InvocationTarget> {
    if !symbol.is_valid() {
        return None;
    }
    if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
        let service = typed_trees::invocations::boundary_service_for_type(
            program.typed_program(),
            parameter.type_reference,
        )?;
        if let Some(index) =
            typed_trees::invocations::machine_entry_parameters(program.typed_program(), machine)
                .iter()
                .position(|entry| {
                    entry.symbol == parameter.symbol
                        || entry.name.as_str() == parameter.name.as_str()
                })
        {
            return Some(InvocationTarget::Parameter(index as u32));
        }
        return Some(InvocationTarget::Service(service));
    }
    if let Some(owned) = program.machine_owned_data_by_symbol(machine, symbol) {
        return typed_trees::invocations::boundary_service_for_type(
            program.typed_program(),
            owned.type_reference,
        )
        .map(InvocationTarget::Service);
    }
    let attached = crate::declarations::data_queries::data_definition_by_symbol(
        program,
        machine.attached_data_symbol,
    );
    attached
        .and_then(|definition| {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                        Some(field)
                    }
                    _ => None,
                })
        })
        .and_then(|member| {
            typed_trees::invocations::boundary_service_for_type(
                program.typed_program(),
                member.type_reference,
            )
        })
        .map(InvocationTarget::Service)
}

fn substitute_target(
    target: InvocationTarget,
    arguments: &[Option<InvocationTarget>],
) -> Option<InvocationTarget> {
    match target {
        InvocationTarget::Parameter(index) => arguments.get(index as usize).copied().flatten(),
        InvocationTarget::Service(_) => Some(target),
    }
}

fn extend_targets(targets: &mut Vec<InvocationTarget>, source: &[InvocationTarget]) {
    for target in source {
        typed_trees::invocations::insert_target(targets, *target);
    }
}
