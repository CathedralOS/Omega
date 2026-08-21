//! Conservative caller-visible write-frame inference.
//!
//! This module owns call-demand collection, internal and boundary call-frame
//! summaries, alias-origin propagation, and transition-cycle frame equations.
//! It produces complete caller-visible paths or fails closed as opaque; call
//! validity and type diagnostics remain owned by the parent module.

use super::receiver_member_chain;
use crate::arithmetic_domains;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::StateParameter;
use psi_typed_trees::state::State;
use psi_typed_trees::statement::{StatementNode, TableCall, TransitionTargetNode};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

mod boundary_calls;
mod demand;
mod isolation;

use boundary_calls::known_boundary_call_written_paths_for_parts;
pub(crate) use boundary_calls::{boundary_trait_signature, known_boundary_call_written_paths};
pub use demand::{CallFrameResolver, frame_paths_overlap};
use demand::{collect_expression_call_written_paths, syntactic_call_written_paths};
pub(crate) use demand::{conservative_call_written_paths, statement_value_expression_roots};
use isolation::{
    struct_literal_field_is_primitive, struct_literal_field_type,
    struct_literal_matches_expected_type, struct_literal_type_is_caller_isolated,
    type_is_caller_isolated_local,
};

/// The FREE top-level machine named `target` and its entry state (`machine
/// compute(item: &Item) -> i32 { ... }`), or None. The parser names a free
/// machine's implicit entry state `entry`; explicit entry states matching the
/// call target name win first.
pub(crate) fn free_machine_entry_state<'program>(
    program: &'program TypedTrees,
    symbols: &TopLevelSymbols<'program>,
    target: &str,
) -> Option<(&'program Machine, &'program State)> {
    let machine = symbols.machine(target)?;
    if machine.attached_data.is_some() {
        return None;
    }

    let states = program.machine_states(machine);
    states
        .iter()
        .find(|state| state.name.as_str() == target)
        .or_else(|| states.iter().find(|state| state.name.as_str() == "entry"))
        .or_else(|| states.first())
        .map(|state| (machine, state))
}

pub(super) fn machine_state_by_symbol(
    program: &TypedTrees,
    symbol: SymbolHandle,
) -> Option<(&Machine, &State)> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == symbol)
            .map(|state| (machine, state))
    })
}

/// Instantiate the conservative may-write set of a resolved internal call in
/// the caller's place namespace. `None` means the summary is not complete and
/// the caller must invalidate every flow fact. Internal acyclic calls and
/// state-transition graphs with complete expression frames compose;
/// implementation shapes this inference cannot summarize remain deliberately
/// opaque. Authored `stores` clauses are retired; precision grows through the
/// shared inferred complete-or-opaque frame instead.
pub(crate) fn known_call_written_paths(
    program: &TypedTrees,
    call: &TableCall,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
) -> Option<Vec<String>> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        program.statement_table.expression_handles(call.arguments),
        current_machine,
        machine_symbols,
        symbols,
        &mut Vec::new(),
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    known_call_written_paths_for_parts_with_origins(
        program,
        target_symbol,
        target,
        receiver_members,
        arguments,
        current_machine,
        machine_symbols,
        symbols,
        active_states,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn known_call_written_paths_for_parts_with_origins(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target: &str,
    receiver_members: &[String],
    arguments: &[ExpressionHandle],
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
) -> Option<Vec<String>> {
    // A static machine parameter's selected target is a specialization input,
    // not an ordinary receiver binding. Until MP summaries instantiate that
    // binding explicitly, retain the sound all-facts invalidation.
    if program
        .machine_parameter_signature_in(current_machine, target_symbol)
        .is_some()
    {
        return None;
    }
    let (callee_machine, callee_state) = if receiver_members.is_empty()
        || matches!(receiver_members, [receiver] if receiver == "self")
    {
        machine_state_by_symbol(program, target_symbol)
            .filter(|(machine, _)| machine.symbol != current_machine.symbol)
            .or_else(|| {
                machine_symbols
                    .state(target)
                    .map(|state| (current_machine, state))
            })
            .or_else(|| {
                current_machine
                    .attached_data
                    .as_ref()
                    .and_then(|attached_data| {
                        symbols.attached_machine_state(program, attached_data.as_str(), target)
                    })
            })
            .or_else(|| free_machine_entry_state(program, symbols, target))?
    } else {
        let receiver = receiver_members.last()?.as_str();
        let machine = machine_symbols
            .callable_field_type(receiver)
            .and_then(|type_name| symbols.machine(type_name))
            .or_else(|| symbols.machine(receiver))?;
        let state = program
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == target)?;
        (machine, state)
    };

    if active_states.contains(&callee_state.symbol) {
        return None;
    }
    active_states.push(callee_state.symbol);
    let result = summarize_resolved_call(
        program,
        arguments,
        callee_machine,
        callee_state,
        receiver_members,
        symbols,
        active_states,
        argument_origins,
    );
    active_states.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn summarize_resolved_call(
    program: &TypedTrees,
    arguments: &[ExpressionHandle],
    callee_machine: &Machine,
    callee_state: &State,
    receiver_members: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
) -> Option<Vec<String>> {
    let receiver_base = (!receiver_members.is_empty())
        .then(|| receiver_members.join("."))
        .or_else(|| {
            callee_machine
                .attached_data
                .as_ref()
                .map(|_| "self".to_owned())
        });
    let parameters = program.state_parameters(callee_state);
    let mut written = Vec::new();

    let relative_paths = summarize_state_written_paths(
        program,
        callee_machine,
        callee_state,
        symbols,
        active_states,
        &mut Vec::new(),
    )
    .or_else(|| {
        summarize_state_written_paths_with_permuted_cycles(
            program,
            callee_machine,
            callee_state,
            symbols,
            active_states,
        )
    })?;
    for relative in relative_paths {
        if let Some(instantiated) = instantiate_written_path_with_origins(
            program,
            &relative,
            receiver_base.as_deref(),
            parameters,
            arguments,
            &[],
            symbols,
            active_states,
            argument_origins,
        )? && !written.contains(&instantiated)
        {
            written.push(instantiated);
        }
    }

    Some(written)
}

fn summarize_state_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
) -> Option<Vec<String>> {
    if let Some((_, paths)) = complete_state_summaries
        .iter()
        .find(|(symbol, _)| *symbol == state.symbol)
    {
        return Some(paths.clone());
    }
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    let mut written = Vec::new();

    let mut nested_diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut nested_diagnostics);
    if !nested_diagnostics.is_empty() {
        return None;
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference) =>
            {
                stable_local_mutable_alias_origin(
                    program,
                    machine,
                    &machine_symbols,
                    active_states,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )
            }
            _ => None,
        };
        let representable_alias_rebinding = match statement {
            StatementNode::Assignment(assignment) => coarse_place_path(program, assignment.target)
                .is_some_and(|target| {
                    stable_local_mutable_alias_rebinding_is_representable(
                        program,
                        machine,
                        &machine_symbols,
                        active_states,
                        state,
                        &target,
                        assignment.value,
                        parameters,
                        &isolated_local_roots,
                        &local_alias_origins,
                        symbols,
                    )
                }),
            _ => false,
        };
        for expression in statement_value_expression_roots(program, statement) {
            if expression_reborrows_local_alias_binding(program, expression, &local_alias_origins)
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
            {
                return None;
            }
            let mut expression_writes = Vec::new();
            collect_expression_call_written_paths(
                program,
                expression,
                machine,
                &machine_symbols,
                symbols,
                active_states,
                &mut expression_writes,
            )?;
            for relative in expression_writes {
                let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                if relative_state_path_is_visible(&relative, parameters, &locals)?
                    && !written.contains(&relative)
                {
                    written.push(relative);
                }
            }
        }
        match statement {
            StatementNode::AssemblyFact(_) => {}
            StatementNode::Assignment(assignment) => {
                let direct_target = coarse_place_path(program, assignment.target);
                if let Some(relative) = direct_target.as_deref()
                    && rebind_stable_local_mutable_alias_origin(
                        program,
                        machine,
                        &machine_symbols,
                        active_states,
                        state,
                        relative,
                        assignment.value,
                        parameters,
                        &isolated_local_roots,
                        &mut local_alias_origins,
                        symbols,
                    )?
                {
                    continue;
                }
                let relative = stable_assignment_target_path(
                    program,
                    machine,
                    &machine_symbols,
                    active_states,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                if relative_state_path_is_visible(&relative, parameters, &locals)?
                    && !written.contains(&relative)
                {
                    written.push(relative);
                }
            }
            StatementNode::Call(nested_call) => {
                let nested_receiver_members = program
                    .statement_table
                    .name_path_members(nested_call.receiver)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect::<Vec<_>>();
                let arguments = program
                    .statement_table
                    .expression_handles(nested_call.arguments);
                let argument_origins = arguments
                    .iter()
                    .map(|argument| {
                        stable_alias_initializer_origin(
                            program,
                            machine,
                            &machine_symbols,
                            active_states,
                            *argument,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            symbols,
                            true,
                        )
                    })
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts_with_origins(
                    program,
                    nested_call.target_symbol,
                    nested_call.target.as_str(),
                    &nested_receiver_members,
                    arguments,
                    machine,
                    &machine_symbols,
                    symbols,
                    active_states,
                    Some(&argument_origins),
                )
                .or_else(|| {
                    (!arguments
                        .iter()
                        .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
                    .then(|| {
                        known_boundary_call_written_paths_for_parts(
                            program,
                            &machine_symbols,
                            symbols,
                            &nested_receiver_members,
                            nested_call.target.as_str(),
                            arguments,
                        )
                    })
                    .flatten()
                })
                .or_else(|| {
                    syntactic_call_written_paths(program, &nested_receiver_members, arguments)
                })?;
                for relative in nested_writes {
                    let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                    if relative_state_path_is_visible(&relative, parameters, &locals)?
                        && !written.contains(&relative)
                    {
                        written.push(relative);
                    }
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    if !local_alias_origins.is_empty()
                        && target.is_valid()
                        && matches!(
                            program.statement_table.transition_target(target),
                            TransitionTargetNode::Named { .. }
                        )
                        && !named_transition_subgraph_is_acyclic(program, machine, state, target)
                    {
                        // Alias origins compose positionally through acyclic
                        // named graphs. A reachable named SCC needs equation-
                        // level alias substitution, so keep it opaque here.
                        return None;
                    }
                    for relative in summarize_transition_target_written_paths(
                        program,
                        machine,
                        state,
                        target,
                        symbols,
                        active_states,
                        complete_state_summaries,
                        &locals,
                    )? {
                        let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                        if relative_state_path_is_visible(&relative, parameters, &locals)?
                            && !written.contains(&relative)
                        {
                            written.push(relative);
                        }
                    }
                }
            }
            StatementNode::Expression(_) => {}
            StatementNode::LocalData(local) => {
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)
                {
                    let origin = declared_local_alias_origin?;
                    local_alias_origins.push((local.name.as_str().to_owned(), origin));
                }
                if type_is_caller_isolated_local(program, local.type_reference) {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    complete_state_summaries.push((state.symbol, written.clone()));
    Some(written)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FramePathPrecision {
    Exact,
    CollectionCoarse,
}

#[derive(Debug, Clone)]
struct FramePlaceOrigin {
    path: String,
    precision: FramePathPrecision,
}

/// Recover the caller-visible origin of the deliberately narrow stable local-
/// alias shapes handled by the ordinary frame summarizer. A reborrow through
/// an already-known alias composes an exact member suffix onto that alias's
/// origin. An indexed reborrow is represented by its whole collection; once
/// coarse, later member suffixes may never narrow that collection again.
/// Caller-isolated locals are also stable origins, but remain local-only and
/// therefore disappear from the published caller frame. A structurally
/// transparent value call preserves such an origin just as it preserves a
/// caller-visible parameter origin. The compiler-owned `as_mut_slice` view
/// preserves its receiver's storage origin. A validated mutable recast is
/// address identity, so an effect-free recast source preserves the same origin.
/// Direct parameter-relative member and effect-free indexed projections compose
/// the same exact/coarse path algebra. Other computed results stay opaque.
fn stable_local_mutable_alias_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    local: &psi_typed_trees::statement::TableLocalData,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> Option<FramePlaceOrigin> {
    let TypeReferenceNode::Reference {
        is_mutable: true, ..
    } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    stable_alias_initializer_origin(
        program,
        current_machine,
        machine_symbols,
        active_states,
        local.initial_value,
        parameters,
        isolated_local_roots,
        aliases,
        symbols,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_initializer_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            active_states,
            *inner,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
        ExpressionNode::Indexed(indexed)
            if expression_is_effectful_for_transparent_result(program, indexed.index) =>
        {
            if !stable_alias_index_expression_preserves_origin(
                program,
                current_machine,
                indexed.index,
                machine_symbols,
                symbols,
                active_states,
                parameters,
                aliases,
                2,
            ) {
                return None;
            }
            let mut collection = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                active_states,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_initializer_origin(
                program,
                current_machine,
                machine_symbols,
                active_states,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => receiver,
            })
        }
        _ => stable_alias_expression_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
/// Admit the same bounded exact-call tree used by transparent statement-call
/// arguments in an alias index. Every call must have a complete frame, and no
/// node may reborrow a mutable-reference binding. The explicit depth budget
/// keeps deeper computation out of the returned-place relation.
fn stable_alias_index_expression_preserves_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, FramePlaceOrigin)],
    remaining_call_depth: usize,
) -> bool {
    if expression_reborrows_stable_alias_binding(program, expression, parameters, aliases) {
        return false;
    }
    if !expression_is_effectful_for_transparent_result(program, expression) {
        return true;
    }
    if remaining_call_depth == 0 {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    if call.receiver.is_valid()
        && expression_is_effectful_for_transparent_result(program, call.receiver)
    {
        return false;
    }
    let receiver_members = if call.receiver.is_valid() {
        let Some(receiver) = receiver_member_chain(program, call.receiver) else {
            return false;
        };
        receiver
    } else {
        Vec::new()
    };
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.iter().any(|argument| {
        !stable_alias_index_expression_preserves_origin(
            program,
            current_machine,
            *argument,
            machine_symbols,
            symbols,
            active_states,
            parameters,
            aliases,
            remaining_call_depth - 1,
        )
    }) {
        return false;
    }
    known_call_written_paths_for_parts(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        arguments,
        current_machine,
        machine_symbols,
        symbols,
        active_states,
    )
    .or_else(|| {
        known_boundary_call_written_paths_for_parts(
            program,
            machine_symbols,
            symbols,
            &receiver_members,
            call.target.as_str(),
            arguments,
        )
    })
    .is_some()
}

fn expression_reborrows_stable_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let visit =
        |child| expression_reborrows_stable_alias_binding(program, child, parameters, aliases);
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            let reborrows_binding = matches!(
                program.expression_table.expression(*inner),
                ExpressionNode::Name(_)
            ) && frame_place_path(program, *inner).is_some_and(|place| {
                let (root, suffix) = split_place_root(&place.path);
                suffix.is_empty()
                    && (parameters.iter().any(|parameter| {
                        matches!(
                            program
                                .type_reference_table
                                .type_reference(parameter.type_reference),
                            TypeReferenceNode::Reference {
                                is_mutable: true,
                                ..
                            }
                        ) && (parameter.is_self && root == "self"
                            || root == parameter.name.as_str())
                    }) || aliases.iter().any(|(name, _)| root == name))
            });
            reborrows_binding || visit(*inner)
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value) || visit(atomic.result),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && visit(call.receiver))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| visit(*argument))
        }
        ExpressionNode::Binary(binary) => visit(binary.left) || visit(binary.right),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => visit(indexed.collection) || visit(indexed.index),
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| visit(*element)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| visit(field.value)),
        ExpressionNode::Range(range) => visit(range.start) || visit(range.end),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn stable_alias_expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => stable_alias_expression_origin(
            program,
            *inner,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        ),
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return stable_alias_expression_origin(
                    program,
                    call.receiver,
                    parameters,
                    isolated_local_roots,
                    aliases,
                    symbols,
                    allow_isolated_local,
                );
            }
            transparent_call_result_origin(
                program,
                call,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            stable_alias_expression_origin(
                program,
                cast.value,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            let mut collection = stable_alias_expression_origin(
                program,
                indexed.collection,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = stable_alias_expression_origin(
                program,
                member.receiver,
                parameters,
                isolated_local_roots,
                aliases,
                symbols,
                allow_isolated_local,
            )?;
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => receiver,
            })
        }
        _ => stable_alias_place_origin(
            program,
            expression,
            parameters,
            isolated_local_roots,
            aliases,
            allow_isolated_local,
        ),
    }
}

