//! Boundary-trait call resolution and caller-visible write frames.
//!
//! Boundary calls have no locally inspectable body. This owner resolves the
//! selected trait signature and derives the exact receiver/exclusive-argument
//! frame, failing closed when a mutable argument has no supported storage origin.
//!
//! Generic receivers retain their exact owner tuple before method inference:
//! losing it to the signature-free ceiling made copied scalar metadata appear
//! writable. Owner and method bindings share the existing storage/origin
//! queries, keyed by distinct symbols. This substitution is private to frame
//! inference; fact-seeding consumers without a bound environment still refuse
//! generic signatures rather than reading uninstantiated contracts.

use super::caller_aliases::{CallerWriteSite, caller_statement_at_site};
use super::isolation::{aggregate_storage_types_match_in, type_is_caller_isolated_local_in};
use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, FrameSourcePlace, append_place_suffix, coarse_place_path,
    push_unique_origin, split_place_root,
};
use super::receiver_member_chain;
use super::reference_origins::{exclusive_reference_origins, referent_has_only_owned_storage_in};
use super::stored_origins::StoredLocalOrigins;
use super::type_capabilities::type_may_carry_write_in;
use super::type_instantiation::{
    TypeBindings, push_generic_application_bindings, substituted_head,
};
use crate::declarations::symbols::{MachineSymbols, TopLevelSymbols};
use crate::machine_calls::calls::write_frames::FrameInference;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::statement::TableCall;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

/// Recognition is deliberately broader than signature selection: even an
/// invalid prefix or ambiguous member on a cached trait receiver must not
/// regain a complete frame through the signature-free fallback.
pub(super) fn receiver_requires_boundary_frame(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
) -> bool {
    let is_boundary = |reference| {
        boundary_receiver_type_symbol(program, reference).is_some_and(|symbol| {
            program
                .traits()
                .iter()
                .any(|definition| definition.symbol == symbol && definition.is_boundary)
        })
    };
    // Recognition grants no signature authority. Any matching declaration
    // must fence the signature-free fallback, even when another state's
    // same-spelled parameter would be an ordinary value.
    let has_boundary_receiver = match receiver {
        [root, member] if root == "self" => {
            crate::value_custody::places::machine_attached_data(program, current_machine)
                .is_some_and(|data| {
                    program.data_members(data).iter().any(|field| match field {
                        DataMember::Field(field) => {
                            field.name.as_str() == member && is_boundary(field.type_reference)
                        }
                        _ => false,
                    })
                })
        }
        [name] => program.machine_states(current_machine).iter().any(|state| {
            program.state_parameters(state).iter().any(|parameter| {
                parameter.name.as_str() == name && is_boundary(parameter.type_reference)
            })
        }),
        _ => false,
    };
    if has_boundary_receiver {
        return true;
    }
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
    boundary_trait_signature_and_receiver_inner(
        program,
        current_machine,
        machine_symbols,
        symbols,
        receiver_members,
        target,
        site,
        false,
    )
    .map(|selected| (selected.signature, selected.has_runtime_receiver))
}

struct BoundaryWriteSignature<'program> {
    signature: &'program typed_trees::signature::StateSignature,
    has_runtime_receiver: bool,
    bindings: TypeBindings,
}

/// Resolve a receiver head without discarding its application arguments:
/// callers retain the original reference for exact owner substitution.
fn boundary_receiver_type_symbol(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    for _ in 0..program.type_reference_table.type_reference_count() {
        // Validate the constraint chain before the service classifier walks it.
        let unconstrained = live_unconstrained_type(program, reference)?;
        // A provisioned service calls its exact boundary requirement, not the
        // opaque carrier's data owner. Classify before peeling qualifications:
        // an invalid Service shell must not acquire a complete write frame.
        if let Some(carrier) =
            typed_trees::service::classify_exact_bound_service_carrier(program, reference).ok()?
        {
            return Some(carrier.requirement);
        }
        reference = unconstrained;
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Reference { referee, .. } => reference = *referee,
            TypeReferenceNode::Generic { base_symbol, .. } => return Some(*base_symbol),
            TypeReferenceNode::Named { symbol, .. }
            | TypeReferenceNode::DynamicTrait { symbol, .. } => return Some(*symbol),
            _ => return None,
        }
    }
    None
}

