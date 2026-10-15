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

use super::caller_aliases::{CallerWriteSite, caller_statement_owner};
use super::isolation::{
    TypeIdentityMemo, aggregate_storage_types_match_in, type_is_caller_isolated_local_in,
};
use super::place_paths::{
    FramePathPrecision, FramePlaceOrigin, FrameSourcePlace, append_place_suffix,
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
use language_core::is_self_receiver;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::statement::TableCall;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// Recognition is deliberately broader than signature selection: even an
/// invalid prefix or ambiguous member on a cached trait receiver must not
/// regain a complete frame through the signature-free fallback.
pub(super) fn receiver_requires_boundary_frame(
    program: &dyn crate::CheckingInput,
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
        [root, member] if is_self_receiver(root) => {
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
/// ensures seeding the value environment). Arbitrary prefixes cannot select a cached
/// field, and a sibling state's same-named parameter supplies no authority.
pub(crate) fn boundary_trait_signature<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    call: &TableCall,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let receiver_members =
        crate::machine_calls::calls::signature_queries::call_receiver_member_names(program, call);
    boundary_trait_signature_for_parts(
        program,
        current_machine,
        &receiver_members,
        call.target.as_str(),
        CallerWriteSite::Call(call),
    )
}

/// Statement-call validation's selection: the same exact receiver/target
/// join as `boundary_trait_signature`, additionally admitting a requirement
/// whose generic telescope is entirely `machine` binders. Such a binder is a
/// compile-time selection, never a runtime formal: MP2b admits the call's
/// static machine argument against its bound at this same call edge, and a
/// `native callback name from Binder` destination takes its selection from
/// that argument (private_callbacks.md, "Authored selection"). The caller
/// still validates result use and every runtime argument against the
/// selected signature. `Type` binders stay excluded: validation has no
/// substitution for their formals, and fact seeding (the monomorphic
/// `boundary_trait_signature`) must not read uninstantiated contracts.
pub(crate) fn boundary_trait_call_signature<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    call: &TableCall,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let receiver_members =
        crate::machine_calls::calls::signature_queries::call_receiver_member_names(program, call);
    boundary_trait_signature_and_receiver_inner(
        program,
        current_machine,
        &receiver_members,
        call.target.as_str(),
        CallerWriteSite::Call(call),
        SignatureBinders::StaticMachines,
    )
    .map(|selected| selected.signature)
}

/// Value-position boundary calls select the same declaration and admitted
/// telescope as statement calls. Selection supplies formals, not permission
/// to omit their argument checks or infer authority from a member spelling.
pub(crate) fn boundary_trait_expression_call_signature<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    expression: ExpressionHandle,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let call = program.expression_table.call_node(expression)?;
    let receiver_members = receiver_member_chain(program, call.receiver).unwrap_or_default();
    boundary_trait_signature_and_receiver_inner(
        program,
        current_machine,
        &receiver_members,
        call.target.as_str(),
        CallerWriteSite::Expression(expression),
        SignatureBinders::StaticMachines,
    )
    .map(|selected| selected.signature)
}

pub(super) fn boundary_trait_signature_for_parts<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<&'program typed_trees::signature::StateSignature> {
    boundary_trait_signature_and_receiver(program, current_machine, receiver_members, target, site)
        .map(|(signature, _)| signature)
}

/// The Boolean records actual runtime receiver storage, not a trait qualifier.
/// Both static and receiver calls keep the same exact signature selection and
/// exclusive-argument origin rules.
fn boundary_trait_signature_and_receiver<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<(&'program typed_trees::signature::StateSignature, bool)> {
    boundary_trait_signature_and_receiver_inner(
        program,
        current_machine,
        receiver_members,
        target,
        site,
        SignatureBinders::Monomorphic,
    )
    .map(|selected| (selected.signature, selected.has_runtime_receiver))
}

/// Which generic binders of a selected requirement a route can discharge.
/// Selection identity is shared; only the admitted telescope differs.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SignatureBinders {
    /// Fact seeding and frame lookup read formals as written.
    Monomorphic,
    /// Write frames substitute `Type` binders the actual arguments pin.
    ArgumentPinnedTypes,
    /// Call validation admits `machine` binders, which no runtime argument
    /// supplies (see `boundary_trait_call_signature`).
    StaticMachines,
}

