//! Boundary-trait call resolution and caller-visible write frames.
//!
//! Boundary calls have no locally inspectable body. This owner resolves the
//! selected trait signature and derives the exact receiver/exclusive-argument
//! frame, failing closed when a mutable argument has no supported storage origin.

use super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::isolation::{aggregate_storage_types_match, type_is_caller_isolated_local};
use super::place_paths::{FramePathPrecision, FramePlaceOrigin, FrameSourcePlace};
use super::receiver_member_chain;
use super::reference_origins::{exclusive_reference_origin, referent_has_only_owned_storage};
use super::type_capabilities::type_may_carry_write;
use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::FrameInference;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::statement::TableCall;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

/// Recognition is deliberately broader than signature selection: even an
/// invalid prefix or ambiguous member on a cached trait receiver must not
/// regain a complete frame through the signature-free fallback.
pub(super) fn receiver_requires_boundary_frame(
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
) -> bool {
    receiver.last().is_some_and(|name| {
        machine_symbols
            .callable_field_type(name)
            .and_then(|name| symbols.trait_definition(name))
            .is_some()
            || symbols
                .trait_definition(name)
                .is_some_and(|definition| definition.is_boundary)
    })
}

/// The boundary-trait signature a call statement resolves to (`self.fw.
/// get_size(..)` -> trait `Firmware`'s `get_size`), or an exact parameter
/// receiver's selected signature. Used by the R4 witness mint (out-param
/// ensures seeding the value env). Arbitrary prefixes cannot select a cached
/// field, and a sibling state's same-named parameter supplies no authority.
pub(crate) fn boundary_trait_signature<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    call: &TableCall,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let receiver_members = program
        .statement_table
        .name_path_members(call.receiver)
        .iter()
        .map(|member| member.as_str().to_owned())
        .collect::<Vec<_>>();
    boundary_trait_signature_for_parts(
        program,
        current_machine,
        machine_symbols,
        symbols,
        &receiver_members,
        call.target.as_str(),
        CallerWriteSite::Call(call),
    )
}

pub(super) fn boundary_trait_signature_for_parts<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<&'program typed_trees::signature::StateSignature> {
    boundary_trait_signature_and_receiver(
        program,
        current_machine,
        machine_symbols,
        symbols,
        receiver_members,
        target,
        site,
    )
    .map(|(signature, _)| signature)
}

/// The Boolean records actual runtime receiver storage, not a trait qualifier.
/// Both static and receiver calls keep the same exact signature selection and
/// exclusive-argument origin rules.
fn boundary_trait_signature_and_receiver<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<(&'program typed_trees::signature::StateSignature, bool)> {
    let (receiver_symbol, target_symbol) = match site {
        CallerWriteSite::Call(call) => (call.receiver_symbol, call.target_symbol),
        CallerWriteSite::Expression(expression) => {
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            (
                expression_receiver_symbol(program, call.receiver),
                call.target_symbol,
            )
        }
        CallerWriteSite::Statement(_) => return None,
    };
    let (trait_definition, has_runtime_receiver) = match receiver_members {
        [receiver] => {
            if !receiver_symbol.is_valid() {
                return None;
            }
            let (state, _, _) = caller_statement_at_site(program, current_machine, site)?;
            if let Some(parameter) = program.state_parameters(state).iter().find(|parameter| {
                parameter.symbol == receiver_symbol && parameter.name.as_str() == receiver
            }) {
                let receiver_type = receiver_type_symbol(program, parameter.type_reference);
                (
                    program
                        .traits()
                        .iter()
                        .find(|definition| definition.symbol == receiver_type)?,
                    true,
                )
            } else {
                // A qualified static call names the trait declaration itself;
                // that qualifier is not reachable caller storage.
                let mut definitions = program.traits().iter().filter(|definition| {
                    definition.symbol == receiver_symbol && definition.name.as_str() == receiver
                });
                let definition = definitions.next()?;
                if definitions.next().is_some()
                    || program.symbols.get(receiver_symbol).kind != symbols::SymbolKind::Trait
                    || program.symbols.name(receiver_symbol) != definition.name.as_str()
                {
                    return None;
                }
                (definition, false)
            }
        }
        [root, receiver] if root == "self" => {
            let receiver_type = machine_symbols.callable_field_type(receiver)?;
            (symbols.trait_definition(receiver_type)?, true)
        }
        _ => return None,
    };
    if !trait_definition.is_boundary || !trait_definition.type_parameters.is_empty() {
        return None;
    }
    let mut signatures = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| signature.name.as_str() == target);
    let signature = signatures.next()?;
    (signatures.next().is_none()
        && signature.type_parameters.is_empty()
        && target_symbol.is_valid()
        && signature.symbol == target_symbol
        && (has_runtime_receiver
            || (program.symbols.get(target_symbol).kind == symbols::SymbolKind::State
                && program.symbols.get(target_symbol).parent == trait_definition.symbol
                && program.symbols.name(target_symbol) == signature.name.as_str()))
        && (has_runtime_receiver
            || !program
                .state_signature_parameters(signature)
                .iter()
                .any(|parameter| parameter.is_self)))
    .then_some((signature, has_runtime_receiver))
}

