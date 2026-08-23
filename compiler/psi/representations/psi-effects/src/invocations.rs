//! Direct synchronous boundary-invocation inference.
//!
//! `reaches` is deliberately transitive and service-shaped. This plan keeps
//! the separate `invokes` axis precise enough to substitute callable binding
//! parameters through checked helpers: `Parameter(n)` always denotes the
//! nth non-`self` entry parameter of the machine whose summary carries it.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateSignature;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableCall};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationTarget {
    /// Positional identity in the callable's non-`self` entry parameters.
    Parameter(u32),
    /// A statically selected boundary-service binding with no parameter path.
    Service(SymbolHandle),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineInvocationInference {
    pub machine: SymbolHandle,
    pub published: Vec<InvocationTarget>,
    pub inferred_direct: Vec<InvocationTarget>,
    pub inferred_transitive: Vec<InvocationTarget>,
    pub effective: Vec<InvocationTarget>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvocationInferencePlan {
    pub machines: Vec<MachineInvocationInference>,
}

impl InvocationInferencePlan {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineInvocationInference> {
        self.machines
            .iter()
            .find(|summary| summary.machine == machine)
    }
}

#[derive(Debug, Clone)]
struct MachineWork {
    symbol: SymbolHandle,
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

pub fn infer_synchronous_invocations(program: &TypedTrees) -> InvocationInferencePlan {
    let mut work = program
        .machines()
        .iter()
        .map(|machine| build_machine_work(program, machine))
        .collect::<Vec<_>>();

    loop {
        let previous = work
            .iter()
            .map(|machine| machine.transitive.clone())
            .collect::<Vec<_>>();
        for index in 0..work.len() {
            let mut transitive = work[index].direct.clone();
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
                        insert_target(&mut transitive, target);
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
    program: &TypedTrees,
    signature: &StateSignature,
) -> Vec<InvocationTarget> {
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    declared_targets(
        program,
        program.state_signature_invokes(signature),
        &parameters,
    )
}

pub fn invocation_target_label(
    program: &TypedTrees,
    machine: &Machine,
    target: InvocationTarget,
) -> String {
    match target {
        InvocationTarget::Parameter(index) => machine_entry_parameters(program, machine)
            .get(index as usize)
            .map(|parameter| parameter.name.as_str().to_owned())
            .unwrap_or_else(|| format!("parameter#{index}")),
        InvocationTarget::Service(symbol) => program
            .traits()
            .iter()
            .find(|definition| definition.symbol == symbol)
            .map(|definition| definition.name.as_str().to_owned())
            .unwrap_or_else(|| format!("service#{}", symbol.arena_index())),
    }
}

/// Whether a checked adapter uses the selected boundary receiver as its one
/// extra leading non-`self` parameter. Composition forwards that value into
/// the adapter but keeps calls through it inside the selected provider
/// artifact; it is not a component-boundary invocation target.
pub fn has_self_forwarded_boundary_parameter(
    program: &TypedTrees,
    machine: &Machine,
    boundary: SymbolHandle,
    requirement_parameter_count: usize,
) -> bool {
    let Some(state) = program.machine_states(machine).first() else {
        return false;
    };
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    parameters.len() == requirement_parameter_count + 1
        && parameters.first().is_some_and(|parameter| {
            program
                .type_reference_table
                .type_reference(parameter.type_reference)
                .type_symbol(&program.type_reference_table)
                == boundary
        })
}

fn build_machine_work(program: &TypedTrees, machine: &Machine) -> MachineWork {
    let entry_parameters = machine_entry_parameters(program, machine);
    let published = declared_targets(program, program.machine_invokes(machine), &entry_parameters);
    let mut direct = Vec::new();
    let mut calls = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            collect_statement_calls(program, machine, state, statement, &mut direct, &mut calls);
        }
    }
    MachineWork {
        symbol: machine.symbol,
        published: published.clone(),
        uses_published: machine.supply_mode
            != psi_language_semantics::MachineSupplyMode::CheckedBody
            || !program.machine_invokes(machine).is_empty(),
        direct: direct.clone(),
        transitive: direct,
        calls,
    }
}

fn collect_statement_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement: &StatementNode,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
) {
    match statement {
        StatementNode::AssemblyFact(_) => {}
        StatementNode::Assignment(assignment) => {
            collect_expression_calls(program, machine, state, assignment.target, direct, calls);
            collect_expression_calls(program, machine, state, assignment.value, direct, calls);
        }
        StatementNode::Call(call) => {
            collect_table_call(program, machine, state, call, direct, calls);
            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_calls(program, machine, state, *argument, direct, calls);
            }
        }
        StatementNode::Expression(expression) => {
            collect_expression_calls(program, machine, state, *expression, direct, calls)
        }
        StatementNode::LocalData(local) if local.initial_value.is_valid() => {
            collect_expression_calls(program, machine, state, local.initial_value, direct, calls);
        }
        StatementNode::Transition(transition) => {
            if let psi_typed_trees::statement::TransitionGuardNode::When(guard) = transition.guard {
                collect_expression_calls(program, machine, state, guard, direct, calls);
            }
            collect_transition_target_expression_calls(
                program,
                machine,
                state,
                transition.target,
                direct,
                calls,
            );
            if transition.continuation.is_valid() {
                collect_transition_target_expression_calls(
                    program,
                    machine,
                    state,
                    transition.continuation,
                    direct,
                    calls,
                );
            }
        }
        StatementNode::LocalData(_) => {}
    }
}