struct BoundaryWriteSignature<'program> {
    signature: &'program typed_trees::signature::StateSignature,
    /// Caller-visible provider-object storage, not an authenticated service
    /// binding that routes to independently established provider storage.
    has_runtime_receiver: bool,
    bindings: TypeBindings,
}

/// The exact selected boundary can consume a routing authority without lending
/// provider-object storage. Operand evaluation still contributes its own writes;
/// only the receiver storage footprint is absent.
pub(super) fn call_has_authority_receiver(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    site: CallerWriteSite<'_>,
) -> bool {
    let (members, target) = match site {
        CallerWriteSite::Expression(expression) => {
            let Some(call) = program.expression_table.call_node(expression) else {
                return false;
            };
            if !call.receiver.is_valid() {
                return false;
            }
            (
                receiver_member_chain(program, call.receiver).unwrap_or_default(),
                call.target.as_str(),
            )
        }
        CallerWriteSite::Call(call) => (
            crate::machine_calls::calls::signature_queries::call_receiver_member_names(
                program, call,
            ),
            call.target.as_str(),
        ),
        _ => return false,
    };
    boundary_trait_signature_and_receiver_inner(
        program,
        machine,
        &members,
        target,
        site,
        SignatureBinders::ArgumentPinnedTypes,
    )
    .is_some_and(|signature| !signature.has_runtime_receiver)
}