fn stable_alias_place_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    let expression = match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => *inner,
        _ => expression,
    };
    let origin = frame_place_path(program, expression)?;
    let (root, suffix) = split_place_root(&origin.path);
    if root == "self"
        || parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == root)
        || (allow_isolated_local && isolated_local_roots.iter().any(|local| local == root))
    {
        return Some(origin);
    }
    let parent = aliases
        .iter()
        .find_map(|(alias, parent)| (alias == root).then_some(parent))?;
    if !allow_isolated_local
        && isolated_local_roots
            .iter()
            .any(|local| local == split_place_root(&parent.path).0)
    {
        return None;
    }
    Some(match parent.precision {
        FramePathPrecision::Exact => FramePlaceOrigin {
            path: append_place_suffix(&parent.path, suffix),
            precision: origin.precision,
        },
        FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
            path: parent.path.clone(),
            precision: FramePathPrecision::CollectionCoarse,
        },
    })
}

/// Resolve an assignment target using the established direct-place behavior,
/// plus the bounded structural origin algebra shared by stable aliases. This
/// admits a validated effectful index through a stable alias or transparent
/// helper result while preserving the depth, rebinding, and opacity fences.
#[allow(clippy::too_many_arguments)]
fn stable_assignment_target_path(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    target: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> Option<String> {
    if let Some(relative) = coarse_place_path(program, target) {
        return Some(rebase_local_alias_path(&relative, aliases));
    }
    Some(
        stable_alias_initializer_origin(
            program,
            current_machine,
            machine_symbols,
            active_states,
            target,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            true,
        )?
        .path,
    )
}

/// Recover one deliberately structural value-call relation. The helper may be
/// free or attached, but must be acyclic at the result surface, return `&mut`,
/// and have one terminal result expression rooted in one mutable-reference
/// parameter. A prefix may contain caller-isolated scratch locals and local
/// mutable-reference bindings that forward direct places from that parameter, an
/// earlier such local, or another structurally transparent helper. Value-shaped
/// assignments with effect-free right-hand sides may write through those
/// places, scratch locals, validated mutable recast aliases with effect-free
/// sources, or exact transparent call-produced targets without changing their
/// origins; the ordinary frame summary still publishes caller-visible writes.
/// Effect-free discarded expressions are also neutral.
/// A direct stable alias rebind updates only that local; prior reborrows retain
/// their established origins. Explicit arguments and an attached helper's
/// actual receiver both supply exact caller origins. This is body evidence, not
/// lifetime elision: a reference-bearing scratch local, opaque computed rebind,
/// discarded/statement call, recursive helper relation, named-state route, or
/// alternate result fails closed.
fn transparent_call_result_origin(
    program: &TypedTrees,
    call: &TableCallExpression,
    caller_parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    allow_isolated_local: bool,
) -> Option<FramePlaceOrigin> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)
        .or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }

    let result_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        &mut Vec::new(),
    )?;
    let parameters = program.state_parameters(callee_state);
    let result_parameter = parameters
        .iter()
        .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
    let (_, result_suffix) = split_place_root(&result_origin.place.path);
    let argument_origin = if result_parameter.is_self {
        if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
            return None;
        }
        stable_alias_expression_origin(
            program,
            call.receiver,
            caller_parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        )?
    } else {
        let (argument_index, _) = parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == result_parameter.symbol)?;
        let argument = *program
            .expression_table
            .expression_handles(call.arguments)
            .get(argument_index)?;
        stable_alias_expression_origin(
            program,
            argument,
            caller_parameters,
            isolated_local_roots,
            aliases,
            symbols,
            allow_isolated_local,
        )?
    };
    Some(match argument_origin.precision {
        FramePathPrecision::Exact => FramePlaceOrigin {
            path: append_place_suffix(&argument_origin.path, result_suffix),
            precision: result_origin.place.precision,
        },
        FramePathPrecision::CollectionCoarse => argument_origin,
    })
}

/// Recover a transparent returned-place origin without imposing a caller
/// namespace. This is used while instantiating a callee frame through a nested
/// statement-call argument: direct places keep their existing spelling, while
/// the compiler-owned `as_mut_slice` view preserves its receiver and a
/// structurally transparent helper selects and composes one of its actual
/// mutable-reference origins. Opaque or recursive helpers fail closed.
fn transparent_place_expression_origin(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            transparent_place_expression_origin(program, *inner, symbols, active_states)
        }
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                return None;
            }
            let mut origin = transparent_place_expression_origin(
                program,
                indexed.collection,
                symbols,
                active_states,
            )?;
            origin.precision = FramePathPrecision::CollectionCoarse;
            Some(origin)
        }
        ExpressionNode::Member(member) => {
            let origin = transparent_place_expression_origin(
                program,
                member.receiver,
                symbols,
                active_states,
            )?;
            Some(match origin.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", origin.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => origin,
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return transparent_place_expression_origin(
                    program,
                    call.receiver,
                    symbols,
                    active_states,
                );
            }
            let (callee_machine, callee_state) =
                machine_state_by_symbol(program, call.target_symbol).or_else(|| {
                    (!call.receiver.is_valid())
                        .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                        .flatten()
                })?;
            if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
                return None;
            }
            let result_origin = transparent_callee_result_origin(
                program,
                callee_machine,
                callee_state,
                symbols,
                active_states,
            )?;
            let parameters = program.state_parameters(callee_state);
            let result_parameter = parameters
                .iter()
                .find(|parameter| parameter.symbol == result_origin.parameter_symbol)?;
            let actual = if result_parameter.is_self {
                if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
                    return None;
                }
                call.receiver
            } else {
                let (argument_index, _) = parameters
                    .iter()
                    .filter(|parameter| !parameter.is_self)
                    .enumerate()
                    .find(|(_, parameter)| parameter.symbol == result_parameter.symbol)?;
                *program
                    .expression_table
                    .expression_handles(call.arguments)
                    .get(argument_index)?
            };
            let actual_origin =
                transparent_place_expression_origin(program, actual, symbols, active_states)?;
            let (_, suffix) = split_place_root(&result_origin.place.path);
            Some(match actual_origin.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: append_place_suffix(&actual_origin.path, suffix),
                    precision: result_origin.place.precision,
                },
                FramePathPrecision::CollectionCoarse => actual_origin,
            })
        }
        _ => frame_place_path(program, expression),
    }
}