fn collect_transition_target_expression_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
) {
    if !target.is_valid() {
        return;
    }
    match program.statement_table.transition_target(target) {
        psi_typed_trees::statement::TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_calls(program, machine, state, *argument, direct, calls);
            }
        }
        psi_typed_trees::statement::TransitionTargetNode::Value(expression) => {
            collect_expression_calls(program, machine, state, *expression, direct, calls);
        }
        psi_typed_trees::statement::TransitionTargetNode::SelfTarget
        | psi_typed_trees::statement::TransitionTargetNode::Terminal => {}
    }
}

fn collect_expression_calls(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
) {
    if !expression.is_valid() {
        return;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(atomic) => {
            collect_expression_calls(program, machine, state, atomic.value, direct, calls)
        }
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_calls(program, machine, state, *value, direct, calls);
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_calls(program, machine, state, binary.left, direct, calls);
            collect_expression_calls(program, machine, state, binary.right, direct, calls);
        }
        ExpressionNode::Cast(cast) => {
            collect_expression_calls(program, machine, state, cast.value, direct, calls)
        }
        ExpressionNode::Call(call) => {
            collect_expression_call(program, machine, state, call, direct, calls);
            collect_expression_calls(program, machine, state, call.receiver, direct, calls);
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_calls(program, machine, state, *argument, direct, calls);
            }
        }
        ExpressionNode::Indexed(indexed) => {
            collect_expression_calls(program, machine, state, indexed.collection, direct, calls);
            collect_expression_calls(program, machine, state, indexed.index, direct, calls);
        }
        ExpressionNode::Member(member) => {
            collect_expression_calls(program, machine, state, member.receiver, direct, calls)
        }
        ExpressionNode::Mutable(inner) => {
            collect_expression_calls(program, machine, state, *inner, direct, calls)
        }
        ExpressionNode::Unary(unary) => {
            collect_expression_calls(program, machine, state, unary.operand, direct, calls)
        }
        ExpressionNode::Range(range) => {
            collect_expression_calls(program, machine, state, range.start, direct, calls);
            collect_expression_calls(program, machine, state, range.end, direct, calls);
        }
        ExpressionNode::StructLiteral(literal) => {
            for field in program.expression_table.struct_fields(literal.fields) {
                collect_expression_calls(program, machine, state, field.value, direct, calls);
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => {}
    }
}

fn collect_table_call(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCall,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
) {
    let receiver = origin_for_statement_receiver(program, machine, state, call);
    let arguments = program
        .statement_table
        .expression_handles(call.arguments)
        .iter()
        .map(|argument| origin_for_expression(program, machine, state, *argument))
        .collect::<Vec<_>>();
    push_call(
        program,
        call.target_symbol,
        receiver,
        arguments,
        direct,
        calls,
    );
}

fn origin_for_statement_receiver(
    program: &TypedTrees,
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
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    head_symbol: SymbolHandle,
    path: &[psi_typed_trees::name::Identifier],
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
        let definition = program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == owner_symbol)?;
        let field = program.data_members(definition).iter().find_map(|member| {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                return None;
            };
            (field.name.as_str() == member_name.as_str()).then_some(field)
        })?;
        type_reference = field.type_reference;
    }
    boundary_service_for_type(program, type_reference)
}

fn type_reference_for_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<psi_typed_trees::types::TypeReferenceHandle> {
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.symbol == symbol)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            let definition = machine.attached_data.as_ref().and_then(|name| {
                program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == name.as_str())
            })?;
            program.data_members(definition).iter().find_map(|member| {
                let psi_typed_trees::data::DataMember::Field(field) = member else {
                    return None;
                };
                (field.symbol == symbol).then_some(field.type_reference)
            })
        })
}

fn collect_expression_call(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    call: &TableCallExpression,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
) {
    let receiver = origin_for_expression(program, machine, state, call.receiver);
    let arguments = program
        .expression_table
        .expression_handles(call.arguments)
        .iter()
        .map(|argument| origin_for_expression(program, machine, state, *argument))
        .collect::<Vec<_>>();
    push_call(
        program,
        call.target_symbol,
        receiver,
        arguments,
        direct,
        calls,
    );
}