/// Resolve a receiver head without discarding its application arguments:
/// callers retain the original reference for exact owner substitution.
fn boundary_receiver_type_symbol(
    program: &dyn crate::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    for _ in 0..program.type_reference_table.type_reference_count() {
        // Validate the constraint chain before the service classifier walks it.
        let unconstrained = live_unconstrained_type(program, reference)?;
        // A provisioned service calls its exact boundary requirement, not the
        // opaque carrier's data owner. Classify before peeling qualifications:
        // an invalid Service shell must not acquire a complete write frame.
        if let Some(carrier) =
            typed_trees::service::inspect_service_carrier_contract(program, reference).ok()?
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
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<BoundaryWriteSignature<'program>> {
    boundary_trait_signature_and_receiver_inner(
        program,
        current_machine,
        receiver_members,
        target,
        site,
        SignatureBinders::ArgumentPinnedTypes,
    )
}

fn boundary_trait_signature_and_receiver_inner<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver_members: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
    binders: SignatureBinders,
) -> Option<BoundaryWriteSignature<'program>> {
    let (receiver_symbol, target_symbol) = call_site_symbols(program, site)?;
    let (state, _, before) = caller_statement_owner(program, current_machine, site)?;
    let reference = match site {
        CallerWriteSite::Call(call) => {
            crate::receiver_projections::projected_statement_receiver_type_reference(
                program, state, before, call,
            )
        }
        CallerWriteSite::Expression(expression) => {
            let call = program.expression_table.call_node(expression)?;
            crate::receiver_projections::expression(
                program,
                state,
                before,
                call.receiver,
                &mut Vec::new(),
            )
            .ok()?
            .map(|projection| projection.type_reference)
            .or_else(|| {
                crate::declared_place_type_raw(program, current_machine, Some(state), call.receiver)
            })
        }
        CallerWriteSite::Transition(_) => parameter_boundary_receiver_type(
            program,
            current_machine,
            receiver_members,
            receiver_symbol,
            site,
        ),
        CallerWriteSite::Statement(_) => None,
    };
    let (trait_definition, has_runtime_receiver, receiver_reference) =
        if let Some(reference) = reference {
            let receiver_type = boundary_receiver_type_symbol(program, reference)?;
            (
                program.trait_definition_by_symbol(receiver_type)?,
                true,
                reference,
            )
        } else {
            // Only an exact declaration identity supplies a namespace. Runtime
            // roots are rejoined above through their complete scoped projection.
            (
                program.trait_definition_by_symbol(receiver_symbol)?,
                false,
                TypeReferenceHandle::invalid(),
            )
        };
    if !trait_definition.is_boundary {
        return None;
    }
    // `Requirement<Arguments>::operation(..)` names its exact application in
    // the call's qualifier; a bare declaration namespace is the trait's
    // abstract scope. Typing retargeted the call to the closed application
    // of that qualifier, so the scope must come from the same qualifier or
    // the closed target matches nothing and the call escapes its argument
    // checks.
    let applied = match site {
        CallerWriteSite::Expression(expression) => program
            .expression_table
            .call_node(expression)
            .map(|call| call.requirement_application)
            .filter(|application| application.is_valid()),
        _ => None,
    };
    let receiver_scope = if has_runtime_receiver {
        crate::declarations::traits::receiver_trait_scope(
            program,
            receiver_reference,
            current_machine.symbol,
        )?
    } else if let Some(application) = applied {
        program.applied_requirement_scope(current_machine.symbol, application)?
    } else {
        typed_trees::trait_definition::AppliedTraitScope::abstract_scope(trait_definition)
    };
    let template = program
        .closed_requirement_application(target_symbol)
        .map_or(target_symbol, |application| application.template);
    let (trait_definition, _) = program.trait_machine_signature(template)?;
    receiver_scope.declaring_scope(program, trait_definition.symbol)?;
    if program
        .closed_requirement_application(target_symbol)
        .is_some()
        && !program.requirement_application_matches_scope(target_symbol, &receiver_scope)
    {
        return None;
    }
    let bindings = super::type_instantiation::trait_receiver_type_bindings(
        program,
        trait_definition,
        target_symbol,
        receiver_reference,
    )?;
    let mut signatures = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| signature.name.as_str() == target);
    let declaration = signatures.next()?;
    if signatures.next().is_some() {
        return None;
    }
    let signature = program.trait_machine_signature_by_symbol(trait_definition, target_symbol)?;
    if !trait_definition.type_parameters.is_empty()
        && !program
            .closed_requirement_application(signature.symbol)
            .is_some_and(|application| {
                if applied.is_some() {
                    return program.requirement_application_matches_scope(
                        application.signature.symbol,
                        &receiver_scope,
                    );
                }
                typed_trees::service::inspect_service_parameter_contract(
                    program,
                    receiver_reference,
                )
                .ok()
                .flatten()
                .and_then(|carrier| {
                    carrier
                        .carrier
                        .application(program, current_machine.symbol, &[])
                })
                .is_some_and(|scope| {
                    program
                        .requirement_application_matches_scope(application.signature.symbol, &scope)
                })
            })
        && binders != SignatureBinders::ArgumentPinnedTypes
    {
        return None;
    }
    // Executable substitution changes the signature symbol, never which
    // authored receiver member the call selected.
    if signature.symbol != declaration.symbol
        && !program
            .closed_requirement_application(signature.symbol)
            .is_some_and(|application| application.template == declaration.symbol)
    {
        return None;
    }
    let admitted_binder = |parameter: &typed_trees::data::TypeParameter| {
        parameter.symbol.is_valid()
            && match binders {
                SignatureBinders::Monomorphic => false,
                SignatureBinders::ArgumentPinnedTypes => {
                    matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
                }
                SignatureBinders::StaticMachines => {
                    matches!(
                        parameter.kind,
                        typed_trees::data::TypeParameterKind::Machine { .. }
                    )
                }
            }
    };
    let exact_signature = program
        .state_signature_type_parameters(signature)
        .iter()
        .all(admitted_binder)
        && target_symbol.is_valid()
        && signature.symbol == target_symbol
        && (has_runtime_receiver
            || (program.symbols.get(declaration.symbol).kind == symbols::SymbolKind::State
                && program.symbols.get(declaration.symbol).parent == trait_definition.symbol
                && program.symbols.name(declaration.symbol) == signature.name.as_str()));
    if !exact_signature {
        return None;
    }
    // Selection above still requires the exact receiver occurrence and target.
    // A compiler-known Binding lends routing authority, not its carrier as
    // provider-object storage. Neither a provider write nor an exclusive result
    // may fabricate an origin inside that authority carrier. Explicit argument
    // origins remain governed by the selected signature's ordinary rules.
    let binding_receiver = if has_runtime_receiver {
        let carrier =
            typed_trees::service::inspect_service_parameter_contract(program, receiver_reference)
                .ok()?;
        carrier.is_some()
    } else {
        false
    };
    Some(BoundaryWriteSignature {
        signature,
        has_runtime_receiver: has_runtime_receiver && !binding_receiver,
        bindings,
    })
}