fn transparent_callee_result_origin(
    program: &TypedTrees,
    callee_machine: &Machine,
    callee_state: &State,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<ParameterRelativeFrameOrigin> {
    if active_states.contains(&callee_state.symbol)
        || !matches!(
            program
                .type_reference_table
                .type_reference(callee_state.return_type),
            TypeReferenceNode::Reference {
                is_mutable: true,
                ..
            }
        )
    {
        return None;
    }
    active_states.push(callee_state.symbol);
    let result = (|| {
        let statements = program
            .statement_table
            .statements(callee_state.statement_nodes);
        let (StatementNode::Expression(result), prefix) = statements.split_last()? else {
            return None;
        };

        let parameters = program.state_parameters(callee_state);
        let mut local_aliases = Vec::new();
        let mut isolated_local_roots = Vec::new();
        for statement in prefix {
            match statement {
                StatementNode::LocalData(local) => {
                    if type_is_caller_isolated_local(program, local.type_reference) {
                        if !isolated_local_initializer_preserves_transparent_result(
                            program,
                            callee_machine,
                            local.initial_value,
                            symbols,
                            active_states,
                            &isolated_local_roots,
                        ) {
                            return None;
                        }
                        isolated_local_roots.push(local.name.as_str().to_owned());
                        continue;
                    }
                    let TypeReferenceNode::Reference {
                        is_mutable: true, ..
                    } = program
                        .type_reference_table
                        .type_reference(local.type_reference)
                    else {
                        return None;
                    };
                    let origin = parameter_relative_place_origin(
                        program,
                        callee_machine,
                        local.initial_value,
                        parameters,
                        &local_aliases,
                        symbols,
                        active_states,
                    )?;
                    local_aliases.push((local.name.as_str().to_owned(), local.symbol, origin));
                }
                StatementNode::Assignment(assignment) => {
                    if expression_is_effectful_for_transparent_result(program, assignment.target)
                        && (!transparent_assignment_target_effect_is_structural(
                            program,
                            assignment.target,
                        ) || parameter_relative_place_origin(
                            program,
                            callee_machine,
                            assignment.target,
                            parameters,
                            &local_aliases,
                            symbols,
                            active_states,
                        )
                        .is_none())
                    {
                        return None;
                    }
                    if value_expression_assignment_preserves_transparent_result(
                        program,
                        callee_machine,
                        assignment.value,
                        assignment_target_type(
                            program,
                            callee_machine,
                            callee_state,
                            assignment.target,
                        ),
                        symbols,
                        active_states,
                        parameters,
                        &local_aliases,
                    ) {
                        // A non-reference call result can change only the
                        // target's value, not an established alias origin.
                    } else if expression_may_rebind_mutable_alias(
                        program,
                        callee_machine,
                        callee_state,
                        assignment.value,
                    ) {
                        let position = parameter_relative_alias_position(
                            program,
                            assignment.target,
                            &local_aliases,
                        )?;
                        let replacement = parameter_relative_place_origin(
                            program,
                            callee_machine,
                            assignment.value,
                            parameters,
                            &local_aliases,
                            symbols,
                            active_states,
                        )?;
                        local_aliases[position].2 = replacement;
                    } else if expression_is_effectful_for_transparent_result(
                        program,
                        assignment.value,
                    ) {
                        return None;
                    }
                }
                StatementNode::Expression(expression)
                    if !expression_is_effectful_for_transparent_result(program, *expression) => {}
                StatementNode::Call(call)
                    if statement_call_preserves_transparent_result(
                        program,
                        callee_machine,
                        call,
                        symbols,
                        active_states,
                        parameters,
                        &local_aliases,
                    ) => {}
                _ => return None,
            }
        }
        parameter_relative_place_origin(
            program,
            callee_machine,
            *result,
            parameters,
            &local_aliases,
            symbols,
            active_states,
        )
    })();
    active_states.pop();
    result
}

/// A caller-isolated scratch local cannot itself redirect a returned place.
/// Its initializer may therefore precede a transparent returned-place result
/// when it is syntactically effect-free, or when it is a direct-call tree of
/// maximum depth two whose inferred frames are complete and write only
/// previously established caller-isolated scratch locals. Keep deeper or
/// computed call shapes and every caller-visible or opaque call fenced: this
/// predicate proves only that the initializer cannot perturb the returned-
/// place relation.
fn isolated_local_initializer_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    isolated_local_roots: &[String],
) -> bool {
    if !expression_is_effectful_for_transparent_result(program, expression) {
        return true;
    }
    if !isolated_local_initializer_call_tree_is_bounded(program, expression, 2) {
        return false;
    }

    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return false;
    }
    let mut written = Vec::new();
    collect_expression_call_written_paths(
        program,
        expression,
        current_machine,
        &machine_symbols,
        symbols,
        active_states,
        &mut written,
    )
    .is_some()
        && written.iter().all(|path| {
            let (root, _) = split_place_root(path);
            isolated_local_roots.iter().any(|local| local == root)
        })
}

/// Count only direct calls along initializer receiver/argument edges. Pure
/// leaves are neutral; calls hidden under operators, aggregates, or other
/// computed expressions remain outside this deliberately small relation.
fn isolated_local_initializer_call_tree_is_bounded(
    program: &TypedTrees,
    expression: ExpressionHandle,
    remaining_call_depth: usize,
) -> bool {
    if !expression_is_effectful_for_transparent_result(program, expression) {
        return true;
    }
    if remaining_call_depth == 0 {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    (!call.receiver.is_valid()
        || isolated_local_initializer_call_tree_is_bounded(
            program,
            call.receiver,
            remaining_call_depth - 1,
        ))
        && program
            .expression_table
            .expression_handles(call.arguments)
            .iter()
            .all(|argument| {
                isolated_local_initializer_call_tree_is_bounded(
                    program,
                    *argument,
                    remaining_call_depth - 1,
                )
            })
}

/// One direct Unit statement call is neutral to a returned-place relation when
/// its inferred frame is complete and no argument exposes a mutable-reference
/// binding for rebinding. The same applies to an explicitly discarded concrete
/// primitive result from a nongeneric checked-body call: the value cannot carry
/// an alias, while the call's complete frame still publishes its writes. Writes
/// through references passed by value may change their contents, but cannot
/// redirect the established origin. One direct value-call argument is admitted
/// under the same complete-frame rule. Other discarded results, deeper computed
/// arguments, binding reborrows, and opaque frames remain fences.
fn statement_call_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCall,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    if call.discards_result
        && !discarded_primitive_internal_call_is_relationally_neutral(program, call, symbols)
    {
        return false;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return false;
    }
    let arguments = program.statement_table.expression_handles(call.arguments);
    // Sibling arguments are independent. Each receives the same deliberately
    // bounded call-depth budget; exhausting it fails closed.
    if arguments.iter().any(|argument| {
        !statement_call_argument_preserves_transparent_result(
            program,
            current_machine,
            *argument,
            &machine_symbols,
            symbols,
            active_states,
            parameters,
            aliases,
            2,
        )
    }) {
        return false;
    }

    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    let argument_origins = arguments
        .iter()
        .map(|argument| {
            parameter_relative_place_origin(
                program,
                current_machine,
                *argument,
                parameters,
                aliases,
                symbols,
                active_states,
            )
            .map(|origin| origin.place)
        })
        .collect::<Vec<_>>();
    known_call_written_paths_for_parts_with_origins(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        arguments,
        current_machine,
        &machine_symbols,
        symbols,
        active_states,
        Some(&argument_origins),
    )
    .or_else(|| {
        (!arguments
            .iter()
            .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
        .then(|| {
            known_boundary_call_written_paths_for_parts(
                program,
                &machine_symbols,
                symbols,
                &receiver_members,
                call.target.as_str(),
                arguments,
            )
        })
        .flatten()
    })
    .is_some()
}

/// An explicitly discarded result cannot redirect a returned-place relation
/// only when the resolved internal callee is an ordinary nongeneric body and
/// its declared result is a concrete primitive. The ordinary complete-frame
/// check below remains responsible for proving every side write. Boundary,
/// generic, reference-bearing, aggregate, and unresolved calls fail closed.
fn discarded_primitive_internal_call_is_relationally_neutral(
    program: &TypedTrees,
    call: &TableCall,
    symbols: &TopLevelSymbols<'_>,
) -> bool {
    let Some((callee_machine, callee_state)) = machine_state_by_symbol(program, call.target_symbol)
        .or_else(|| {
            call.receiver
                .is_empty()
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    call.receiver.is_empty() != callee_machine.attached_data.is_some()
        && callee_machine.supply_mode == psi_language_semantics::MachineSupplyMode::CheckedBody
        && callee_machine.lifetime_parameters.is_empty()
        && program.machine_type_parameters(callee_machine).is_empty()
        && call.machine_arguments.is_empty()
        && program
            .primitive_type_reference(callee_state.return_type)
            .is_some()
}

/// A complete bounded call tree may supply an assignment value without
/// perturbing a separately returned place only when its root result is proven
/// non-reference. A direct primitive scalar value may wrap complete
/// caller-isolated call producers in up to two unary, binary, primitive-cast,
/// member-projection, or indexing shells. One
/// primitive-only record, selected-case, or fixed-array literal may
/// independently contain such a tree in each direct field/element, and one
/// nested aggregate of either concrete kind may do the same. Reference-bearing or
/// generic literals, wider aggregate or scalar-computation depth, and unknown
/// return types fail closed.
const TRANSPARENT_ASSIGNMENT_VALUE_CALL_DEPTH: usize = 4;
const TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_DEPTH: usize = 2;
const TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH: usize = 2;

fn value_expression_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    assignment_target_type: Option<TypeReferenceHandle>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
            program,
            current_machine,
            expression,
            symbols,
            active_states,
            parameters,
            aliases,
        ),
        ExpressionNode::StructLiteral(_) => {
            aggregate_value_assignment_preserves_transparent_result(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_DEPTH,
            )
        }
        ExpressionNode::ArrayLiteral(_) => assignment_target_type.is_some_and(|target_type| {
            array_value_assignment_preserves_transparent_result(
                program,
                current_machine,
                expression,
                target_type,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_DEPTH,
                TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
            )
        }),
        ExpressionNode::Binary(_)
        | ExpressionNode::Indexed(_)
        | ExpressionNode::Member(_)
        | ExpressionNode::Unary(_)
            if assignment_target_type.is_some_and(|target_type| {
                program.primitive_type_reference(target_type).is_some()
            }) =>
        {
            primitive_computed_assignment_value_preserves_transparent_result(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
            )
        }
        ExpressionNode::Cast(cast)
            if assignment_target_type.is_some_and(|target_type| {
                program.primitive_type_reference(target_type).is_some()
            }) && program.primitive_type_reference(cast.target_type).is_some() =>
        {
            primitive_computed_assignment_value_preserves_transparent_result(
                program,
                current_machine,
                expression,
                symbols,
                active_states,
                parameters,
                aliases,
                TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
            )
        }
        _ => false,
    }
}