fn receiver_type_symbol(
    program: &TypedTrees,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => return *symbol,
            _ => return SymbolHandle::invalid(),
        }
    }
}

pub(super) fn expression_receiver_requires_boundary_frame(
    program: &TypedTrees,
    machine: &Machine,
    receiver: ExpressionHandle,
) -> bool {
    super::caller_aliases::caller_name_root_type(program, machine, receiver)
        .map(|reference| receiver_type_symbol(program, reference))
        .is_some_and(|symbol| {
            symbol.is_valid()
                && program
                    .traits()
                    .iter()
                    .any(|definition| definition.symbol == symbol && definition.is_boundary)
        })
}

fn expression_receiver_symbol(program: &TypedTrees, receiver: ExpressionHandle) -> SymbolHandle {
    match program.expression_table.expression(receiver) {
        typed_trees::expression::ExpressionNode::Name(name)
            if program
                .expression_table
                .name_path_members(name.members)
                .len()
                == 1
                && name.head_symbol != name.symbol =>
        {
            SymbolHandle::invalid()
        }
        typed_trees::expression::ExpressionNode::Name(name) => name.symbol,
        typed_trees::expression::ExpressionNode::Member(member) => member.member_symbol,
        _ => SymbolHandle::invalid(),
    }
}

pub(super) fn selected_boundary_signature(program: &TypedTrees, target: SymbolHandle) -> bool {
    target.is_valid()
        && program.traits().iter().any(|definition| {
            definition.is_boundary
                && program
                    .trait_machine_signatures(definition)
                    .iter()
                    .any(|signature| signature.symbol == target)
        })
}

/// The program-place frame of a resolved boundary call. Boundary code may
/// mutate its receiver and every supplied
/// exclusive argument; it cannot manufacture reach to unrelated caller
/// fields. A direct exclusive borrow or a verified caller reference binding
/// supplies that argument's path. Checked helpers transport that origin
/// through their proven result relation, and a nested boundary call's
/// exclusive result transports the caller routes its selected signature
/// admits. Untracked reference reach stays opaque.
pub(super) fn known_boundary_call_written_paths_for_parts(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let (signature, has_runtime_receiver) = boundary_trait_signature_and_receiver(
        program,
        current_machine,
        machine_symbols,
        symbols,
        receiver,
        target,
        site,
    )?;
    let mut written = if has_runtime_receiver {
        vec![receiver.join(".")]
    } else {
        Vec::new()
    };
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }

    for (parameter, argument) in parameters.into_iter().zip(arguments) {
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
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
        for origin in boundary_argument_origins(
            program,
            current_machine,
            machine_symbols,
            symbols,
            *argument,
            inference,
        )? {
            if !written.contains(&origin.path) {
                written.push(origin.path);
            }
        }
    }

    Some(written)
}

/// The caller storage an exclusive argument may actually reach. A nested
/// boundary call transports the candidate origins its selected signature
/// admits for the result; every other supported expression contributes its
/// single proven origin. The nested call's own receiver/argument footprint
/// is counted separately by the enclosing expression/statement walk; only
/// its exclusive result's may-alias routes join this argument's origins.
fn boundary_argument_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    argument: ExpressionHandle,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    if let ExpressionNode::Call(call) = program.expression_table.expression(argument)
        && selected_boundary_signature(program, call.target_symbol)
    {
        return boundary_result_origins(
            program,
            current_machine,
            machine_symbols,
            symbols,
            call,
            argument,
            inference,
        );
    }
    exclusive_reference_origin(program, current_machine, argument, symbols, inference)
        .map(|origin| vec![origin])
}

/// The candidate caller-storage origins a boundary call's exclusive result
/// may alias. There is no inspectable body: a `&mut`/`&write` result can
/// reach the runtime receiver's opaque storage or the storage behind an
/// exclusive argument whose own storage may hold the referent. Shared
/// references cannot lend exclusive reach. A by-value or exclusive carrier
/// whose stored exclusive references could still reach the referent — and
/// therefore route the result into untracked storage — keeps the whole
/// result opaque, as does a result with no admitted caller route at all.
fn boundary_result_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCallExpression,
    expression: ExpressionHandle,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let receiver = receiver_member_chain(program, call.receiver).unwrap_or_default();
    let (signature, has_runtime_receiver) = boundary_trait_signature_and_receiver(
        program,
        current_machine,
        machine_symbols,
        symbols,
        &receiver,
        call.target.as_str(),
        CallerWriteSite::Expression(expression),
    )?;
    let result_type = live_unconstrained_type(program, signature.return_type)?;
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = program.type_reference_table.type_reference(result_type)
    else {
        return None;
    };
    if !access.is_exclusive() {
        return None;
    }
    let referent = live_unconstrained_type(program, *referee)?;

    let mut origins = Vec::new();
    if has_runtime_receiver {
        // The implementor's storage is opaque to the caller: an exclusive
        // result may point anywhere inside it, and the receiver place covers
        // every such subpath.
        origins.push(FramePlaceOrigin {
            path: receiver.join("."),
            precision: FramePathPrecision::Exact,
            source: FrameSourcePlace::from_expression(program, call.receiver),
        });
    }
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let arguments = program.expression_table.expression_handles(call.arguments);
    if parameters.len() != arguments.len() {
        return None;
    }
    for (parameter, actual) in parameters.into_iter().zip(arguments) {
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            // A by-value carrier can still store exclusive references whose
            // referents this frame cannot name; its route stays opaque.
            if type_may_carry_write(program, parameter_type) {
                return None;
            }
            continue;
        };
        if !access.is_exclusive() {
            continue;
        }
        match owned_storage_may_hold(program, *referee, referent) {
            Some(true) => {
                for origin in boundary_argument_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    *actual,
                    inference,
                )? {
                    push_unique_origin(&mut origins, origin);
                }
            }
            Some(false) => {}
            None => return None,
        }
    }
    (!origins.is_empty()).then_some(origins)
}