/// A static machine's lexical owner supplies no implicit receiver. Resolve
/// parameter projections from the call's retained root and field identities.
fn parameter_boundary_receiver_type(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    members: &[String],
    leaf: SymbolHandle,
    site: CallerWriteSite<'_>,
) -> Option<TypeReferenceHandle> {
    let (state, _, _) = caller_statement_owner(program, machine, site)?;
    let root = match site {
        CallerWriteSite::Call(call) => call.receiver_root_symbol,
        CallerWriteSite::Expression(expression) => {
            let call = program.expression_table.call_node(expression)?;
            let mut receiver = call.receiver;
            let mut root = SymbolHandle::invalid();
            for _ in 0..members.len() {
                match program.expression_table.expression(receiver) {
                    ExpressionNode::Member(member) => receiver = member.receiver,
                    ExpressionNode::Name(name) => {
                        root = name.head_symbol;
                        break;
                    }
                    _ => return None,
                }
            }
            root
        }
        CallerWriteSite::Transition(target) => {
            let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(target)
            else {
                return None;
            };
            path.head_symbol
        }
        CallerWriteSite::Statement(_) => return None,
    };
    let (name, fields) = members.split_first()?;
    let parameter = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root && parameter.name.as_str() == name)?;
    if !root.is_valid() || !leaf.is_valid() || fields.is_empty() {
        return None;
    }
    super::reference_origins::retained_receiver_projection(
        program,
        parameter.type_reference,
        root,
        leaf,
        fields.iter().map(String::as_str),
    )
    .map(|(_, reference)| reference)
}

/// The call's retained receiver and target identities at this write site. A
/// statement call stores both symbols directly; a value call carries the
/// target and rejoins the receiver symbol from its receiver expression.
fn call_site_symbols(
    program: &dyn crate::CheckingInput,
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
        CallerWriteSite::Transition(target) => {
            let typed_trees::statement::TransitionTargetNode::Named { path, .. } =
                program.statement_table.transition_target(target)
            else {
                return None;
            };
            Some((path.head_symbol, path.symbol))
        }
        CallerWriteSite::Statement(_) => None,
    }
}

/// The non-boundary trait requirement signature the checked call selected, or
/// None. The shared selection query rejoins trait-owned signatures and evidence
/// children with their applied contracts. A selector is not runtime storage.
/// Boundary traits stay on their own route; a raw generic trait signature
/// without an application stays opaque. A real machine state belongs to its
/// internal callee summary.
pub(super) fn requirement_signature_by_target(
    program: &dyn crate::CheckingInput,
    target_symbol: SymbolHandle,
) -> Option<&typed_trees::signature::StateSignature> {
    if !target_symbol.is_valid()
        || program.symbols.get(target_symbol).kind != symbols::SymbolKind::State
        || super::machine_state_by_symbol(program, target_symbol).is_some()
    {
        return None;
    }
    let (definition, signature) = crate::selected_requirement_signature(program, target_symbol)?;
    if definition.is_boundary
        || (!definition.type_parameters.is_empty()
            && signature.symbol == target_symbol
            && program
                .closed_requirement_application(target_symbol)
                .is_none())
    {
        return None;
    }
    program
        .state_signature_type_parameters(signature)
        .iter()
        .all(|parameter| {
            parameter.symbol.is_valid()
                && matches!(parameter.kind, typed_trees::data::TypeParameterKind::Type)
        })
        .then_some(signature)
}

/// The unique requirement `choose` selects on a non-boundary, non-generic
/// trait, or None. Signatures without closed `Type` binders stay opaque.
fn requirement_signature_in_trait<'program>(
    program: &'program dyn crate::CheckingInput,
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
/// The written paths every admitted exclusive argument contributes, shared
/// by the boundary frame and the resolved-signature frame: a by-value
/// carrier that can still contain mutable references refuses the frame
/// rather than manufacture a receiver-only approximation.
#[allow(clippy::too_many_arguments)]
fn boundary_written_argument_paths(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    parameters: Vec<&StateParameter>,
    arguments: &[ExpressionHandle],
    written: &mut Vec<String>,
    bindings: &[(SymbolHandle, TypeReferenceHandle)],
    inference: &mut FrameInference,
) -> Option<()> {
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

    Some(())
}