fn assignment_target_type(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    target: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    crate::places::declared_place_type(program, machine, Some(state), target).or_else(|| {
        crate::places::declared_indexed_projection_type(program, machine, Some(state), target)
    })
}

/// Apply the concrete aggregate-value rail to one fixed-array literal. The
/// assignment target supplies the exact contextual element type that an array
/// literal does not carry itself. Only literal-length, caller-isolated arrays
/// participate; every effectful element independently obeys the ordinary
/// depth-four call budget, primitive elements may use the carried scalar
/// computation budget, and one nested fixed-array or concrete aggregate
/// literal consumes the second aggregate level.
#[allow(clippy::too_many_arguments)]
fn array_value_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    expected_type: TypeReferenceHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
    remaining_computed_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 || !type_is_caller_isolated_local(program, expected_type) {
        return false;
    }
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
    else {
        return false;
    };
    let Some(expected_type) = crate::places::unwrapped_type_reference(program, expected_type)
    else {
        return false;
    };
    let TypeReferenceNode::FixedArray {
        element_type,
        length: psi_typed_trees::types::FixedArrayLength::Literal(length),
    } = program.type_reference_table.type_reference(expected_type)
    else {
        return false;
    };
    let elements = program.expression_table.expression_handles(*elements);
    if *length != elements.len() {
        return false;
    }
    let Some(element_type) = crate::places::unwrapped_type_reference(program, *element_type) else {
        return false;
    };
    elements.iter().all(|element| {
        if !expression_is_effectful_for_transparent_result(program, *element) {
            return true;
        }
        match program.expression_table.expression(*element) {
            ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                program,
                current_machine,
                *element,
                symbols,
                active_states,
                parameters,
                aliases,
            ),
            ExpressionNode::ArrayLiteral(_) => array_value_assignment_preserves_transparent_result(
                program,
                current_machine,
                *element,
                element_type,
                symbols,
                active_states,
                parameters,
                aliases,
                remaining_aggregate_depth - 1,
                remaining_computed_depth,
            ),
            ExpressionNode::StructLiteral(literal)
                if struct_literal_matches_expected_type(program, literal, element_type) =>
            {
                aggregate_value_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    *element,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_aggregate_depth - 1,
                )
            }
            ExpressionNode::Binary(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Unary(_)
                if program.primitive_type_reference(element_type).is_some() =>
            {
                primitive_computed_value_preserves_transparent_result(
                    program,
                    current_machine,
                    *element,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_computed_depth,
                    false,
                )
            }
            _ => false,
        }
    })
}

/// Apply the settled primitive aggregate-field computation algebra directly to
/// a scalar assignment value. The primitive assignment target supplies the
/// typed result fact; every effectful call producer must return a caller-
/// isolated value, so generic/reference carriers remain fenced while concrete
/// records and fixed arrays returned by calls can feed member/index shells. A
/// direct member projection may additionally select from one concrete literal
/// whose effectful fields are bounded direct-call trees, and may itself sit
/// below one further computation shell. The literal member consumes one of the
/// established two shells rather than resetting that budget. A third
/// computation shell fails closed.
#[allow(clippy::too_many_arguments)]
fn primitive_computed_assignment_value_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_computed_depth: usize,
) -> bool {
    primitive_computed_value_preserves_transparent_result(
        program,
        current_machine,
        expression,
        symbols,
        active_states,
        parameters,
        aliases,
        remaining_computed_depth,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn caller_isolated_value_call_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some((_, callee_state)) =
        machine_state_by_symbol(program, call.target_symbol).or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    type_is_caller_isolated_local(program, callee_state.return_type)
        && value_call_assignment_preserves_transparent_result(
            program,
            current_machine,
            expression,
            symbols,
            active_states,
            parameters,
            aliases,
        )
}

#[allow(clippy::too_many_arguments)]
fn aggregate_value_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 {
        return false;
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return false;
    };
    if !struct_literal_type_is_caller_isolated(program, literal) {
        return false;
    }
    program
        .expression_table
        .struct_fields(literal.fields)
        .iter()
        .all(|field| {
            if !expression_is_effectful_for_transparent_result(program, field.value) {
                return true;
            }
            match program.expression_table.expression(field.value) {
                ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    field.value,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                ),
                ExpressionNode::StructLiteral(_) => {
                    aggregate_value_assignment_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        remaining_aggregate_depth - 1,
                    )
                }
                ExpressionNode::ArrayLiteral(_) => {
                    struct_literal_field_type(program, literal, field.name.as_str()).is_some_and(
                        |field_type| {
                            array_value_assignment_preserves_transparent_result(
                                program,
                                current_machine,
                                field.value,
                                field_type,
                                symbols,
                                active_states,
                                parameters,
                                aliases,
                                remaining_aggregate_depth - 1,
                                TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
                            )
                        },
                    )
                }
                ExpressionNode::Binary(_)
                    if struct_literal_field_is_primitive(program, literal, field.name.as_str()) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
                        false,
                    )
                }
                ExpressionNode::Cast(cast)
                    if program.primitive_type_reference(cast.target_type).is_some()
                        && struct_literal_field_is_primitive(
                            program,
                            literal,
                            field.name.as_str(),
                        ) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
                        false,
                    )
                }
                ExpressionNode::Unary(_)
                    if struct_literal_field_is_primitive(program, literal, field.name.as_str()) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
                        false,
                    )
                }
                ExpressionNode::Member(_) | ExpressionNode::Indexed(_)
                    if struct_literal_field_is_primitive(program, literal, field.name.as_str()) =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        TRANSPARENT_ASSIGNMENT_VALUE_COMPUTED_DEPTH,
                        false,
                    )
                }
                _ => false,
            }
        })
}

#[allow(clippy::too_many_arguments)]
fn primitive_computed_value_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_computed_depth: usize,
    require_caller_isolated_call_result: bool,
) -> bool {
    if remaining_computed_depth == 0 {
        return false;
    }
    let direct_concrete_literal_member = require_caller_isolated_call_result
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(_)
        );
    let direct_array_literal_index = require_caller_isolated_call_result
        && matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Indexed(_)
        );
    let operands = match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => [Some(binary.left), Some(binary.right)],
        ExpressionNode::Cast(cast)
            if program.primitive_type_reference(cast.target_type).is_some() =>
        {
            [Some(cast.value), None]
        }
        ExpressionNode::Indexed(indexed) => [Some(indexed.collection), Some(indexed.index)],
        ExpressionNode::Member(member) => [Some(member.receiver), None],
        ExpressionNode::Unary(unary) => [Some(unary.operand), None],
        _ => return false,
    };
    operands.into_iter().flatten().all(|operand| {
        if !expression_is_effectful_for_transparent_result(program, operand) {
            return true;
        }
        match program.expression_table.expression(operand) {
            ExpressionNode::Call(_) if require_caller_isolated_call_result => {
                caller_isolated_value_call_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                )
            }
            ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                program,
                current_machine,
                operand,
                symbols,
                active_states,
                parameters,
                aliases,
            ),
            ExpressionNode::StructLiteral(_) if direct_concrete_literal_member => {
                concrete_literal_member_operand_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    TRANSPARENT_ASSIGNMENT_VALUE_AGGREGATE_DEPTH,
                    remaining_computed_depth - 1,
                )
            }
            ExpressionNode::ArrayLiteral(_) if direct_array_literal_index => {
                concrete_array_literal_index_operand_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_computed_depth - 1,
                )
            }
            ExpressionNode::Binary(_)
            | ExpressionNode::Cast(_)
            | ExpressionNode::Indexed(_)
            | ExpressionNode::Member(_)
            | ExpressionNode::Unary(_)
                if remaining_computed_depth > 1 =>
            {
                primitive_computed_value_preserves_transparent_result(
                    program,
                    current_machine,
                    operand,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                    remaining_computed_depth - 1,
                    require_caller_isolated_call_result,
                )
            }
            _ => false,
        }
    })
}

/// Admit one fixed-array literal directly below a primitive index projection.
/// Typing has already established one primitive element type from the indexed
/// result. Every eagerly evaluated element publishes its complete call frame;
/// primitive computation shells share the same depth budget consumed by the
/// index projection. Nested aggregate literals remain outside this cohort
/// because this expression site carries no independent contextual aggregate
/// type for validating them.
#[allow(clippy::too_many_arguments)]
fn concrete_array_literal_index_operand_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_computed_depth: usize,
) -> bool {
    let ExpressionNode::ArrayLiteral(elements) = program.expression_table.expression(expression)
    else {
        return false;
    };
    program
        .expression_table
        .expression_handles(*elements)
        .iter()
        .all(|element| {
            if !expression_is_effectful_for_transparent_result(program, *element) {
                return true;
            }
            match program.expression_table.expression(*element) {
                ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                    program,
                    current_machine,
                    *element,
                    symbols,
                    active_states,
                    parameters,
                    aliases,
                ),
                ExpressionNode::Binary(_)
                | ExpressionNode::Cast(_)
                | ExpressionNode::Indexed(_)
                | ExpressionNode::Member(_)
                | ExpressionNode::Unary(_)
                    if remaining_computed_depth > 0 =>
                {
                    primitive_computed_value_preserves_transparent_result(
                        program,
                        current_machine,
                        *element,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        remaining_computed_depth,
                        false,
                    )
                }
                _ => false,
            }
        })
}