/// The write-frame route additionally admits a signature whose `Type`
/// parameters the call's actual arguments pin concretely; callers then prove
/// every carrier against the instantiated environment. A parameter that no
/// argument binds keeps the named leaf and fails the same single-origin
/// gates.
fn boundary_write_signature_and_receiver<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<BoundaryWriteSignature<'program>> {
    boundary_trait_signature_and_receiver_inner(
        program,
        current_machine,
        machine_symbols,
        symbols,
        receiver_members,
        target,
        site,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn boundary_trait_signature_and_receiver_inner<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    _machine_symbols: &MachineSymbols<'_>,
    _symbols: &TopLevelSymbols<'program>,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
    allow_type_parameters: bool,
) -> Option<BoundaryWriteSignature<'program>> {
    let (receiver_symbol, target_symbol) = call_site_symbols(program, site)?;
    let (trait_definition, has_runtime_receiver, receiver_reference) = match receiver_members {
        [receiver] => {
            if !receiver_symbol.is_valid() {
                return None;
            }
            let (state, _, _) = caller_statement_at_site(program, current_machine, site)?;
            if let Some(parameter) = program.state_parameters(state).iter().find(|parameter| {
                parameter.symbol == receiver_symbol && parameter.name.as_str() == receiver
            }) {
                let receiver_type =
                    boundary_receiver_type_symbol(program, parameter.type_reference)?;
                (
                    program
                        .traits()
                        .iter()
                        .find(|definition| definition.symbol == receiver_type)?,
                    true,
                    parameter.type_reference,
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
                (definition, false, TypeReferenceHandle::invalid())
            }
        }
        [root, receiver] if root == "self" => {
            let data =
                crate::value_custody::places::machine_attached_data(program, current_machine)?;
            let mut fields = program
                .data_members(data)
                .iter()
                .filter_map(|member| match member {
                    DataMember::Field(field) if field.name.as_str() == receiver => Some(field),
                    _ => None,
                });
            let field = fields.next()?;
            if fields.next().is_some() {
                return None;
            }
            let receiver_type = boundary_receiver_type_symbol(program, field.type_reference)?;
            let definition = program
                .traits()
                .iter()
                .find(|definition| definition.symbol == receiver_type)?;
            (definition, true, field.type_reference)
        }
        _ => return None,
    };
    if !trait_definition.is_boundary
        || (!allow_type_parameters && !trait_definition.type_parameters.is_empty())
    {
        return None;
    }
    let bindings = super::type_instantiation::trait_receiver_type_bindings(
        program,
        trait_definition,
        receiver_reference,
    )?;
    let mut signatures = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| signature.name.as_str() == target);
    let signature = signatures.next()?;
    (signatures.next().is_none()
        && (signature.type_parameters.is_empty()
            || (allow_type_parameters
                && program
                    .state_signature_type_parameters(signature)
                    .iter()
                    .all(|parameter| {
                        parameter.symbol.is_valid()
                            && matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                    })))
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
    .then_some(BoundaryWriteSignature {
        signature,
        has_runtime_receiver,
        bindings,
    })
}

/// The call's retained receiver and target identities at this write site. A
/// statement call stores both symbols directly; a value call carries the
/// target and rejoins the receiver symbol from its receiver expression.
fn call_site_symbols(
    program: &TypedTrees,
    site: CallerWriteSite<'_>,
) -> Option<(SymbolHandle, SymbolHandle)> {
    match site {
        CallerWriteSite::Call(call) => Some((call.receiver_symbol, call.target_symbol)),
        CallerWriteSite::Expression(expression) => {
            let typed_trees::expression::ExpressionNode::Call(call) =
                program.expression_table.expression(expression)
            else {
                return None;
            };
            Some((
                expression_receiver_symbol(program, call.receiver),
                call.target_symbol,
            ))
        }
        CallerWriteSite::Statement(_) => None,
    }
}

/// The non-boundary trait requirement signature the checked call selected, or
/// None. `target_symbol` retains that selection exactly: its parent is the
/// declaring trait and its identity is the signature symbol, so no receiver
/// spelling is needed to pick the signature. Boundary traits stay on their
/// own rung, generic traits and signatures without closed `Type` binders stay
/// opaque, and a target that names a real machine state belongs to the
/// internal callee summary.
pub(super) fn requirement_signature_by_target(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<&typed_trees::signature::StateSignature> {
    if !target_symbol.is_valid()
        || program.symbols.get(target_symbol).kind != symbols::SymbolKind::State
        || super::machine_state_by_symbol(program, target_symbol).is_some()
    {
        return None;
    }
    let parent = program.symbols.get(target_symbol).parent;
    let mut definitions = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == parent);
    let definition = definitions.next()?;
    if definitions.next().is_some() {
        return None;
    }
    requirement_signature_in_trait(program, definition, |signature| {
        signature.symbol == target_symbol
            && program.symbols.name(target_symbol) == signature.name.as_str()
    })
}