fn push_unique_origin(origins: &mut Vec<FramePlaceOrigin>, origin: FramePlaceOrigin) {
    if !origins.iter().any(|existing| {
        existing.path == origin.path
            && existing.precision == origin.precision
            && existing.source == origin.source
    }) {
        origins.push(origin);
    }
}

/// May `container`'s own storage hold a `referent`-typed value? The walk
/// follows declared fields and elements only, never a reference's own
/// storage: `Some(true)` finds the referent, `Some(false)` rules it out, and
/// `None` means a stored exclusive reference or an unfinished proof could
/// still reach one, so the callee may route the result into storage this
/// frame cannot name.
fn owned_storage_may_hold(
    program: &TypedTrees,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
) -> Option<bool> {
    owned_storage_may_hold_inner(program, container, referent, &mut Vec::new())
}

fn owned_storage_may_hold_inner(
    program: &TypedTrees,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    visiting: &mut Vec<SymbolHandle>,
) -> Option<bool> {
    let container = live_unconstrained_type(program, container)?;
    if aggregate_storage_types_match(program, container, referent) {
        return Some(true);
    }
    if program.primitive_type_reference(container).is_some() {
        return Some(false);
    }
    match program.type_reference_table.type_reference(container) {
        TypeReferenceNode::Reference {
            access, referee, ..
        } => {
            // The slot stores the reference, not the referent value. Shared
            // references cannot lend exclusive reach; an exclusive one may
            // still route the result into its own untracked storage.
            if access.is_exclusive()
                && owned_storage_may_hold_inner(program, *referee, referent, visiting)
                    != Some(false)
            {
                return None;
            }
            Some(false)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            owned_storage_may_hold_inner(program, *element_type, referent, visiting)
        }
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic {
            base_symbol: symbol,
            ..
        } => {
            let mut definitions = program
                .data_definitions()
                .iter()
                .filter(|definition| definition.symbol == *symbol);
            let Some(definition) = definitions.next() else {
                // An uninspected nominal — built-in, dynamic, or a type
                // parameter — could hold the referent.
                return Some(true);
            };
            if definitions.next().is_some() {
                return Some(true);
            }
            if visiting.contains(&definition.symbol) {
                // A recursive shape cannot finish the proof; a stored
                // exclusive link in the cycle may still reach the referent.
                return None;
            }
            visiting.push(definition.symbol);
            let mut found = false;
            for member in program.data_members(definition) {
                let field_types: Vec<TypeReferenceHandle> = match member {
                    DataMember::Field(field) => vec![field.type_reference],
                    DataMember::Variant(variant) => program
                        .data_payload_fields(variant)
                        .iter()
                        .map(|field| field.type_reference)
                        .collect(),
                };
                for field_type in field_types {
                    match owned_storage_may_hold_inner(program, field_type, referent, visiting) {
                        Some(true) => found = true,
                        Some(false) => {}
                        None => {
                            visiting.pop();
                            return None;
                        }
                    }
                }
            }
            visiting.pop();
            Some(found)
        }
        // Provider-opaque storage may hold the referent.
        TypeReferenceNode::DynamicTrait { .. } => Some(true),
        TypeReferenceNode::Unit => Some(false),
        // A proof-static type-level expression is uninspectable storage
        // here; keep the result opaque rather than ruling the route out.
        TypeReferenceNode::ConstExpression(_) => None,
        // `live_unconstrained_type` above already erased every constraint.
        TypeReferenceNode::Constrained { .. } => None,
    }
}

/// A stale nonzero handle resolves to dummy Unit, not an effect-free formal.
/// An acyclic chain visits at most the number of stored type nodes, so this
/// bound rejects cycles without imposing a smaller depth limit on valid types.
fn live_unconstrained_type(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    for _ in 0..program.type_reference_table.type_reference_count() {
        if !program
            .type_reference_table
            .contains_type_reference(reference)
        {
            return None;
        }
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            _ => return Some(reference),
        }
    }
    None
}