/// Admit one named concrete literal directly below a primitive member
/// projection, including when that projection sits below one outer computation
/// shell. The literal's type makes the aggregate shape explicit, unlike a
/// projected array literal whose contextual fixed-array type is unavailable
/// after the scalar projection. Effectful primitive fields may use only the
/// computation budget left after the member projection itself. Nested
/// aggregates consume the same explicit aggregate-depth budget as ordinary
/// aggregate assignments while retaining that reduced computation budget.
/// Any shell or nested literal that would reset or exceed either budget stays
/// outside this narrow cohort.
#[allow(clippy::too_many_arguments)]
fn concrete_literal_member_operand_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_aggregate_depth: usize,
    remaining_computed_depth: usize,
) -> bool {
    if remaining_aggregate_depth == 0 {
        return false;
    }
    let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(expression)
    else {
        return false;
    };
    struct_literal_type_is_caller_isolated(program, literal)
        && program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .all(|field| {
                if !expression_is_effectful_for_transparent_result(program, field.value) {
                    return true;
                }
                match program.expression_table.expression(field.value) {
                    ExpressionNode::Call(_) => value_call_assignment_preserves_transparent_result(
                        program,
                        current_machine,
                        field.value,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                    ),
                    ExpressionNode::StructLiteral(_) if remaining_aggregate_depth > 1 => {
                        concrete_literal_member_operand_preserves_transparent_result(
                            program,
                            current_machine,
                            field.value,
                            symbols,
                            active_states,
                            parameters,
                            aliases,
                            remaining_aggregate_depth - 1,
                            remaining_computed_depth,
                        )
                    }
                    ExpressionNode::ArrayLiteral(_) if remaining_aggregate_depth > 1 => {
                        struct_literal_field_type(program, literal, field.name.as_str())
                            .is_some_and(|field_type| {
                                array_value_assignment_preserves_transparent_result(
                                    program,
                                    current_machine,
                                    field.value,
                                    field_type,
                                    symbols,
                                    active_states,
                                    parameters,
                                    aliases,
                                    remaining_aggregate_depth - 1,
                                    remaining_computed_depth,
                                )
                            })
                    }
                    ExpressionNode::Binary(_)
                    | ExpressionNode::Cast(_)
                    | ExpressionNode::Indexed(_)
                    | ExpressionNode::Member(_)
                    | ExpressionNode::Unary(_)
                        if remaining_computed_depth > 0
                            && struct_literal_field_is_primitive(
                                program,
                                literal,
                                field.name.as_str(),
                            ) =>
                    {
                        primitive_computed_value_preserves_transparent_result(
                            program,
                            current_machine,
                            field.value,
                            symbols,
                            active_states,
                            parameters,
                            aliases,
                            remaining_computed_depth,
                            false,
                        )
                    }
                    _ => false,
                }
            })
}

fn value_call_assignment_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    let Some((_, callee_state)) =
        machine_state_by_symbol(program, call.target_symbol).or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })
    else {
        return false;
    };
    if !callee_state.return_type.is_valid()
        || type_reference_is_reference(program, callee_state.return_type)
    {
        return false;
    }
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, current_machine, &mut diagnostics);
    diagnostics.is_empty()
        && statement_call_argument_preserves_transparent_result(
            program,
            current_machine,
            expression,
            &machine_symbols,
            symbols,
            active_states,
            parameters,
            aliases,
            TRANSPARENT_ASSIGNMENT_VALUE_CALL_DEPTH,
        )
}

#[allow(clippy::too_many_arguments)]
/// Admit a bounded exact-call tree in one statement-call argument. Applying
/// this independently to every sibling permits bounded width; the explicit
/// budget prevents unbounded expression depth from becoming relational proof.
fn statement_call_argument_preserves_transparent_result(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    remaining_call_depth: usize,
) -> bool {
    if expression_reborrows_transparent_alias_binding(program, expression, parameters, aliases) {
        return false;
    }
    if !expression_is_effectful_for_transparent_result(program, expression) {
        return true;
    }
    if remaining_call_depth == 2 && expression_is_effectful_indexed_place(program, expression) {
        return parameter_relative_place_origin(
            program,
            current_machine,
            expression,
            parameters,
            aliases,
            symbols,
            active_states,
        )
        .is_some();
    }
    if remaining_call_depth == 0 {
        return false;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    if call.receiver.is_valid()
        && expression_is_effectful_for_transparent_result(program, call.receiver)
    {
        return false;
    }

    let receiver_members = if call.receiver.is_valid() {
        let Some(receiver) = receiver_member_chain(program, call.receiver) else {
            return false;
        };
        receiver
    } else {
        Vec::new()
    };
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.iter().any(|argument| {
        !statement_call_argument_preserves_transparent_result(
            program,
            current_machine,
            *argument,
            machine_symbols,
            symbols,
            active_states,
            parameters,
            aliases,
            remaining_call_depth - 1,
        )
    }) {
        return false;
    }
    known_call_written_paths_for_parts(
        program,
        call.target_symbol,
        call.target.as_str(),
        &receiver_members,
        arguments,
        current_machine,
        machine_symbols,
        symbols,
        active_states,
    )
    .or_else(|| {
        known_boundary_call_written_paths_for_parts(
            program,
            machine_symbols,
            symbols,
            &receiver_members,
            call.target.as_str(),
            arguments,
        )
    })
    .is_some()
}

fn expression_is_effectful_indexed_place(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_is_effectful_indexed_place(program, *inner),
        ExpressionNode::Member(member) => {
            expression_is_effectful_indexed_place(program, member.receiver)
        }
        ExpressionNode::Indexed(indexed)
            if expression_is_effectful_for_transparent_result(program, indexed.index) =>
        {
            true
        }
        _ => false,
    }
}

fn expression_reborrows_transparent_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let visit =
        |child| expression_reborrows_transparent_alias_binding(program, child, parameters, aliases);
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            let reborrows_binding = matches!(
                program.expression_table.expression(*inner),
                ExpressionNode::Name(_)
            ) && frame_place_path(program, *inner).is_some_and(|place| {
                let (root, suffix) = split_place_root(&place.path);
                if !suffix.is_empty() {
                    return false;
                }
                let root_symbol = frame_place_root_symbol(program, *inner);
                parameters.iter().any(|parameter| {
                    matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        TypeReferenceNode::Reference {
                            is_mutable: true,
                            ..
                        }
                    ) && (root_symbol == Some(parameter.symbol)
                        || parameter.is_self && root == "self"
                        || root == parameter.name.as_str())
                }) || aliases.iter().any(|(name, symbol, _)| {
                    root_symbol
                        .is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol)
                        || root == name
                })
            });
            reborrows_binding || visit(*inner)
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value) || visit(atomic.result),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && visit(call.receiver))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| visit(*argument))
        }
        ExpressionNode::Binary(binary) => visit(binary.left) || visit(binary.right),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => visit(indexed.collection) || visit(indexed.index),
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| visit(*element)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| visit(field.value)),
        ExpressionNode::Range(range) => visit(range.start) || visit(range.end),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn parameter_relative_alias_position(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
) -> Option<usize> {
    let place = frame_place_path(program, expression)?;
    let (root, suffix) = split_place_root(&place.path);
    if !suffix.is_empty() {
        return None;
    }
    let root_symbol = frame_place_root_symbol(program, expression);
    aliases.iter().position(|(name, symbol, _)| {
        let exact_symbol =
            root_symbol.is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
        let unresolved_name = root_symbol.is_none_or(|root| !root.is_valid()) && name == root;
        exact_symbol || unresolved_name
    })
}

#[derive(Debug, Clone)]
struct ParameterRelativeFrameOrigin {
    place: FramePlaceOrigin,
    parameter_symbol: SymbolHandle,
}

/// Effects are permitted only along the place-producing call spine or inside a
/// separately validated index expression. `parameter_relative_place_origin`
/// owns the bounded-call and non-rebinding proof for the latter.
fn transparent_assignment_target_effect_is_structural(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            transparent_assignment_target_effect_is_structural(program, *inner)
        }
        ExpressionNode::Indexed(_) => true,
        ExpressionNode::Member(member) => {
            transparent_assignment_target_effect_is_structural(program, member.receiver)
        }
        ExpressionNode::Call(_) => true,
        _ => false,
    }
}

fn parameter_relative_place_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
    parameters: &[StateParameter],
    aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<ParameterRelativeFrameOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => parameter_relative_place_origin(
            program,
            current_machine,
            *inner,
            parameters,
            aliases,
            symbols,
            active_states,
        ),
        ExpressionNode::Indexed(indexed) => {
            if expression_is_effectful_for_transparent_result(program, indexed.index) {
                let mut diagnostics = Vec::new();
                let machine_symbols =
                    MachineSymbols::build(program, current_machine, &mut diagnostics);
                if !diagnostics.is_empty()
                    || !statement_call_argument_preserves_transparent_result(
                        program,
                        current_machine,
                        indexed.index,
                        &machine_symbols,
                        symbols,
                        active_states,
                        parameters,
                        aliases,
                        2,
                    )
                {
                    return None;
                }
            }
            let mut origin = parameter_relative_place_origin(
                program,
                current_machine,
                indexed.collection,
                parameters,
                aliases,
                symbols,
                active_states,
            )?;
            origin.place.precision = FramePathPrecision::CollectionCoarse;
            Some(origin)
        }
        ExpressionNode::Member(member) => {
            let mut origin = parameter_relative_place_origin(
                program,
                current_machine,
                member.receiver,
                parameters,
                aliases,
                symbols,
                active_states,
            )?;
            if origin.place.precision == FramePathPrecision::Exact {
                origin.place.path = format!("{}.{}", origin.place.path, member.member.as_str());
            }
            Some(origin)
        }
        ExpressionNode::Name(_) => {
            let place = frame_place_path(program, expression)?;
            let root_symbol = frame_place_root_symbol(program, expression);
            let (root, suffix) = split_place_root(&place.path);
            if let Some(parameter) = parameters.iter().find(|parameter| {
                (root_symbol == Some(parameter.symbol) || (parameter.is_self && root == "self"))
                    && matches!(
                        program
                            .type_reference_table
                            .type_reference(parameter.type_reference),
                        TypeReferenceNode::Reference {
                            is_mutable: true,
                            ..
                        }
                    )
            }) {
                return Some(ParameterRelativeFrameOrigin {
                    place,
                    parameter_symbol: parameter.symbol,
                });
            }
            let parent = aliases.iter().find_map(|(name, symbol, origin)| {
                let exact_symbol = root_symbol
                    .is_some_and(|root| root.is_valid() && symbol.is_valid() && root == *symbol);
                let unresolved_name =
                    root_symbol.is_none_or(|root| !root.is_valid()) && name == root;
                (exact_symbol || unresolved_name).then_some(origin)
            })?;
            Some(ParameterRelativeFrameOrigin {
                place: match parent.place.precision {
                    FramePathPrecision::Exact => FramePlaceOrigin {
                        path: append_place_suffix(&parent.place.path, suffix),
                        precision: place.precision,
                    },
                    FramePathPrecision::CollectionCoarse => parent.place.clone(),
                },
                parameter_symbol: parent.parameter_symbol,
            })
        }
        ExpressionNode::Call(call) => {
            if call_is_transparent_mutable_slice_view(program, call) {
                return parameter_relative_place_origin(
                    program,
                    current_machine,
                    call.receiver,
                    parameters,
                    aliases,
                    symbols,
                    active_states,
                );
            }
            parameter_relative_call_result_origin(
                program,
                current_machine,
                call,
                parameters,
                aliases,
                symbols,
                active_states,
            )
        }
        ExpressionNode::Cast(cast)
            if cast.form.is_recast()
                && !expression_is_effectful_for_transparent_result(program, cast.value) =>
        {
            parameter_relative_place_origin(
                program,
                current_machine,
                cast.value,
                parameters,
                aliases,
                symbols,
                active_states,
            )
        }
        _ => None,
    }
}

