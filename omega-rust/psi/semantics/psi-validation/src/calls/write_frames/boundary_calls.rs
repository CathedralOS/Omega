//! Boundary-trait call resolution and caller-visible write frames.
//!
//! Boundary calls have no locally inspectable body. This owner resolves the
//! selected trait signature and derives the exact receiver/exclusive-argument
//! frame, failing closed when a mutable argument has no supported storage origin.

use super::isolation::type_is_caller_isolated_local;
use super::reference_origins::{exclusive_reference_origin, referent_has_only_owned_storage};
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::ExpressionHandle;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::statement::TableCall;
use psi_typed_trees::types::TypeReferenceNode;

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
        let path = exclusive_reference_origin(
            program,
            current_machine,
            *argument,
            symbols,
            active_states,
        )?
        .path;
        if !written.contains(&path) {
            written.push(path);
        }
    }

    Some(written)
}