/// The unique requirement `choose` selects on a non-boundary, non-generic
/// trait, or None. Signatures without closed `Type` binders stay opaque.
fn requirement_signature_in_trait<'program>(
    program: &'program TypedTrees,
    definition: &'program typed_trees::trait_definition::TraitDefinition,
    mut choose: impl FnMut(&typed_trees::signature::StateSignature) -> bool,
) -> Option<&'program typed_trees::signature::StateSignature> {
    if definition.is_boundary || !definition.type_parameters.is_empty() {
        return None;
    }
    let mut signatures = program
        .trait_machine_signatures(definition)
        .iter()
        .filter(|signature| choose(signature));
    let signature = signatures.next()?;
    if signatures.next().is_some() {
        return None;
    }
    (signature.type_parameters.is_empty()
        || program
            .state_signature_type_parameters(signature)
            .iter()
            .all(|parameter| {
                parameter.symbol.is_valid()
                    && matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
            }))
    .then_some(signature)
}

/// The requirement signature selected by an exact receiver place's declared
/// leaf type plus the target name, or None. The typer does not retain call
/// symbols on nested receiver paths (`self.group.handler.code()`), but the
/// leaf's declared `dyn`/trait type still pins the dispatch contract: every
/// member must resolve through concrete data fields, so the leaf's trait is
/// proven rather than spelled, and the requirement is then unique by name.
/// An indexed or non-place link never reaches this fallback — its receiver
/// chain was already rejected by the caller's exact-receiver gate.
fn requirement_signature_for_receiver_path<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    receiver: &[String],
    target: &str,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let [root, members @ ..] = receiver else {
        return None;
    };
    if root != "self" || members.is_empty() {
        return None;
    }
    let attached = current_machine.attached_data.as_ref()?;
    let mut data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == attached.as_str())?;
    for (depth, member) in members.iter().enumerate() {
        let field_type = program
            .data_members(data)
            .iter()
            .find_map(|member_node| match member_node {
                DataMember::Field(field) if field.name.as_str() == member.as_str() => {
                    Some(field.type_reference)
                }
                _ => None,
            })
            .and_then(|reference| live_unconstrained_type(program, reference))?;
        if depth + 1 == members.len() {
            let trait_symbol = receiver_type_symbol(program, field_type);
            let mut definitions = program
                .traits()
                .iter()
                .filter(|definition| definition.symbol == trait_symbol);
            let definition = definitions.next()?;
            if definitions.next().is_some() {
                return None;
            }
            return requirement_signature_in_trait(program, definition, |signature| {
                signature.name.as_str() == target
            });
        }
        data = program
            .data_definitions()
            .iter()
            .find(|data| data.symbol == receiver_type_symbol(program, field_type))?;
    }
    None
}

/// Does the signature's `self` parameter grant exclusive reach? A `&mut self`
/// requirement may write the whole runtime receiver object; a shared or
/// by-value `self` cannot publish a receiver write at all.
fn self_parameter_is_exclusive(
    program: &TypedTrees,
    signature: &typed_trees::signature::StateSignature,
) -> bool {
    program
        .state_signature_parameters(signature)
        .iter()
        .find(|parameter| parameter.is_self)
        .is_some_and(|parameter| {
            live_unconstrained_type(program, parameter.type_reference).is_some_and(|reference| {
                matches!(
                    program.type_reference_table.type_reference(reference),
                    TypeReferenceNode::Reference { access, .. } if access.is_exclusive()
                )
            })
        })
}

/// The resolved requirement signature plus how `self` reaches the callee, at
/// a call write site. Value-position calls rejoin the receiver symbol from
/// the receiver expression exactly as boundary selection does. When the
/// retained target symbol is absent — a nested receiver path the typer did
/// not annotate — the receiver place's declared leaf trait still selects
/// the requirement by name.
pub(super) fn requirement_signature_for_site<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    receiver: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<(&'program typed_trees::signature::StateSignature, bool, bool)> {
    let (receiver_symbol, target_symbol) = call_site_symbols(program, site)?;
    requirement_signature_and_self(
        program,
        current_machine,
        receiver,
        target,
        receiver_symbol,
        target_symbol,
    )
}