fn parameter_relative_call_result_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    call: &TableCallExpression,
    caller_parameters: &[StateParameter],
    caller_aliases: &[(String, SymbolHandle, ParameterRelativeFrameOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<ParameterRelativeFrameOrigin> {
    let (callee_machine, callee_state) = machine_state_by_symbol(program, call.target_symbol)
        .or_else(|| {
            (!call.receiver.is_valid())
                .then(|| free_machine_entry_state(program, symbols, call.target.as_str()))
                .flatten()
        })?;
    if call.receiver.is_valid() != callee_machine.attached_data.is_some() {
        return None;
    }
    let callee_origin = transparent_callee_result_origin(
        program,
        callee_machine,
        callee_state,
        symbols,
        active_states,
    )?;
    let callee_parameters = program.state_parameters(callee_state);
    let callee_parameter = callee_parameters
        .iter()
        .find(|parameter| parameter.symbol == callee_origin.parameter_symbol)?;
    let actual = if callee_parameter.is_self {
        if callee_machine.attached_data.is_none() || !call.receiver.is_valid() {
            return None;
        }
        call.receiver
    } else {
        let (argument_index, _) = callee_parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == callee_parameter.symbol)?;
        *program
            .expression_table
            .expression_handles(call.arguments)
            .get(argument_index)?
    };
    let actual_origin = parameter_relative_place_origin(
        program,
        current_machine,
        actual,
        caller_parameters,
        caller_aliases,
        symbols,
        active_states,
    )?;
    let (_, suffix) = split_place_root(&callee_origin.place.path);
    Some(match actual_origin.place.precision {
        FramePathPrecision::Exact => ParameterRelativeFrameOrigin {
            place: FramePlaceOrigin {
                path: append_place_suffix(&actual_origin.place.path, suffix),
                precision: callee_origin.place.precision,
            },
            parameter_symbol: actual_origin.parameter_symbol,
        },
        FramePathPrecision::CollectionCoarse => actual_origin,
    })
}

fn expression_is_effectful_for_transparent_result(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Atomic(_) => true,
        ExpressionNode::Call(call) => {
            !call_is_effect_free_slice_view(program, call)
                || expression_is_effectful_for_transparent_result(program, call.receiver)
        }
        ExpressionNode::Binary(binary) => {
            expression_is_effectful_for_transparent_result(program, binary.left)
                || expression_is_effectful_for_transparent_result(program, binary.right)
        }
        ExpressionNode::Cast(cast) => {
            expression_is_effectful_for_transparent_result(program, cast.value)
        }
        ExpressionNode::Indexed(indexed) => {
            expression_is_effectful_for_transparent_result(program, indexed.collection)
                || expression_is_effectful_for_transparent_result(program, indexed.index)
        }
        ExpressionNode::Member(member) => {
            expression_is_effectful_for_transparent_result(program, member.receiver)
        }
        ExpressionNode::Mutable(inner) => {
            expression_is_effectful_for_transparent_result(program, *inner)
        }
        ExpressionNode::Unary(unary) => {
            expression_is_effectful_for_transparent_result(program, unary.operand)
        }
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| expression_is_effectful_for_transparent_result(program, *element)),
        ExpressionNode::Range(range) => {
            expression_is_effectful_for_transparent_result(program, range.start)
                || expression_is_effectful_for_transparent_result(program, range.end)
        }
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| expression_is_effectful_for_transparent_result(program, field.value)),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn call_is_transparent_mutable_slice_view(
    program: &TypedTrees,
    call: &TableCallExpression,
) -> bool {
    call.target.as_str() == "as_mut_slice" && call_is_effect_free_slice_view(program, call)
}

fn call_is_effect_free_slice_view(program: &TypedTrees, call: &TableCallExpression) -> bool {
    matches!(call.target.as_str(), "as_slice" | "as_mut_slice")
        && call.receiver.is_valid()
        && program
            .expression_table
            .expression_handles(call.arguments)
            .is_empty()
}

fn frame_place_root_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => frame_place_root_symbol(program, *inner),
        ExpressionNode::Indexed(indexed) => frame_place_root_symbol(program, indexed.collection),
        ExpressionNode::Member(member) => frame_place_root_symbol(program, member.receiver),
        ExpressionNode::Name(path) => path
            .head_symbol
            .is_valid()
            .then_some(path.head_symbol)
            .or_else(|| path.symbol.is_valid().then_some(path.symbol)),
        _ => None,
    }
}

fn rebase_local_alias_path(relative: &str, aliases: &[(String, FramePlaceOrigin)]) -> String {
    let (root, suffix) = split_place_root(relative);
    aliases
        .iter()
        .find_map(|(alias, origin)| {
            (alias == root).then(|| match origin.precision {
                FramePathPrecision::Exact => append_place_suffix(&origin.path, suffix),
                FramePathPrecision::CollectionCoarse => origin.path.clone(),
            })
        })
        .unwrap_or_else(|| relative.to_owned())
}

fn expression_reborrows_local_alias_binding(
    program: &TypedTrees,
    expression: ExpressionHandle,
    aliases: &[(String, FramePlaceOrigin)],
) -> bool {
    if !expression.is_valid() {
        return false;
    }
    let visit = |child| expression_reborrows_local_alias_binding(program, child, aliases);
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            let borrows_binding = matches!(
                program.expression_table.expression(*inner),
                ExpressionNode::Name(_)
            ) && arithmetic_domains::place_path(program, *inner)
                .is_some_and(|path| aliases.iter().any(|(alias, _)| path == *alias));
            borrows_binding || visit(*inner)
        }
        ExpressionNode::Atomic(atomic) => visit(atomic.value) || visit(atomic.result),
        ExpressionNode::Call(call) => {
            (call.receiver.is_valid() && visit(call.receiver))
                || program
                    .expression_table
                    .expression_handles(call.arguments)
                    .iter()
                    .any(|argument| visit(*argument))
        }
        ExpressionNode::Binary(binary) => visit(binary.left) || visit(binary.right),
        ExpressionNode::Unary(unary) => visit(unary.operand),
        ExpressionNode::Cast(cast) => visit(cast.value),
        ExpressionNode::Indexed(indexed) => visit(indexed.collection) || visit(indexed.index),
        ExpressionNode::Member(member) => visit(member.receiver),
        ExpressionNode::ArrayLiteral(elements) => program
            .expression_table
            .expression_handles(*elements)
            .iter()
            .any(|element| visit(*element)),
        ExpressionNode::StructLiteral(literal) => program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .any(|field| visit(field.value)),
        ExpressionNode::Range(range) => visit(range.start) || visit(range.end),
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

/// Update one local mutable-reference binding when its replacement is another
/// directly representable place. Existing aliases retain their already-
/// canonicalized origins, so rebinding an upstream local never redirects a
/// previously established reborrow. Structurally transparent call results
/// compose through the same origin algebra; other computed replacements remain
/// opaque.
#[allow(clippy::too_many_arguments)]
fn rebind_stable_local_mutable_alias_origin(
    program: &TypedTrees,
    machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    state: &State,
    target: &str,
    value: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &mut [(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> Option<bool> {
    let Some(position) = aliases.iter().position(|(alias, _)| alias == target) else {
        return Some(false);
    };
    if !expression_may_rebind_mutable_alias(program, machine, state, value) {
        return Some(false);
    }
    let origin = stable_alias_initializer_origin(
        program,
        machine,
        machine_symbols,
        active_states,
        value,
        parameters,
        isolated_local_roots,
        aliases,
        symbols,
        true,
    )?;
    aliases[position].1 = origin;
    Some(true)
}

#[allow(clippy::too_many_arguments)]
fn stable_local_mutable_alias_rebinding_is_representable(
    program: &TypedTrees,
    machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    state: &State,
    target: &str,
    value: ExpressionHandle,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
) -> bool {
    aliases.iter().any(|(alias, _)| alias == target)
        && expression_may_rebind_mutable_alias(program, machine, state, value)
        && stable_alias_initializer_origin(
            program,
            machine,
            machine_symbols,
            active_states,
            value,
            parameters,
            isolated_local_roots,
            aliases,
            symbols,
            true,
        )
        .is_some()
}

/// A bare write through `alias` (`alias = 1`) targets the borrowed place, but
/// Psi also permits a mutable-reference local declared with plain `let` to be
/// rebound (`alias = &mut other`). Accept an exact origin only while the RHS is
/// proven value-shaped; unknown/reference-shaped replacements fail closed.
fn expression_may_rebind_mutable_alias(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(_) | ExpressionNode::Call(_) => true,
        ExpressionNode::Name(_) | ExpressionNode::Member(_) | ExpressionNode::Indexed(_) => {
            let declared =
                crate::places::declared_place_type_raw(program, machine, Some(state), expression)
                    .or_else(|| {
                        crate::places::declared_indexed_projection_type_raw(
                            program,
                            machine,
                            Some(state),
                            expression,
                        )
                    });
            declared.is_none_or(|handle| type_reference_is_reference(program, handle))
        }
        ExpressionNode::Cast(cast) => type_reference_is_reference(program, cast.target_type),
        ExpressionNode::ArrayLiteral(_)
        | ExpressionNode::Atomic(_)
        | ExpressionNode::Binary(_)
        | ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Range(_)
        | ExpressionNode::StructLiteral(_)
        | ExpressionNode::String(_)
        | ExpressionNode::Unary(_)
        | ExpressionNode::ZeroValue(_) => false,
    }
}

fn type_reference_is_reference(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference { .. } => true,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_reference_is_reference(program, *base_type)
        }
        _ => false,
    }
}

fn named_transition_subgraph_is_acyclic(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: psi_typed_trees::statement::TransitionTargetHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        visiting: &mut Vec<SymbolHandle>,
        complete: &mut Vec<SymbolHandle>,
    ) -> bool {
        if complete.contains(&state.symbol) {
            return true;
        }
        if visiting.contains(&state.symbol) {
            return false;
        }
        visiting.push(state.symbol);
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for edge in [transition.target, transition.continuation] {
                if !edge.is_valid()
                    || !matches!(
                        program.statement_table.transition_target(edge),
                        TransitionTargetNode::Named { .. }
                    )
                {
                    continue;
                }
                let Some(next) = named_transition_target_state(program, machine, state, edge)
                else {
                    return false;
                };
                if !visit(program, machine, next, visiting, complete) {
                    return false;
                }
            }
        }
        visiting.pop();
        complete.push(state.symbol);
        true
    }

    let Some(target) = named_transition_target_state(program, machine, source, target) else {
        return false;
    };
    visit(program, machine, target, &mut Vec::new(), &mut Vec::new())
}

fn named_transition_target_state<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    source: &'program State,
    target: psi_typed_trees::statement::TransitionTargetHandle,
) -> Option<&'program State> {
    let TransitionTargetNode::Named { path, .. } =
        program.statement_table.transition_target(target)
    else {
        return None;
    };
    program
        .machine_states(machine)
        .iter()
        .find(|candidate| candidate.symbol == path.symbol)
        .or_else(|| {
            let members = program.statement_table.name_path_members(path.members);
            matches!(members, [member] if member.as_str() == "self").then_some(source)
        })
}

