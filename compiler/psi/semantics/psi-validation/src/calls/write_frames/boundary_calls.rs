//! Boundary-trait call resolution and caller-visible write frames.
//!
//! Boundary calls have no locally inspectable body. This owner resolves the
//! selected trait signature and derives the exact receiver/exclusive-argument
//! frame, failing closed when a mutable argument is not a direct place.

use super::coarse_place_path;
use crate::symbols::{MachineSymbols, TopLevelSymbols};
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::statement::TableCall;
use psi_typed_trees::types::TypeReferenceNode;

/// The boundary-trait signature a call statement resolves to (`self.fw.
/// get_size(..)` -> trait `Firmware`'s `get_size`), or None for every other
/// receiver class. Mirrors `validate_call_node`'s trait branch; used by the
/// R4 witness mint (out-param ensures seeding the value env). Kept
/// cache-based (vs the shared `psi_typed_trees::boundary` chain the
/// checker/proof consumers use) because `contained_type` also resolves
/// `contains`-clause receivers, not just attached-data fields.
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

fn boundary_trait_signature_for_parts<'program>(
    program: &'program TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
) -> Option<&'program psi_typed_trees::signature::StateSignature> {
    let receiver = receiver_members.last()?.as_str();
    let receiver_type = machine_symbols.callable_field_type(receiver)?;
    let trait_definition = symbols.trait_definition(receiver_type)?;
    program
        .trait_machine_signatures(trait_definition)
        .iter()
        .find(|signature| signature.name.as_str() == target)
}

/// The program-place frame of a resolved boundary call before authored
/// `stores` lands. Boundary code may mutate its receiver and every explicit
/// exclusive argument; it cannot manufacture reach to unrelated caller
/// fields. An exclusive parameter not represented by a direct `&mut place`
/// remains opaque and returns `None`, preserving the fail-closed fallback.
pub(crate) fn known_boundary_call_written_paths(
    program: &TypedTrees,
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
        machine_symbols,
        symbols,
        &receiver,
        call.target.as_str(),
        program.statement_table.expression_handles(call.arguments),
    )
}

pub(super) fn known_boundary_call_written_paths_for_parts(
    program: &TypedTrees,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    arguments: &[ExpressionHandle],
) -> Option<Vec<String>> {
    let signature =
        boundary_trait_signature_for_parts(program, machine_symbols, symbols, receiver, target)?;
    if receiver.is_empty() {
        return None;
    }
    let mut written = vec![receiver.join(".")];
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self);

    for (parameter, argument) in parameters.zip(arguments) {
        let TypeReferenceNode::Reference { access, .. } = program
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            continue;
        };
        if !access.is_exclusive() {
            continue;
        }
        let ExpressionNode::Mutable(place) = program.expression_table.expression(*argument) else {
            return None;
        };
        let path = coarse_place_path(program, *place)?;
        if !written.contains(&path) {
            written.push(path);
        }
    }

    Some(written)
}