/// The resolved requirement signature plus how `self` reaches the callee. A
/// declaration qualifier (`Issuer::issue(x)`, `Receipt::ack(v)`) is not a
/// runtime place: the callee's `self` then arrives as an ordinary argument,
/// exactly as the resolved-target validation rung pairs them.
fn requirement_signature_and_self<'program>(
    program: &'program TypedTrees,
    current_machine: &Machine,
    receiver: &[String],
    target: &str,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
) -> Option<(&'program typed_trees::signature::StateSignature, bool, bool)> {
    // The retained target is authoritative evidence: a valid symbol must name
    // the selected signature, so a stale or foreign selection fails closed
    // here even when the receiver's declared trait offers a same-named
    // requirement. Only an absent annotation — a nested receiver path the
    // typer does not decorate — falls back to the receiver's declared leaf
    // type.
    let signature = requirement_signature_by_target(program, target_symbol).or_else(|| {
        (!target_symbol.is_valid())
            .then(|| {
                requirement_signature_for_receiver_path(program, current_machine, receiver, target)
            })
            .flatten()
    })?;
    let self_is_argument = matches!(
        program.symbols.get(receiver_symbol).kind,
        symbols::SymbolKind::BuiltinType
            | symbols::SymbolKind::Data
            | symbols::SymbolKind::Domain
            | symbols::SymbolKind::Machine
            | symbols::SymbolKind::Module
            | symbols::SymbolKind::Trait
            | symbols::SymbolKind::ConformanceParameter
    );
    Some((
        signature,
        self_is_argument,
        self_parameter_is_exclusive(program, signature),
    ))
}