#[derive(Debug, Clone)]
struct PermutedCycleFrameEdge {
    target: SymbolHandle,
    arguments: Vec<ExpressionHandle>,
}

#[derive(Debug)]
struct PermutedCycleFrameEquation<'program> {
    state: &'program State,
    locals: Vec<String>,
    local_alias_origins: Vec<(String, FramePlaceOrigin)>,
    direct_writes: Vec<String>,
    edges: Vec<PermutedCycleFrameEdge>,
}

/// Recover an exact finite frame for transition SCCs whose write-capable state
/// parameters are only permuted around each cycle.
///
/// The ordinary recursive summarizer above deliberately fails closed when it
/// reaches a named cycle that redirects an exclusive parameter. A permutation
/// is nevertheless finite: repeated traversal can only move an already-known
/// path among the SCC's positional roots. This fallback solves the reachable
/// state equations to a fixed point after proving that every cyclic edge is an
/// exact bijection over those write-capable roots. Structurally transparent
/// returned places preserve the root they forward. Projections, opaque helper
/// results, duplication, omission, and computed rebinding stay opaque;
/// otherwise suffix growth could make the path set unbounded or alias two
/// semantic roots.
fn summarize_state_written_paths_with_permuted_cycles<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    entry: &'program State,
    symbols: &TopLevelSymbols<'program>,
    outer_active_states: &[SymbolHandle],
) -> Option<Vec<String>> {
    let mut diagnostics = Vec::new();
    let machine_symbols = MachineSymbols::build(program, machine, &mut diagnostics);
    if !diagnostics.is_empty() {
        return None;
    }

    let mut equations = Vec::<PermutedCycleFrameEquation<'program>>::new();
    let mut pending = vec![entry.symbol];
    while let Some(symbol) = pending.pop() {
        if equations
            .iter()
            .any(|equation| equation.state.symbol == symbol)
        {
            continue;
        }
        let state = program
            .machine_states(machine)
            .iter()
            .find(|candidate| candidate.symbol == symbol)?;
        let equation = build_permuted_cycle_frame_equation(
            program,
            machine,
            state,
            symbols,
            &machine_symbols,
            outer_active_states,
        )?;
        pending.extend(equation.edges.iter().map(|edge| edge.target));
        equations.push(equation);
    }

    let mut has_transition_cycle = false;
    for equation in &equations {
        for edge in &equation.edges {
            if transition_state_reaches(&equations, edge.target, equation.state.symbol) {
                has_transition_cycle = true;
                let target = equations
                    .iter()
                    .find(|candidate| candidate.state.symbol == edge.target)?
                    .state;
                let mut active_states = outer_active_states.to_vec();
                if !transition_is_exact_write_parameter_permutation(
                    program,
                    equation.state,
                    target,
                    &edge.arguments,
                    &equation.local_alias_origins,
                    symbols,
                    &mut active_states,
                ) {
                    return None;
                }
            }
        }
    }
    if !has_transition_cycle {
        return None;
    }

    let mut summaries = equations
        .iter()
        .map(|equation| (equation.state.symbol, equation.direct_writes.clone()))
        .collect::<Vec<_>>();
    loop {
        let mut changed = false;
        for equation in &equations {
            for edge in &equation.edges {
                let target = equations
                    .iter()
                    .find(|candidate| candidate.state.symbol == edge.target)?;
                let target_writes = summaries
                    .iter()
                    .find(|(symbol, _)| *symbol == edge.target)?
                    .1
                    .clone();
                for relative in target_writes {
                    let Some(instantiated) = instantiate_written_path(
                        program,
                        &relative,
                        Some("self"),
                        program.state_parameters(target.state),
                        &edge.arguments,
                        &equation.locals,
                        symbols,
                        &mut outer_active_states.to_vec(),
                    )?
                    else {
                        continue;
                    };
                    let instantiated =
                        rebase_local_alias_path(&instantiated, &equation.local_alias_origins);
                    if !relative_state_path_is_visible(
                        &instantiated,
                        program.state_parameters(equation.state),
                        &equation.locals,
                    )? {
                        continue;
                    }
                    let source_writes = summaries
                        .iter_mut()
                        .find(|(symbol, _)| *symbol == equation.state.symbol)?;
                    if !source_writes.1.contains(&instantiated) {
                        source_writes.1.push(instantiated);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    summaries
        .into_iter()
        .find_map(|(symbol, writes)| (symbol == entry.symbol).then_some(writes))
}

fn build_permuted_cycle_frame_equation<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &'program State,
    symbols: &TopLevelSymbols<'program>,
    machine_symbols: &MachineSymbols<'program>,
    outer_active_states: &[SymbolHandle],
) -> Option<PermutedCycleFrameEquation<'program>> {
    let parameters = program.state_parameters(state);
    let mut locals = Vec::new();
    let mut isolated_local_roots = Vec::new();
    let mut local_alias_origins = Vec::<(String, FramePlaceOrigin)>::new();
    let mut direct_writes = Vec::new();
    let mut edges = Vec::new();
    let mut active_states = outer_active_states.to_vec();
    if !active_states.contains(&state.symbol) {
        active_states.push(state.symbol);
    }

    for statement in program.statement_table.statements(state.statement_nodes) {
        let declared_local_alias_origin = match statement {
            StatementNode::LocalData(local)
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference) =>
            {
                stable_local_mutable_alias_origin(
                    program,
                    machine,
                    machine_symbols,
                    &mut active_states,
                    local,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )
            }
            _ => None,
        };
        let representable_alias_rebinding = match statement {
            StatementNode::Assignment(assignment) => coarse_place_path(program, assignment.target)
                .is_some_and(|target| {
                    stable_local_mutable_alias_rebinding_is_representable(
                        program,
                        machine,
                        machine_symbols,
                        &mut active_states,
                        state,
                        &target,
                        assignment.value,
                        parameters,
                        &isolated_local_roots,
                        &local_alias_origins,
                        symbols,
                    )
                }),
            _ => false,
        };
        for expression in statement_value_expression_roots(program, statement) {
            if expression_reborrows_local_alias_binding(program, expression, &local_alias_origins)
                && declared_local_alias_origin.is_none()
                && !representable_alias_rebinding
            {
                return None;
            }
            let mut expression_writes = Vec::new();
            collect_expression_call_written_paths(
                program,
                expression,
                machine,
                machine_symbols,
                symbols,
                &mut active_states,
                &mut expression_writes,
            )?;
            for relative in expression_writes {
                let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
        }
        match statement {
            StatementNode::AssemblyFact(_) | StatementNode::Expression(_) => {}
            StatementNode::Assignment(assignment) => {
                let direct_target = coarse_place_path(program, assignment.target);
                if let Some(relative) = direct_target.as_deref()
                    && rebind_stable_local_mutable_alias_origin(
                        program,
                        machine,
                        machine_symbols,
                        &mut active_states,
                        state,
                        relative,
                        assignment.value,
                        parameters,
                        &isolated_local_roots,
                        &mut local_alias_origins,
                        symbols,
                    )?
                {
                    continue;
                }
                let relative = stable_assignment_target_path(
                    program,
                    machine,
                    machine_symbols,
                    &mut active_states,
                    assignment.target,
                    parameters,
                    &isolated_local_roots,
                    &local_alias_origins,
                    symbols,
                )?;
                push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
            }
            StatementNode::Call(call) => {
                let receiver_members = program
                    .statement_table
                    .name_path_members(call.receiver)
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect::<Vec<_>>();
                let arguments = program.statement_table.expression_handles(call.arguments);
                let argument_origins = arguments
                    .iter()
                    .map(|argument| {
                        stable_alias_initializer_origin(
                            program,
                            machine,
                            machine_symbols,
                            &mut active_states,
                            *argument,
                            parameters,
                            &isolated_local_roots,
                            &local_alias_origins,
                            symbols,
                            true,
                        )
                    })
                    .collect::<Vec<_>>();
                let nested_writes = known_call_written_paths_for_parts_with_origins(
                    program,
                    call.target_symbol,
                    call.target.as_str(),
                    &receiver_members,
                    arguments,
                    machine,
                    machine_symbols,
                    symbols,
                    &mut active_states,
                    Some(&argument_origins),
                )
                .or_else(|| {
                    (!arguments
                        .iter()
                        .any(|argument| expression_is_effectful_indexed_place(program, *argument)))
                    .then(|| {
                        known_boundary_call_written_paths_for_parts(
                            program,
                            machine_symbols,
                            symbols,
                            &receiver_members,
                            call.target.as_str(),
                            arguments,
                        )
                    })
                    .flatten()
                })
                .or_else(|| syntactic_call_written_paths(program, &receiver_members, arguments))?;
                for relative in nested_writes {
                    let relative = rebase_local_alias_path(&relative, &local_alias_origins);
                    push_visible_frame_path(&mut direct_writes, relative, parameters, &locals)?;
                }
            }
            StatementNode::Transition(transition) => {
                for target in [transition.target, transition.continuation] {
                    append_permuted_cycle_frame_edge(program, machine, state, target, &mut edges)?;
                }
            }
            StatementNode::LocalData(local) => {
                if type_may_carry_write(program, local.type_reference)
                    && !type_is_caller_isolated_local(program, local.type_reference)
                {
                    let origin = declared_local_alias_origin?;
                    local_alias_origins.push((local.name.as_str().to_owned(), origin));
                }
                if type_is_caller_isolated_local(program, local.type_reference) {
                    isolated_local_roots.push(local.name.as_str().to_owned());
                }
                locals.push(local.name.as_str().to_owned());
            }
        }
    }

    Some(PermutedCycleFrameEquation {
        state,
        locals,
        local_alias_origins,
        direct_writes,
        edges,
    })
}

fn push_visible_frame_path(
    writes: &mut Vec<String>,
    relative: String,
    parameters: &[StateParameter],
    locals: &[String],
) -> Option<()> {
    if relative_state_path_is_visible(&relative, parameters, locals)? && !writes.contains(&relative)
    {
        writes.push(relative);
    }
    Some(())
}

fn append_permuted_cycle_frame_edge(
    program: &TypedTrees,
    machine: &Machine,
    source: &State,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    edges: &mut Vec<PermutedCycleFrameEdge>,
) -> Option<()> {
    if !target.is_valid() {
        return Some(());
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Terminal
        | TransitionTargetNode::Value(_)
        | TransitionTargetNode::SelfTarget => Some(()),
        TransitionTargetNode::Named {
            path, arguments, ..
        } => {
            let target = program
                .machine_states(machine)
                .iter()
                .find(|candidate| candidate.symbol == path.symbol)
                .or_else(|| {
                    let members = program.statement_table.name_path_members(path.members);
                    matches!(members, [member] if member.as_str() == "self").then_some(source)
                })?;
            edges.push(PermutedCycleFrameEdge {
                target: target.symbol,
                arguments: program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
            });
            Some(())
        }
    }
}

fn transition_state_reaches(
    equations: &[PermutedCycleFrameEquation<'_>],
    start: SymbolHandle,
    sought: SymbolHandle,
) -> bool {
    let mut pending = vec![start];
    let mut visited = Vec::new();
    while let Some(symbol) = pending.pop() {
        if symbol == sought {
            return true;
        }
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some(equation) = equations
            .iter()
            .find(|equation| equation.state.symbol == symbol)
        else {
            return false;
        };
        pending.extend(equation.edges.iter().map(|edge| edge.target));
    }
    false
}

fn transition_is_exact_write_parameter_permutation(
    program: &TypedTrees,
    source: &State,
    target: &State,
    arguments: &[ExpressionHandle],
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> bool {
    let source_write_parameters = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self && parameter_may_carry_write(program, parameter))
        .collect::<Vec<_>>();
    let target_parameters = program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let target_write_positions = target_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter_may_carry_write(program, parameter))
        .collect::<Vec<_>>();
    if source_write_parameters.len() != target_write_positions.len()
        || target_parameters.len() != arguments.len()
    {
        return false;
    }

    let mut forwarded = Vec::new();
    for (position, _) in target_write_positions {
        let Some(source_parameter) = source_write_parameters.iter().find(|parameter| {
            expression_forwards_exact_write_parameter(
                program,
                arguments[position],
                parameter,
                aliases,
                symbols,
                active_states,
            )
        }) else {
            return false;
        };
        if forwarded.contains(&source_parameter.symbol) {
            return false;
        }
        forwarded.push(source_parameter.symbol);
    }
    forwarded.len() == source_write_parameters.len()
}

fn expression_forwards_exact_write_parameter(
    program: &TypedTrees,
    expression: ExpressionHandle,
    parameter: &StateParameter,
    aliases: &[(String, FramePlaceOrigin)],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> bool {
    if expression_forwards_exact_symbol(program, expression, parameter.symbol) {
        return true;
    }
    if transparent_place_expression_origin(program, expression, symbols, active_states).is_some_and(
        |origin| {
            origin.precision == FramePathPrecision::Exact && origin.path == parameter.name.as_str()
        },
    ) {
        return true;
    }
    let Some(argument) = frame_place_path(program, expression) else {
        return false;
    };
    let (root, suffix) = split_place_root(&argument.path);
    suffix.is_empty()
        && argument.precision == FramePathPrecision::Exact
        && aliases.iter().any(|(alias, origin)| {
            alias == root
                && origin.precision == FramePathPrecision::Exact
                && origin.path == parameter.name.as_str()
        })
}

#[allow(clippy::too_many_arguments)]
/// Summarize one tail transition in the source state's namespace. Named target
/// states compose only when their complete state graph is acyclic; target
/// parameters substitute through authored arguments exactly like call-frame
/// instantiation. Value-position call writes are collected before the jump.
fn summarize_transition_target_written_paths(
    program: &TypedTrees,
    machine: &Machine,
    source_state: &State,
    target: psi_typed_trees::statement::TransitionTargetHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    complete_state_summaries: &mut Vec<(SymbolHandle, Vec<String>)>,
    source_locals: &[String],
) -> Option<Vec<String>> {
    if !target.is_valid() {
        return Some(Vec::new());
    }
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Terminal => Some(Vec::new()),
        TransitionTargetNode::Value(_) => Some(Vec::new()),
        // A bare `-> self` re-enters this exact state with the same receiver
        // and parameter namespace. The body's writes have already been
        // collected, so another iteration adds no new caller-visible path.
        // Named cycles remain opaque below unless every edge around the cycle
        // forwards the complete state-parameter namespace positionally without
        // rebinding.
        TransitionTargetNode::SelfTarget => Some(Vec::new()),
        TransitionTargetNode::Named { arguments, .. } => {
            let arguments = program.statement_table.expression_handles(*arguments);
            let target_state =
                named_transition_target_state(program, machine, source_state, target)?;
            if active_states.contains(&target_state.symbol) {
                return named_transition_preserves_state_namespace(
                    program,
                    source_state,
                    target_state,
                    arguments,
                )
                .then(Vec::new);
            }
            active_states.push(target_state.symbol);
            let target_writes = summarize_state_written_paths(
                program,
                machine,
                target_state,
                symbols,
                active_states,
                complete_state_summaries,
            );
            active_states.pop();
            let target_writes = target_writes?;
            let parameters = program.state_parameters(target_state);
            let mut instantiated = Vec::new();
            for relative in target_writes {
                if let Some(path) = instantiate_written_path(
                    program,
                    &relative,
                    Some("self"),
                    parameters,
                    arguments,
                    source_locals,
                    symbols,
                    active_states,
                )? && !instantiated.contains(&path)
                {
                    instantiated.push(path);
                }
            }
            Some(instantiated)
        }
    }
}

/// A named edge closing a state cycle is frame-equivalent to a bare `self`
/// edge when every parameter capable of carrying caller-visible writes is fed
/// by the source parameter at that same ordinal. Reordering primitive values
/// and shared references cannot redirect a write and therefore does not make
/// an otherwise finite frame opaque. Parameter symbols are state-local, so a
/// multi-state cycle compares each write-capable argument to the source
/// namespace rather than requiring the target's distinct symbol.
fn named_transition_preserves_state_namespace(
    program: &TypedTrees,
    source_state: &State,
    target_state: &State,
    arguments: &[ExpressionHandle],
) -> bool {
    let source_parameters = program
        .state_parameters(source_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let target_parameters = program
        .state_parameters(target_state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    source_parameters.len() == target_parameters.len()
        && target_parameters.len() == arguments.len()
        && source_parameters
            .into_iter()
            .zip(target_parameters)
            .zip(arguments.iter().copied())
            .all(|((source, target), argument)| {
                !(parameter_may_carry_write(program, source)
                    || parameter_may_carry_write(program, target))
                    || expression_forwards_exact_symbol(program, argument, source.symbol)
            })
}

fn parameter_may_carry_write(program: &TypedTrees, parameter: &StateParameter) -> bool {
    type_may_carry_write(program, parameter.type_reference)
}

fn type_may_carry_write(program: &TypedTrees, handle: TypeReferenceHandle) -> bool {
    if program.primitive_type_reference(handle).is_some() {
        return false;
    }

    match program.type_reference_table.type_reference(handle) {
        TypeReferenceNode::Reference {
            is_mutable: false, ..
        } => false,
        TypeReferenceNode::Constrained { base_type, .. } => {
            type_may_carry_write(program, *base_type)
        }
        TypeReferenceNode::Unit | TypeReferenceNode::ConstExpression(_) => false,
        TypeReferenceNode::Reference {
            is_mutable: true, ..
        }
        | TypeReferenceNode::Named { .. }
        | TypeReferenceNode::Generic { .. }
        | TypeReferenceNode::FixedArray { .. }
        | TypeReferenceNode::Slice { .. }
        | TypeReferenceNode::DynamicTrait { .. } => true,
    }
}

fn expression_forwards_exact_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
    symbol: SymbolHandle,
) -> bool {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_forwards_exact_symbol(program, *inner, symbol),
        ExpressionNode::Name(path) => path.symbol == symbol,
        _ => false,
    }
}

fn relative_state_path_is_visible(
    relative: &str,
    parameters: &[StateParameter],
    locals: &[String],
) -> Option<bool> {
    let (root, _) = split_place_root(relative);
    if root == "self"
        || parameters
            .iter()
            .any(|parameter| parameter.name.as_str() == root)
    {
        return Some(true);
    }
    if locals.iter().any(|local| local == root) {
        return Some(false);
    }
    None
}

fn normalize_state_relative_path(
    program: &TypedTrees,
    state: &State,
    relative: &str,
) -> Option<Option<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        return Some(Some(append_place_suffix("self", suffix)));
    }
    if let Some(parameter_index) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == root)
    {
        return Some(Some(append_place_suffix(
            &format!("$P{parameter_index}"),
            suffix,
        )));
    }
    let is_local = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.name.as_str() == root)
        });
    is_local.then_some(None)
}