fn requirement_signature_for_receiver_path<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver: &[String],
    target: &str,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let [root, members @ ..] = receiver else {
        return None;
    };
    if !is_self_receiver(root) || members.is_empty() {
        return None;
    }
    let attached = current_machine.attached_data.as_ref()?;
    let mut data = program.data_definition_by_name_str(attached.as_str())?;
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
        data = crate::declarations::data_queries::data_definition_by_symbol(
            program,
            receiver_type_symbol(program, field_type),
        )?;
    }
    None
}

/// The resolved requirement signature at
/// a call write site. Value-position calls rejoin the receiver symbol from
/// the receiver expression exactly as boundary selection does. When the
/// retained target symbol is absent — a nested receiver path the typer did
/// not annotate — the receiver place's declared leaf trait still selects
/// the requirement by name.
pub(super) fn requirement_signature_for_site<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
) -> Option<&'program typed_trees::signature::StateSignature> {
    let (_, target_symbol) = call_site_symbols(program, site)?;
    requirement_signature_for_target(program, current_machine, receiver, target, target_symbol)
}

/// Resolve the exact requirement independently of its explicit argument storage.
fn requirement_signature_for_target<'program>(
    program: &'program dyn crate::CheckingInput,
    current_machine: &Machine,
    receiver: &[String],
    target: &str,
    target_symbol: SymbolHandle,
) -> Option<&'program typed_trees::signature::StateSignature> {
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
    Some(signature)
}

/// The exact frame of a resolved non-boundary trait requirement call. The
/// implementing body is selected at runtime, so the signature's declared
/// reach is the complete caller-visible evidence: every exclusive
/// parameter writes its argument's proven origins.
pub(super) fn known_requirement_call_written_paths_for_parts(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    // A selector call targets its instantiated slot: the requirement with its
    // trait arguments substituted and the selector as operand 0. That roster
    // is the complete write contract of every implementation it can reach.
    if let Some((_, target_symbol)) = call_site_symbols(program, site)
        && let Some(slot) = program.dynamic_slot_by_symbol(target_symbol)
    {
        return signature_parameters_written_paths(
            program,
            current_machine,
            machine_symbols,
            symbols,
            &slot.parameters,
            arguments,
            inference,
        );
    }
    let signature =
        requirement_signature_for_site(program, current_machine, receiver, target, site)?;
    let parameters = program.state_signature_parameters(signature);
    let paired = parameters.iter().collect::<Vec<_>>();
    if paired.len() != arguments.len() {
        return None;
    }
    let mut written = Vec::new();
    let bindings = super::type_instantiation::signature_call_type_bindings_seeded(
        program,
        current_machine,
        signature,
        site,
        arguments,
        Vec::new(),
    )?;
    boundary_written_argument_paths(
        program,
        current_machine,
        machine_symbols,
        symbols,
        paired,
        arguments,
        &mut written,
        &bindings,
        inference,
    )?;
    Some(written)
}

/// The exact frame of a resolved signature-only machine callee: a boundary,
/// requirement, admitted, or externally realized machine has no authored
/// body, so its declared signature is the complete caller-visible write
/// contract -- each exclusive parameter writes its
/// argument's proven origins, exactly the reach the trait-signature rungs
/// derive. A by-value carrier that can still contain mutable references
/// keeps the call opaque rather than manufacturing a receiver-only frame,
/// and a generic declaration stays opaque because this contract does not
/// seed its bindings.
#[allow(clippy::too_many_arguments)]
pub(super) fn signature_state_call_written_paths(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    callee_machine: &Machine,
    callee_state: &typed_trees::state::State,
    arguments: &[ExpressionHandle],
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    if !program.machine_type_parameters(callee_machine).is_empty()
        || !callee_machine.lifetime_parameters.is_empty()
    {
        return None;
    }
    signature_parameters_written_paths(
        program,
        current_machine,
        machine_symbols,
        symbols,
        program.state_parameters(callee_state),
        arguments,
        inference,
    )
}