/// The exact frame of a resolved non-boundary trait requirement call. The
/// implementing body is selected at runtime, so the signature's declared
/// reach is the complete caller-visible evidence: an exclusive `self` writes
/// the receiver place (or the explicit `self` argument on a declaration-
/// qualified call such as `Issuer::issue(self.issuer)`), and every exclusive
/// parameter writes its argument's proven origins. A caller cannot supply a
/// path for a receiver expression that names no place; such writes stay
/// inside the temporary and contribute nothing.
pub(super) fn known_requirement_call_written_paths_for_parts(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    receiver_origin: Option<&FramePlaceOrigin>,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let (signature, self_is_argument, self_exclusive) =
        requirement_signature_for_site(program, current_machine, receiver, target, site)?;
    let parameters = program.state_signature_parameters(signature);
    let paired = parameters
        .iter()
        .filter(|parameter| self_is_argument || !parameter.is_self)
        .collect::<Vec<_>>();
    if paired.len() != arguments.len() {
        return None;
    }
    let mut written = Vec::new();
    if !self_is_argument && self_exclusive {
        let path = receiver_origin
            .map(|origin| origin.path.clone())
            .or_else(|| (!receiver.is_empty()).then(|| receiver.join(".")))
            .unwrap_or_else(|| "self".to_owned());
        written.push(path);
    }
    let bindings = super::type_instantiation::signature_call_type_bindings_with_self(
        program,
        current_machine,
        signature,
        site,
        arguments,
        self_is_argument,
    )?;
    for (parameter, argument) in paired.into_iter().zip(arguments) {
        if parameter.is_self {
            if self_exclusive {
                let path = coarse_place_path(program, *argument)?;
                if !written.contains(&path) {
                    written.push(path);
                }
            }
            continue;
        }
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
        let parameter_type = substituted_head(program, parameter_type, &bindings);
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            if !matches!(
                program.type_reference_table.type_reference(parameter_type),
                TypeReferenceNode::Unit
            ) && !type_is_caller_isolated_local_in(program, parameter_type, &bindings)
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
        if !referent_has_only_owned_storage_in(program, *referee, &bindings) {
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

pub(super) fn receiver_type_symbol(
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

pub(super) fn expression_receiver_symbol(
    program: &TypedTrees,
    receiver: ExpressionHandle,
) -> SymbolHandle {
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
    let selected = boundary_write_signature_and_receiver(
        program,
        current_machine,
        machine_symbols,
        symbols,
        receiver,
        target,
        site,
    )?;
    let BoundaryWriteSignature {
        signature,
        has_runtime_receiver,
        bindings,
    } = selected;
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
    let bindings = super::type_instantiation::signature_call_type_bindings_seeded(
        program,
        current_machine,
        signature,
        site,
        arguments,
        bindings,
    )?;

    for (parameter, argument) in parameters.into_iter().zip(arguments) {
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
        let parameter_type = substituted_head(program, parameter_type, &bindings);
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            if !matches!(
                program.type_reference_table.type_reference(parameter_type),
                TypeReferenceNode::Unit
            ) && !type_is_caller_isolated_local_in(program, parameter_type, &bindings)
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
        if !referent_has_only_owned_storage_in(program, *referee, &bindings) {
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
    if let ExpressionNode::Call(call) = program.expression_table.expression(argument) {
        if selected_boundary_signature(program, call.target_symbol) {
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
        if requirement_signature_by_target(program, call.target_symbol).is_some() {
            return requirement_result_origins(
                program,
                current_machine,
                machine_symbols,
                symbols,
                call,
                argument,
                inference,
            );
        }
    }
    exclusive_reference_origins(program, current_machine, argument, symbols, inference)
}

/// The candidate caller-storage origins a boundary call's exclusive result
/// may alias. There is no inspectable body: a `&mut`/`&write` result can
/// reach the runtime receiver's opaque storage or the storage behind an
/// exclusive argument whose own storage may hold the referent. Shared
/// references cannot lend exclusive reach. A by-value or exclusive carrier
/// whose stored exclusive references could still reach the referent — and
/// therefore route the result into untracked storage — keeps the whole
/// result opaque, as does a result with no admitted caller route at all.
///
/// An origin's precision says whether the referent is that place exactly or
/// an unknown position inside it. Only the single-candidate consumer in
/// `single_boundary_result_origin` reads the distinction; write-path
/// consumers read `path` alone, where a storage root already covers every
/// interior subpath.
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
    let selected = boundary_write_signature_and_receiver(
        program,
        current_machine,
        machine_symbols,
        symbols,
        &receiver,
        call.target.as_str(),
        CallerWriteSite::Expression(expression),
    )?;
    let BoundaryWriteSignature {
        signature,
        has_runtime_receiver,
        bindings,
    } = selected;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut bindings = super::type_instantiation::signature_call_type_bindings_seeded(
        program,
        current_machine,
        signature,
        CallerWriteSite::Expression(expression),
        arguments,
        bindings,
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
    let referent =
        live_unconstrained_type(program, substituted_head(program, *referee, &bindings))?;

    let mut origins = Vec::new();
    if has_runtime_receiver {
        // The implementor's storage is opaque to the caller: an exclusive
        // result may point anywhere inside it. The receiver place covers
        // every such subpath, but a member projection must not narrow it to
        // a fabricated subpath, so the candidate stays collection-coarse.
        origins.push(FramePlaceOrigin {
            path: receiver.join("."),
            precision: FramePathPrecision::CollectionCoarse,
            source: FrameSourcePlace::from_expression(program, call.receiver),
        });
    }
    let parameters = program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != arguments.len() {
        return None;
    }
    for (parameter, actual) in parameters.into_iter().zip(arguments) {
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
        let parameter_type = substituted_head(program, parameter_type, &bindings);
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            // A by-value carrier can still store exclusive references whose
            // referents this frame cannot name; its route stays opaque.
            if type_may_carry_write_in(program, parameter_type, &bindings) {
                return None;
            }
            continue;
        };
        if !access.is_exclusive() {
            continue;
        }
        match owned_storage_may_hold(program, *referee, referent, &mut bindings) {
            Some(true) => {
                // The admitted route places name the storage the result may
                // reach. Unless that storage can hold the referent only at
                // its root, the referent's offset inside stays unknown and a
                // projected origin must not narrow beneath the root.
                let root_only =
                    storage_holds_referent_only_at_root(program, *referee, referent, &mut bindings);
                for origin in boundary_argument_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    *actual,
                    inference,
                )? {
                    push_unique_origin(
                        &mut origins,
                        if root_only {
                            origin
                        } else {
                            FramePlaceOrigin {
                                precision: FramePathPrecision::CollectionCoarse,
                                ..origin
                            }
                        },
                    );
                }
            }
            Some(false) => {}
            None => return None,
        }
    }
    (!origins.is_empty()).then_some(origins)
}

/// The signature's admitted candidate routes canonicalized through the
/// caller's current alias and stored-carrier evidence, kept as the exact
/// finite union. Union consumers (nested-call arguments) keep every proven
/// route; single-binding consumers collapse it through `single_place_origin`.
#[allow(clippy::too_many_arguments)]
pub(super) fn boundary_result_candidate_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCallExpression,
    expression: ExpressionHandle,
    inference: &mut FrameInference,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    canonical_result_origins(
        boundary_result_origins(
            program,
            current_machine,
            machine_symbols,
            symbols,
            call,
            expression,
            inference,
        )?,
        parameters,
        isolated_local_roots,
        aliases,
        allow_isolated_local,
        stored,
    )
}

/// The candidate caller-storage origins a resolved requirement call's
/// exclusive result may alias — the same admitted routes as a boundary
/// result, with the signature selected by retained `target_symbol` rather
/// than receiver spelling. An exclusive `self` contributes the whole runtime
/// receiver storage as a coarse candidate, whether it arrives as a place
/// receiver or as the explicit `self` argument on a declaration-qualified
/// call.
fn requirement_result_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCallExpression,
    expression: ExpressionHandle,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let receiver = receiver_member_chain(program, call.receiver).unwrap_or_default();
    let (signature, self_is_argument, self_exclusive) = requirement_signature_for_site(
        program,
        current_machine,
        &receiver,
        call.target.as_str(),
        CallerWriteSite::Expression(expression),
    )?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let parameters = program.state_signature_parameters(signature);
    let paired = parameters
        .iter()
        .filter(|parameter| self_is_argument || !parameter.is_self)
        .collect::<Vec<_>>();
    if paired.len() != arguments.len() {
        return None;
    }
    let mut bindings = super::type_instantiation::signature_call_type_bindings_with_self(
        program,
        current_machine,
        signature,
        CallerWriteSite::Expression(expression),
        arguments,
        self_is_argument,
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
    let referent =
        live_unconstrained_type(program, substituted_head(program, *referee, &bindings))?;

    let mut origins = Vec::new();
    if self_exclusive {
        // The implementor's storage is opaque to the caller: an exclusive
        // result may point anywhere inside it. The receiver place covers
        // every such subpath, but a member projection must not narrow it to
        // a fabricated subpath, so the candidate stays collection-coarse.
        let (path, source) = if self_is_argument {
            let actual = parameters
                .iter()
                .position(|parameter| parameter.is_self)
                .and_then(|index| arguments.get(index))?;
            (
                coarse_place_path(program, *actual)?,
                FrameSourcePlace::from_expression(program, *actual),
            )
        } else {
            (
                receiver.join("."),
                FrameSourcePlace::from_expression(program, call.receiver),
            )
        };
        origins.push(FramePlaceOrigin {
            path,
            precision: FramePathPrecision::CollectionCoarse,
            source,
        });
    }
    for (parameter, actual) in paired.into_iter().zip(arguments) {
        if parameter.is_self {
            continue;
        }
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
        let parameter_type = substituted_head(program, parameter_type, &bindings);
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            // A by-value carrier can still store exclusive references whose
            // referents this frame cannot name; its route stays opaque.
            if type_may_carry_write_in(program, parameter_type, &bindings) {
                return None;
            }
            continue;
        };
        if !access.is_exclusive() {
            continue;
        }
        match owned_storage_may_hold(program, *referee, referent, &mut bindings) {
            Some(true) => {
                // The admitted route places name the storage the result may
                // reach. Unless that storage can hold the referent only at
                // its root, the referent's offset inside stays unknown and a
                // projected origin must not narrow beneath the root.
                let root_only =
                    storage_holds_referent_only_at_root(program, *referee, referent, &mut bindings);
                for origin in boundary_argument_origins(
                    program,
                    current_machine,
                    machine_symbols,
                    symbols,
                    *actual,
                    inference,
                )? {
                    push_unique_origin(
                        &mut origins,
                        if root_only {
                            origin
                        } else {
                            FramePlaceOrigin {
                                precision: FramePathPrecision::CollectionCoarse,
                                ..origin
                            }
                        },
                    );
                }
            }
            Some(false) => {}
            None => return None,
        }
    }
    (!origins.is_empty()).then_some(origins)
}

/// The requirement signature's admitted candidate routes canonicalized into
/// the caller's alias and stored-carrier namespace, kept as the exact finite
/// union for the same shared consumers as `boundary_result_candidate_origins`.
#[allow(clippy::too_many_arguments)]
pub(super) fn requirement_result_candidate_origins(
    program: &TypedTrees,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCallExpression,
    expression: ExpressionHandle,
    inference: &mut FrameInference,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    canonical_result_origins(
        requirement_result_origins(
            program,
            current_machine,
            machine_symbols,
            symbols,
            call,
            expression,
            inference,
        )?,
        parameters,
        isolated_local_roots,
        aliases,
        allow_isolated_local,
        stored,
    )
}

/// Canonicalize admitted candidate origins through the caller's current
/// alias and stored-carrier evidence. A candidate no caller storage,
/// established alias, isolated local, or tracked stored carrier owns is not a
/// proven route and fails the whole result.
fn canonical_result_origins(
    origins: Vec<FramePlaceOrigin>,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<Vec<FramePlaceOrigin>> {
    let mut canonical = Vec::new();
    for candidate in origins {
        push_unique_origin(
            &mut canonical,
            caller_canonical_result_origin(
                candidate,
                parameters,
                isolated_local_roots,
                aliases,
                allow_isolated_local,
                stored,
            )?,
        );
    }
    Some(canonical)
}

/// Canonicalize one admitted candidate path through the caller's current
/// alias and stored-carrier evidence. `boundary_result_origins` answers in
/// caller spellings — a binding argument names its local, and a carrier
/// leaf names its symbolic local path — while the alias relation stores
/// canonical origins, so a bound local must keep the referent the argument
/// holds at bind time even when that binding is later rebound. A candidate
/// whose root no caller storage, established alias, isolated local, or
/// tracked stored carrier owns is not a proven single origin.
fn caller_canonical_result_origin(
    candidate: FramePlaceOrigin,
    parameters: &[StateParameter],
    isolated_local_roots: &[String],
    aliases: &[(String, FramePlaceOrigin)],
    allow_isolated_local: bool,
    stored: &[StoredLocalOrigins],
) -> Option<FramePlaceOrigin> {
    let mut origin = candidate;
    for _ in 0..=aliases.len() {
        let (root, suffix) = split_place_root(&origin.path);
        if root == "self"
            || parameters
                .iter()
                .any(|parameter| parameter.name.as_str() == root)
        {
            return Some(origin);
        }
        if let Some((_, prior)) = aliases.iter().find(|(name, _)| name == root) {
            let source = prior.source.append_source(&origin.source);
            origin = match prior.precision {
                FramePathPrecision::Exact => FramePlaceOrigin {
                    path: append_place_suffix(&prior.path, suffix),
                    precision: origin.precision,
                    source,
                },
                FramePathPrecision::CollectionCoarse => FramePlaceOrigin {
                    path: prior.path.clone(),
                    precision: FramePathPrecision::CollectionCoarse,
                    source,
                },
            };
            continue;
        }
        return (allow_isolated_local && isolated_local_roots.iter().any(|local| local == root)
            || stored
                .iter()
                .any(|local| local.local_symbol == origin.source.root))
        .then_some(origin);
    }
    None
}

/// May `container`'s own storage hold a `referent`-typed value? The walk
/// follows declared fields and elements only, never a reference's own
/// storage: `Some(true)` finds the referent, `Some(false)` rules it out, and
/// `None` means a stored exclusive reference or an unfinished proof could
/// still reach one, so the callee may route the result into storage this
/// frame cannot name. An applied generic carrier is inspected under its own
/// argument bindings; an unbound parameter keeps the named leaf.
fn owned_storage_may_hold(
    program: &TypedTrees,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    bindings: &mut TypeBindings,
) -> Option<bool> {
    let referent = substituted_head(program, referent, bindings);
    owned_storage_may_hold_inner(program, container, referent, &mut Vec::new(), bindings)
}

/// `visiting` records the *instantiated* containers already on the path, so
/// `Wrap<Wrap<u64>>` — a finite shape — is not mistaken for the genuine
/// recursion a self-referential instantiation produces.
fn owned_storage_may_hold_inner(
    program: &TypedTrees,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    visiting: &mut Vec<TypeReferenceHandle>,
    bindings: &mut TypeBindings,
) -> Option<bool> {
    let container =
        live_unconstrained_type(program, substituted_head(program, container, bindings))?;
    let referent = substituted_head(program, referent, bindings);
    if aggregate_storage_types_match_in(program, container, referent, bindings) {
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
                && owned_storage_may_hold_inner(program, *referee, referent, visiting, bindings)
                    != Some(false)
            {
                return None;
            }
            Some(false)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            owned_storage_may_hold_inner(program, *element_type, referent, visiting, bindings)
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
            if visiting.iter().any(|visited| {
                aggregate_storage_types_match_in(program, *visited, container, bindings)
            }) {
                // A recursive instantiation cannot finish the proof; a
                // stored exclusive link in the cycle may still reach the
                // referent.
                return None;
            }
            // An applied generic carrier binds its `Type` parameters to the
            // supplied arguments for the member walk; an application that
            // cannot resolve cannot prove its fields.
            let mark = bindings.len();
            let applied = match program.type_reference_table.type_reference(container) {
                TypeReferenceNode::Generic { arguments, .. } => {
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .to_vec();
                    push_generic_application_bindings(program, *symbol, &arguments, bindings)
                        .is_some()
                }
                _ => true,
            };
            if !applied {
                return None;
            }
            visiting.push(container);
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
                    match owned_storage_may_hold_inner(
                        program, field_type, referent, visiting, bindings,
                    ) {
                        Some(true) => found = true,
                        Some(false) => {}
                        None => {
                            visiting.pop();
                            bindings.truncate(mark);
                            return None;
                        }
                    }
                }
            }
            visiting.pop();
            bindings.truncate(mark);
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

/// Is `container`'s root the only position inside its declared storage that
/// can hold a `referent`-typed value? When true, an exclusive result routed
/// to this storage points at the root itself, so member projections of the
/// bound local stay exact. Any admitted deeper position — or a proof that
/// cannot finish — leaves the referent offset unknown.
fn storage_holds_referent_only_at_root(
    program: &TypedTrees,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    bindings: &mut TypeBindings,
) -> bool {
    let Some(container) =
        live_unconstrained_type(program, substituted_head(program, container, bindings))
    else {
        return false;
    };
    aggregate_storage_types_match_in(program, container, referent, bindings)
        && storage_member_may_hold(program, container, referent, &mut Vec::new(), bindings)
            == Some(false)
}

/// `Some` answers whether a proper member or element position inside
/// `container`'s declared storage may hold `referent`; `None` means the
/// proof cannot finish. `container` already matched `referent` at its root,
/// so recursion only ever inspects strictly interior positions.
fn storage_member_may_hold(
    program: &TypedTrees,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    visiting: &mut Vec<TypeReferenceHandle>,
    bindings: &mut TypeBindings,
) -> Option<bool> {
    let container = substituted_head(program, container, bindings);
    if program.primitive_type_reference(container).is_some() {
        return Some(false);
    }
    match program.type_reference_table.type_reference(container) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => {
            owned_storage_may_hold_inner(program, *element_type, referent, visiting, bindings)
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
            let definition = definitions.next()?;
            if definitions.next().is_some()
                || visiting.iter().any(|visited| {
                    aggregate_storage_types_match_in(program, *visited, container, bindings)
                })
            {
                // An ambiguous nominal cannot be inspected, and a recursive
                // instantiation cannot prove its interior excludes the
                // referent.
                return Some(true);
            }
            // An applied generic carrier binds its `Type` parameters to the
            // supplied arguments for the member walk; an application that
            // cannot resolve cannot prove its interior excludes the referent.
            let mark = bindings.len();
            let applied = match program.type_reference_table.type_reference(container) {
                TypeReferenceNode::Generic { arguments, .. } => {
                    let arguments = program
                        .type_reference_table
                        .type_reference_handles(*arguments)
                        .to_vec();
                    push_generic_application_bindings(program, *symbol, &arguments, bindings)
                        .is_some()
                }
                _ => true,
            };
            if !applied {
                return Some(true);
            }
            visiting.push(container);
            let result = (|| {
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
                        match owned_storage_may_hold_inner(
                            program, field_type, referent, visiting, bindings,
                        ) {
                            Some(false) => {}
                            outcome => return outcome.map(|_| true),
                        }
                    }
                }
                Some(false)
            })();
            visiting.pop();
            bindings.truncate(mark);
            result
        }
        // Provider-opaque and proof-static storage cannot prove their
        // interior excludes the referent. Constrained nodes are already
        // erased by `live_unconstrained_type`; a reference slot stores the
        // reference, never the referent value.
        TypeReferenceNode::DynamicTrait { .. } | TypeReferenceNode::ConstExpression(_) => {
            Some(true)
        }
        TypeReferenceNode::Reference { .. }
        | TypeReferenceNode::Unit
        | TypeReferenceNode::Constrained { .. } => Some(false),
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