fn instantiate_written_path(
    program: &TypedTrees,
    relative: &str,
    receiver_base: Option<&str>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Option<String>> {
    instantiate_written_path_with_origins(
        program,
        relative,
        receiver_base,
        parameters,
        arguments,
        locals,
        symbols,
        active_states,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn instantiate_written_path_with_origins(
    program: &TypedTrees,
    relative: &str,
    receiver_base: Option<&str>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    locals: &[String],
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
    argument_origins: Option<&[Option<FramePlaceOrigin>]>,
) -> Option<Option<String>> {
    let (root, suffix) = split_place_root(relative);
    if root == "self" {
        return Some(Some(append_place_suffix(receiver_base?, suffix)));
    }
    if let Some(argument_index) = parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == root)
    {
        let argument = *arguments.get(argument_index)?;
        let base = argument_origins
            .and_then(|origins| origins.get(argument_index))
            .and_then(Clone::clone)
            .or_else(|| {
                transparent_place_expression_origin(program, argument, symbols, active_states)
            })?;
        return Some(Some(match base.precision {
            FramePathPrecision::Exact => append_place_suffix(&base.path, suffix),
            FramePathPrecision::CollectionCoarse => base.path,
        }));
    }
    if locals.iter().any(|local| local == root) {
        return Some(None);
    }
    // A write whose root is neither local nor a known parameter is externally
    // visible in a way this rung cannot instantiate safely.
    None
}

fn split_place_root(path: &str) -> (&str, &str) {
    let boundary = path.find(['.', '[']).unwrap_or(path.len());
    path.split_at(boundary)
}

fn append_place_suffix(base: &str, suffix: &str) -> String {
    format!("{base}{suffix}")
}

/// Coarsen indexed writes to their collection (`self.cells[i]` writes
/// `self.cells`). The value environment does not track index-sensitive facts.
fn coarse_place_path(program: &TypedTrees, expression: ExpressionHandle) -> Option<String> {
    Some(frame_place_path(program, expression)?.path)
}

/// Recover a frame path together with whether indexing discarded element
/// identity. Collection-coarse paths are absorbing: callers must not append a
/// callee/member suffix and accidentally manufacture `self.cells.value` from
/// a write through `self.cells[i].value`.
fn frame_place_path(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => frame_place_path(program, *inner),
        ExpressionNode::Indexed(indexed) => {
            let mut collection = frame_place_path(program, indexed.collection)?;
            collection.precision = FramePathPrecision::CollectionCoarse;
            Some(collection)
        }
        ExpressionNode::Member(member) => {
            let receiver = frame_place_path(program, member.receiver)?;
            Some(match receiver.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: format!("{}.{}", receiver.path, member.member.as_str()),
                    precision: FramePathPrecision::Exact,
                },
                FramePathPrecision::CollectionCoarse => receiver,
            })
        }
        _ => Some(FramePlaceOrigin {
            path: arithmetic_domains::place_path(program, expression)?,
            precision: FramePathPrecision::Exact,
        }),
    }
}