/// Project a closed callable signature into caller storage. Machine and legacy
/// operator declarations have the same parameter ownership ceiling; neither a
/// static declaration qualifier nor a copied scalar gives the callee storage.
/// Generic binding and exact declaration selection belong to the caller.
#[allow(clippy::too_many_arguments)]
pub(super) fn signature_parameters_written_paths(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    parameters: &[StateParameter],
    arguments: &[ExpressionHandle],
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let paired = parameters.iter().collect::<Vec<_>>();
    if paired.len() != arguments.len() {
        return None;
    }
    let mut written = Vec::new();
    boundary_written_argument_paths(
        program,
        current_machine,
        machine_symbols,
        symbols,
        paired,
        arguments,
        &mut written,
        &[],
        inference,
    )?;
    Some(written)
}

pub(super) fn receiver_type_symbol(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
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

pub(super) fn selected_boundary_signature(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> bool {
    target.is_valid()
        && program.traits().iter().any(|definition| {
            definition.is_boundary
                && program
                    .trait_machine_signature_by_symbol(definition, target)
                    .is_some()
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
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    receiver: &[String],
    target: &str,
    site: CallerWriteSite<'_>,
    arguments: &[ExpressionHandle],
    inference: &mut FrameInference,
) -> Option<Vec<String>> {
    let selected =
        boundary_write_signature_and_receiver(program, current_machine, receiver, target, site)?;
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
    let parameters =
        super::type_instantiation::signature_arity_parameters(program, signature, arguments)?;
    let bindings = super::type_instantiation::signature_call_type_bindings_seeded(
        program,
        current_machine,
        signature,
        site,
        arguments,
        bindings,
    )?;

    boundary_written_argument_paths(
        program,
        current_machine,
        machine_symbols,
        symbols,
        parameters,
        arguments,
        &mut written,
        &bindings,
        inference,
    )?;
    Some(written)
}

/// The caller storage an exclusive argument may actually reach. A nested
/// boundary call transports the candidate origins its selected signature
/// admits for the result; every other supported expression contributes its
/// single proven origin. The nested call's own receiver/argument footprint
/// is counted separately by the enclosing expression/statement walk; only
/// its exclusive result's may-alias routes join this argument's origins.
fn boundary_argument_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    argument: ExpressionHandle,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    if let Some(call) = program.expression_table.call_node(argument) {
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
/// The signature's exclusive-result referent — `None` when the result is
/// not an exclusive reference at all.
fn exclusive_result_referent(
    program: &dyn crate::CheckingInput,
    signature: &typed_trees::signature::StateSignature,
    bindings: &super::type_instantiation::TypeBindings,
) -> Option<TypeReferenceHandle> {
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
    live_unconstrained_type(program, substituted_head(program, *referee, bindings))
}

fn boundary_result_origins(
    program: &dyn crate::CheckingInput,
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
    let referent = exclusive_result_referent(program, signature, &bindings)?;

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
    let parameters =
        super::type_instantiation::signature_arity_parameters(program, signature, arguments)?;
    exclusive_boundary_result_origins(
        program,
        current_machine,
        machine_symbols,
        symbols,
        parameters,
        arguments,
        referent,
        &mut bindings,
        inference,
    )
}

/// The signature's admitted candidate routes canonicalized through the
/// caller's current alias and stored-carrier evidence, kept as the exact
/// finite union. Union consumers (nested-call arguments) keep every proven
/// route; single-binding consumers collapse it through `single_place_origin`.
/// The signature query behind a `*_result_candidate_origins` entry — the
/// boundary and requirement walks differ only in which resolved signature
/// they ask.
type SignatureResultOrigins = fn(
    &dyn crate::CheckingInput,
    &Machine,
    &MachineSymbols<'_>,
    &TopLevelSymbols<'_>,
    &TableCallExpression,
    ExpressionHandle,
    &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>>;

#[allow(clippy::too_many_arguments)]
fn signature_result_candidate_origins(
    origins: SignatureResultOrigins,
    program: &dyn crate::CheckingInput,
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
        origins(
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

/// Each signature-routed candidate-origins adapter forwards the whole
/// write-frames context to `signature_result_candidate_origins` with its own
/// origins resolver; this states the shared forwarding shape once.
macro_rules! signature_result_candidate_adapter {
    ($(#[$attr:meta])* $adapter:ident => $origins:ident) => {
        $(#[$attr])*
        #[allow(clippy::too_many_arguments)]
        pub(super) fn $adapter(
            program: &dyn crate::CheckingInput,
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
            signature_result_candidate_origins(
                $origins,
                program,
                current_machine,
                machine_symbols,
                symbols,
                call,
                expression,
                inference,
                parameters,
                isolated_local_roots,
                aliases,
                allow_isolated_local,
                stored,
            )
        }
    };
}

signature_result_candidate_adapter! {
    boundary_result_candidate_origins => boundary_result_origins
}

/// The candidate caller-storage origins a resolved requirement call's
/// exclusive result may alias — the same admitted routes as a boundary
/// result, with the signature selected by retained `target_symbol` rather
/// than receiver spelling. Explicit exclusive arguments supply the admitted
/// storage origins.
/// The boundary result's exclusive-reference storage reach, shared by the
/// boundary and requirement walks: each admitted argument whose carrier type
/// may itself store the referent contributes its origins, root-only storage
/// keeps them collection-coarse.
#[allow(clippy::too_many_arguments)]
fn exclusive_boundary_result_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    parameters: Vec<&StateParameter>,
    arguments: &[ExpressionHandle],
    referent: TypeReferenceHandle,
    bindings: &mut TypeBindings,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let mut origins = Vec::new();
    for (parameter, actual) in parameters.into_iter().zip(arguments) {
        let parameter_type = live_unconstrained_type(program, parameter.type_reference)?;
        let parameter_type = substituted_head(program, parameter_type, &*bindings);
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = program.type_reference_table.type_reference(parameter_type)
        else {
            // A by-value carrier can still store exclusive references whose
            // referents this frame cannot name; its route stays opaque.
            if type_may_carry_write_in(program, parameter_type, &*bindings) {
                return None;
            }
            continue;
        };
        if !access.is_exclusive() {
            continue;
        }
        match owned_storage_may_hold(program, *referee, referent, &mut *bindings) {
            Some(true) => {
                // The admitted route places name the storage the result may
                // reach. Unless that storage can hold the referent only at
                // its root, the referent's offset inside stays unknown and a
                // projected origin must not narrow beneath the root.
                let root_only = storage_holds_referent_only_at_root(
                    program,
                    *referee,
                    referent,
                    &mut *bindings,
                );
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

fn requirement_result_origins(
    program: &dyn crate::CheckingInput,
    current_machine: &Machine,
    machine_symbols: &MachineSymbols<'_>,
    symbols: &TopLevelSymbols<'_>,
    call: &TableCallExpression,
    expression: ExpressionHandle,
    inference: &mut FrameInference,
) -> Option<Vec<FramePlaceOrigin>> {
    let receiver = receiver_member_chain(program, call.receiver).unwrap_or_default();
    let signature = requirement_signature_for_site(
        program,
        current_machine,
        &receiver,
        call.target.as_str(),
        CallerWriteSite::Expression(expression),
    )?;
    let arguments = program.expression_table.expression_handles(call.arguments);
    let parameters = program.state_signature_parameters(signature);
    let paired = parameters.iter().collect::<Vec<_>>();
    if paired.len() != arguments.len() {
        return None;
    }
    let mut bindings = super::type_instantiation::signature_call_type_bindings_seeded(
        program,
        current_machine,
        signature,
        CallerWriteSite::Expression(expression),
        arguments,
        Vec::new(),
    )?;
    let referent = exclusive_result_referent(program, signature, &bindings)?;

    exclusive_boundary_result_origins(
        program,
        current_machine,
        machine_symbols,
        symbols,
        paired,
        arguments,
        referent,
        &mut bindings,
        inference,
    )
}

signature_result_candidate_adapter! {
    /// The requirement signature's admitted candidate routes canonicalized
    /// into the caller's alias and stored-carrier namespace, kept as the exact
    /// finite union for the same shared consumers as
    /// `boundary_result_candidate_origins`.
    requirement_result_candidate_origins => requirement_result_origins
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
        if is_self_receiver(root)
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
    program: &dyn crate::CheckingInput,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    bindings: &mut TypeBindings,
) -> Option<bool> {
    let referent = substituted_head(program, referent, bindings);
    owned_storage_may_hold_inner(
        program,
        container,
        referent,
        &mut Vec::new(),
        bindings,
        &mut TypeIdentityMemo::default(),
    )
}

/// `visiting` records the *instantiated* containers already on the path, so
/// `Wrap<Wrap<u64>>` — a finite shape — is not mistaken for the genuine
/// recursion a self-referential instantiation produces.
fn owned_storage_may_hold_inner(
    program: &dyn crate::CheckingInput,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    visiting: &mut Vec<TypeReferenceHandle>,
    bindings: &mut TypeBindings,
    identities: &mut TypeIdentityMemo,
) -> Option<bool> {
    let container =
        live_unconstrained_type(program, substituted_head(program, container, bindings))?;
    let referent = substituted_head(program, referent, bindings);
    if aggregate_storage_types_match_in(program, container, referent, bindings, identities) {
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
                && owned_storage_may_hold_inner(
                    program, *referee, referent, visiting, bindings, identities,
                ) != Some(false)
            {
                return None;
            }
            Some(false)
        }
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => owned_storage_may_hold_inner(
            program,
            *element_type,
            referent,
            visiting,
            bindings,
            identities,
        ),
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
                aggregate_storage_types_match_in(program, *visited, container, bindings, identities)
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
                let field_types = member_field_types(program, member);
                for field_type in field_types {
                    match owned_storage_may_hold_inner(
                        program, field_type, referent, visiting, bindings, identities,
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
    program: &dyn crate::CheckingInput,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    bindings: &mut TypeBindings,
) -> bool {
    let Some(container) =
        live_unconstrained_type(program, substituted_head(program, container, bindings))
    else {
        return false;
    };
    let mut identities = TypeIdentityMemo::default();
    aggregate_storage_types_match_in(program, container, referent, bindings, &mut identities)
        && storage_member_may_hold(
            program,
            container,
            referent,
            &mut Vec::new(),
            bindings,
            &mut identities,
        ) == Some(false)
}

/// `Some` answers whether a proper member or element position inside
/// `container`'s declared storage may hold `referent`; `None` means the
/// proof cannot finish. `container` already matched `referent` at its root,
/// so recursion only ever inspects strictly interior positions.
fn storage_member_may_hold(
    program: &dyn crate::CheckingInput,
    container: TypeReferenceHandle,
    referent: TypeReferenceHandle,
    visiting: &mut Vec<TypeReferenceHandle>,
    bindings: &mut TypeBindings,
    identities: &mut TypeIdentityMemo,
) -> Option<bool> {
    let container = substituted_head(program, container, bindings);
    if program.primitive_type_reference(container).is_some() {
        return Some(false);
    }
    match program.type_reference_table.type_reference(container) {
        TypeReferenceNode::FixedArray { element_type, .. }
        | TypeReferenceNode::Slice { element_type } => owned_storage_may_hold_inner(
            program,
            *element_type,
            referent,
            visiting,
            bindings,
            identities,
        ),
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
                    aggregate_storage_types_match_in(
                        program, *visited, container, bindings, identities,
                    )
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
                    let field_types = member_field_types(program, member);
                    for field_type in field_types {
                        match owned_storage_may_hold_inner(
                            program, field_type, referent, visiting, bindings, identities,
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

/// Every field-type a member contributes to a storage walk: an ordinary
/// field's single type, or each payload field's type of a variant.
fn member_field_types(
    program: &dyn crate::CheckingInput,
    member: &DataMember,
) -> Vec<TypeReferenceHandle> {
    match member {
        DataMember::Field(field) => vec![field.type_reference],
        DataMember::Variant(variant) => program
            .data_payload_fields(variant)
            .iter()
            .map(|field| field.type_reference)
            .collect(),
    }
}

/// A stale nonzero handle resolves to dummy Unit, not an effect-free formal.
/// An acyclic chain visits at most the number of stored type nodes, so this
/// bound rejects cycles without imposing a smaller depth limit on valid types.
fn live_unconstrained_type(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    typed_trees::operator::unconstrained_type_reference(program.typed_program(), reference)
}
