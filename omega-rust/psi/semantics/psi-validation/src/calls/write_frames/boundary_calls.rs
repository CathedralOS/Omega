//! Boundary-trait call resolution and caller-visible write frames.
//!
//! Boundary calls have no locally inspectable body. This owner resolves the
//! selected trait signature and derives the exact receiver/exclusive-argument
//! frame, failing closed when a mutable argument has no supported storage origin.

use super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::isolation::{data_definition_has_only_owned_storage, type_is_caller_isolated_local};
use super::{
    FramePathPrecision, FramePlaceOrigin, frame_place_path, transparent_call_result_origin,
};
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::machine::Machine;
use psi_typed_trees::statement::{StatementNode, TableCall};
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Recognition is deliberately broader than signature selection: even an
/// invalid prefix or ambiguous member on a cached trait receiver must not
/// regain a complete frame through the signature-free fallback.
pub(super) fn receiver_requires_boundary_frame(
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
) -> bool {
    receiver
        .last()
        .and_then(|name| machine_symbols.callable_field_type(name))
        .and_then(|name| symbols.trait_definition(name))
        .is_some()
}

/// The boundary-trait signature a call statement resolves to (`self.fw.
/// get_size(..)` -> trait `Firmware`'s `get_size`), or None for every other
/// receiver class. Used by the R4 witness mint (out-param ensures seeding the
/// value env). Resolution uses the current machine's attached-field cache;
/// arbitrary receiver prefixes cannot select a same-named cached field.
pub(crate) fn boundary_trait_signature<'program>(
    program: &'program TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    call: &TableCall,
) -> Option<&'program psi_typed_trees::signature::StateSignature> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    boundary_trait_signature_for_parts(
        program,
        machine_symbols,
        symbols,
        &receiver_members,
        call.target.as_str(),
    )
}

pub(super) fn boundary_trait_signature_for_parts<'program>(
    program: &'program TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
) -> Option<&'program psi_typed_trees::signature::StateSignature> {
    let [root, receiver] = receiver_members else {
        return None;
    };
    if root != "self" {
        return None;
    }
    let receiver_type = machine_symbols.callable_field_type(receiver)?;
    let trait_definition = symbols.trait_definition(receiver_type)?;
    if !trait_definition.is_boundary || !trait_definition.type_parameters.is_empty() {
        return None;
    }
    let mut signatures = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| signature.name.as_str() == target);
    let signature = signatures.next()?;
    (signatures.next().is_none() && signature.type_parameters.is_empty()).then_some(signature)
}

/// The program-place frame of a resolved boundary call. Boundary code may
/// mutate its receiver and every supplied
/// exclusive argument; it cannot manufacture reach to unrelated caller
/// fields. A direct exclusive borrow or a verified caller reference binding
/// supplies that argument's path. Checked helpers can transport that origin
/// through their proven result relation. Untracked reference reach stays opaque.
pub(crate) fn known_boundary_call_written_paths(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCall,
) -> Option<Vec<String>> {
    let receiver = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    known_boundary_call_written_paths_for_parts(
        program,
        current_machine,
        machine_symbols,
        symbols,
        &receiver,
        call.target.as_str(),
        program.statement_table.expression_handles(call.arguments),
        &mut Vec::new(),
    )
}

pub(super) fn known_boundary_call_written_paths_for_parts(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    arguments: &[ExpressionHandle],
    active_states: &mut Vec<SymbolHandle>,
) -> Option<Vec<String>> {
    let signature =
        boundary_trait_signature_for_parts(program, machine_symbols, symbols, receiver, target)?;
    let mut written = vec![receiver.join(".")];
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }

    for (parameter, argument) in parameters.into_iter().zip(arguments) {
        let mut parameter_type = parameter.type_reference;
        while let TypeReferenceNode::Constrained { base_type, .. } =
            program.type_reference_table.type_reference(parameter_type)
        {
            parameter_type = *base_type;
        }
        if !parameter_type.is_valid() {
            return None;
        }
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            if !matches!(
                program.type_reference_table.type_reference(parameter_type),
                TypeReferenceNode::Unit
            ) && !type_is_caller_isolated_local(program, parameter_type)
            {
                // A by-value carrier can still contain mutable references.
                // Without leaf-origin transport, omitting their writes would
                // manufacture a complete receiver-only frame.
                return None;
            }
            continue;
        };
        if !access.is_exclusive() {
            continue;
        }
        if !referent_has_only_owned_storage(program, *referee) {
            return None;
        }
        let path =
            boundary_reference_origin(program, current_machine, *argument, symbols, active_states)?
                .path;
        if !written.contains(&path) {
            written.push(path);
        }
    }

    Some(written)
}