fn push_call(
    program: &TypedTrees,
    target: SymbolHandle,
    receiver: Option<InvocationTarget>,
    arguments: Vec<Option<InvocationTarget>>,
    direct: &mut Vec<InvocationTarget>,
    calls: &mut Vec<CallWork>,
) {
    let boundary_signature = boundary_trait_for_signature(program, target);
    let crosses_boundary = receiver.is_some() || boundary_signature.is_some();
    if let Some(receiver) = receiver {
        insert_target(direct, receiver);
    } else if let Some(service) = boundary_signature {
        insert_target(direct, InvocationTarget::Service(service));
    }

    // A component-boundary call contributes only its direct edge. The
    // callee's own `invokes` ceiling describes edges originating inside that
    // component and belongs in the realized provider graph, not in the
    // caller's refinement set. Local/helper calls still substitute their
    // declared binding parameters so forwarding is inferred precisely.
    let declared = (!crosses_boundary)
        .then(|| signature_for_symbol(program, target))
        .flatten()
        .map(|signature| declared_signature_invocations(program, signature))
        .unwrap_or_default()
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
            machine_symbol_for_state(program, target)
        },
        arguments,
        declared,
    });
}

fn signature_for_symbol(program: &TypedTrees, symbol: SymbolHandle) -> Option<&StateSignature> {
    if let Some((_, signature)) = program.machine_parameter_signature(symbol) {
        return Some(signature);
    }
    program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == symbol)
    })
}

fn boundary_trait_for_signature(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
        .find(|definition| {
            program
                .trait_machine_signatures(definition)
                .iter()
                .any(|signature| signature.symbol == symbol)
        })
        .map(|definition| definition.symbol)
}

fn machine_symbol_for_state(program: &TypedTrees, state: SymbolHandle) -> SymbolHandle {
    program
        .machines()
        .iter()
        .find(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state)
        })
        .map(|machine| machine.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn declared_targets(
    program: &TypedTrees,
    names: &[psi_typed_trees::name::Identifier],
    parameters: &[&psi_typed_trees::signature::StateParameter],
) -> Vec<InvocationTarget> {
    let mut targets = Vec::new();
    for name in names {
        if let Some(index) = parameters
            .iter()
            .position(|parameter| parameter.name.as_str() == name.as_str())
        {
            insert_target(&mut targets, InvocationTarget::Parameter(index as u32));
            continue;
        }
        if let Some(service) = program.traits().iter().find(|definition| {
            definition.is_boundary
                && (definition.name.as_str() == name.as_str()
                    || definition.name.as_str().rsplit("::").next() == Some(name.as_str()))
        }) {
            insert_target(&mut targets, InvocationTarget::Service(service.symbol));
        }
    }
    targets
}

fn machine_entry_parameters<'a>(
    program: &'a TypedTrees,
    machine: &Machine,
) -> Vec<&'a psi_typed_trees::signature::StateParameter> {
    program
        .machine_states(machine)
        .first()
        .map(|state| {
            program
                .state_parameters(state)
                .iter()
                .filter(|parameter| !parameter.is_self)
                .collect()
        })
        .unwrap_or_default()
}

fn origin_for_expression(
    program: &TypedTrees,
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
        ExpressionNode::Mutable(inner) => origin_for_expression(program, machine, state, *inner),
        _ => None,
    }
}

fn origin_for_symbol(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbol: SymbolHandle,
) -> Option<InvocationTarget> {
    if !symbol.is_valid() {
        return None;
    }
    if let Some(parameter) = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == symbol)
    {
        let service = boundary_service_for_type(program, parameter.type_reference)?;
        if let Some(index) = machine_entry_parameters(program, machine)
            .iter()
            .position(|entry| {
                entry.symbol == parameter.symbol || entry.name.as_str() == parameter.name.as_str()
            })
        {
            return Some(InvocationTarget::Parameter(index as u32));
        }
        return Some(InvocationTarget::Service(service));
    }
    if let Some(owned) = program
        .machine_owned_data(machine)
        .iter()
        .find(|owned| owned.symbol == symbol)
    {
        return boundary_service_for_type(program, owned.type_reference)
            .map(InvocationTarget::Service);
    }
    let attached = machine.attached_data.as_ref().and_then(|name| {
        program
            .data_definitions()
            .iter()
            .find(|definition| definition.name.as_str() == name.as_str())
    });
    attached
        .and_then(|definition| {
            program
                .data_members(definition)
                .iter()
                .find_map(|member| match member {
                    psi_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                        Some(field)
                    }
                    _ => None,
                })
        })
        .and_then(|member| boundary_service_for_type(program, member.type_reference))
        .map(InvocationTarget::Service)
}

fn boundary_service_for_type(
    program: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let symbol = program
        .type_reference_table
        .type_reference(type_reference)
        .type_symbol(&program.type_reference_table);
    program
        .traits()
        .iter()
        .any(|definition| definition.is_boundary && definition.symbol == symbol)
        .then_some(symbol)
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

fn insert_target(targets: &mut Vec<InvocationTarget>, target: InvocationTarget) {
    if !targets.contains(&target) {
        targets.push(target);
        targets.sort_by_key(|target| match target {
            InvocationTarget::Parameter(index) => (0u8, *index),
            InvocationTarget::Service(symbol) => (1u8, symbol.arena_index()),
        });
    }
}

fn extend_targets(targets: &mut Vec<InvocationTarget>, source: &[InvocationTarget]) {
    for target in source {
        insert_target(targets, *target);
    }
}