/// Reuse the checked body's result relation, validating its selected input at
/// this boundary. A helper cannot turn an untracked reference-bearing carrier
/// or a foreign binding identity into a proven caller storage origin.
fn boundary_reference_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
    symbols: &TopLevelSymbols<'_>,
    active_states: &mut Vec<SymbolHandle>,
) -> Option<FramePlaceOrigin> {
    match program.expression_table.expression(argument) {
        ExpressionNode::Borrow(place) if place.access.is_exclusive() => {
            frame_place_path(program, place.target)
        }
        ExpressionNode::Name(_) => Some(FramePlaceOrigin {
            path: exclusive_reference_binding_path(program, current_machine, argument)?,
            precision: FramePathPrecision::Exact,
        }),
        ExpressionNode::Call(call) => transparent_call_result_origin(
            program,
            call,
            symbols,
            active_states,
            |callee_machine, parameter, actual, active_states| {
                let referee = exclusive_reference_referee(program, parameter.type_reference)?;
                let owned = if parameter.is_self {
                    // The typed receiver uses nominal `Self`, whose concrete
                    // declaration belongs to this resolved attached machine.
                    let attached = callee_machine.attached_data.as_ref()?;
                    let mut definitions = program
                        .data_definitions()
                        .iter()
                        .filter(|definition| definition.name == *attached);
                    let definition = definitions.next()?;
                    definitions.next().is_none()
                        && data_definition_has_only_owned_storage(program, definition)
                } else {
                    referent_has_only_owned_storage(program, referee)
                };
                if !owned {
                    return None;
                }
                if parameter.is_self {
                    // Attached methods implicitly borrow their owned receiver;
                    // they cannot bypass the origin fence on a loaded reference.
                    match program.expression_table.expression(actual) {
                        ExpressionNode::Name(_)
                        | ExpressionNode::Member(_)
                        | ExpressionNode::Indexed(_) => {
                            return boundary_receiver_origin(program, current_machine, actual);
                        }
                        _ => {}
                    }
                }
                boundary_reference_origin(program, current_machine, actual, symbols, active_states)
            },
        ),
        _ => None,
    }
}

fn exclusive_reference_has_owned_storage(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    exclusive_reference_referee(program, reference)
        .is_some_and(|referee| referent_has_only_owned_storage(program, referee))
}

fn exclusive_reference_referee(
    program: &TypedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            exclusive_reference_referee(program, *base_type)
        }
        TypeReferenceNode::Reference {
            access, referee, ..
        } => (reference.is_valid() && access.is_exclusive()).then_some(*referee),
        _ => None,
    }
}

fn boundary_receiver_origin(
    program: &TypedTrees,
    current_machine: &Machine,
    expression: ExpressionHandle,
) -> Option<FramePlaceOrigin> {
    let parent = match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => {
            let reference = caller_binding_type(program, current_machine, expression)?;
            if exclusive_reference_referee(program, reference).is_none()
                && !type_is_caller_isolated_local(program, reference)
            {
                return None;
            }
            return frame_place_path(program, expression);
        }
        ExpressionNode::Member(member) => member.receiver,
        ExpressionNode::Indexed(indexed) => indexed.collection,
        _ => return None,
    };
    boundary_receiver_origin(program, current_machine, parent)?;
    let (state, _, _) = caller_statement_at_site(
        program,
        current_machine,
        CallerWriteSite::Expression(expression),
    )?;
    let reference =
        crate::places::declared_place_type_raw(program, current_machine, Some(state), expression)
            .or_else(|| {
            crate::places::declared_indexed_projection_type_raw(
                program,
                current_machine,
                Some(state),
                expression,
            )
        })?;
    if !type_is_caller_isolated_local(program, reference) {
        return None;
    }
    frame_place_path(program, expression)
}

/// Forward a reference value, not a borrow of its binding slot. Only an exact
/// caller declaration can supply its type. This returns a raw binding path:
/// the state transfer and public demand closure own local-origin admission.
/// Replaying that prefix here would recursively replay every earlier boundary
/// call. Carrier fields stay opaque.
fn exclusive_reference_binding_path(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<String> {
    let reference = caller_binding_type(program, current_machine, argument)?;
    exclusive_reference_has_owned_storage(program, reference)
        .then(|| frame_place_path(program, argument).map(|origin| origin.path))
        .flatten()
}

fn caller_binding_type(
    program: &TypedTrees,
    current_machine: &Machine,
    argument: ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    let ExpressionNode::Name(name) = program.expression_table.expression(argument) else {
        return None;
    };
    let [member] = program.expression_table.name_path_members(name.members) else {
        return None;
    };
    if !name.symbol.is_valid() || name.head_symbol != name.symbol {
        return None;
    }
    let (state, _, index) = caller_statement_at_site(
        program,
        current_machine,
        CallerWriteSite::Expression(argument),
    )?;
    let declaration = program.symbols.get(name.symbol);
    // Typed `self` paths retain the owning machine identity, not the synthetic
    // state parameter identity. Only that exact machine may select this state's
    // unique receiver declaration.
    if member.as_str() == "self"
        && name.symbol == current_machine.symbol
        && declaration.kind == psi_symbols::SymbolKind::Machine
    {
        let mut receivers = program
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.is_self);
        let receiver = receivers.next()?;
        return (receivers.next().is_none() && receiver.type_reference.is_valid())
            .then_some(receiver.type_reference);
    }
    if declaration.parent != state.symbol || program.symbols.name(name.symbol) != member.as_str() {
        return None;
    }
    let reference = match declaration.kind {
        psi_symbols::SymbolKind::Parameter => {
            program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == name.symbol)?
                .type_reference
        }
        psi_symbols::SymbolKind::Local => {
            let local = program.statement_table.statements(state.statement_nodes)[..index]
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == name.symbol => Some(local),
                    _ => None,
                })?;
            local.type_reference
        }
        _ => return None,
    };
    reference.is_valid().then_some(reference)
}

fn referent_has_only_owned_storage(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, .. } => {
            referent_has_only_owned_storage(program, *base_type)
        }
        TypeReferenceNode::Slice { element_type } => {
            type_is_caller_isolated_local(program, *element_type)
        }
        TypeReferenceNode::Unit => reference.is_valid(),
        _ => type_is_caller_isolated_local(program, reference),
    }
}
