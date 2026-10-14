//! Which boundary scalar targets, argument presentations and
//! mutable byte-array views a structural call may admit.

use crate::execution::terminal_unit::{
    Multiplicity, PrimitiveType, StateParameter, SymbolHandle, TypeReferenceNode,
};

/// The source of boundary authority is independent of operand scheduling.
/// Consumers retain the ordinary parameter in their own signature. Selecting
/// a provider and establishing an entry context belong to realization; this
/// query neither provisions a value nor invents implicit `self` authority.
pub(crate) enum BoundaryTraitReceiver {
    Namespace,
    Parameters(Vec<SymbolHandle>),
}

pub(crate) fn boundary_trait_receiver_source(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
    call_site: &validation::CallSite<'_>,
    requirement: SymbolHandle,
) -> Option<BoundaryTraitReceiver> {
    if !call.has_receiver {
        return None;
    }
    if call.receiver_symbol == requirement {
        return Some(BoundaryTraitReceiver::Namespace);
    }
    let sources = parameter_binding_receiver_sources(
        program,
        state,
        call.statement_index,
        call_site,
        requirement,
    )?;
    Some(BoundaryTraitReceiver::Parameters(
        sources
            .into_iter()
            .map(|(source, _)| source.symbol)
            .collect(),
    ))
}

/// Resolve the ordinary source parameter whose Binding or projection has an exact
/// declared requirement. This identifies a receiver, not a call admission: the
/// call planner separately requires its structural parameter, and attachment
/// collection uses this only for an already-planned matching boundary call.
pub(crate) fn parameter_binding_receiver_sources<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_site: &validation::CallSite<'_>,
    requirement: SymbolHandle,
) -> Option<Vec<(&'program StateParameter, validation::CanonicalPlace)>> {
    resolve_parameter_binding_receiver_sources(
        program,
        state,
        statement_index,
        call_site,
        requirement,
    )
    .ok()
}

fn resolve_parameter_binding_receiver_sources<'program>(
    program: &'program dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statement_index: usize,
    call_site: &validation::CallSite<'_>,
    requirement: SymbolHandle,
) -> Result<Vec<(&'program StateParameter, validation::CanonicalPlace)>, &'static str> {
    let place = match call_site {
        validation::CallSite::Expression { call, .. } => {
            validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                call.receiver,
            )
        }
        validation::CallSite::Statement(call) => {
            validation::projected_statement_receiver_place(program, state, statement_index, call)
                .map(|(root, segments)| validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                })
        }
        validation::CallSite::TransitionNamed { .. } => None,
    };
    let place = place.ok_or("boundary authority: operand has no retained source place")?;
    let machine = program
        .machine_holding_state(state.symbol)
        .ok_or("boundary authority: missing caller machine")?;
    let before = program
        .statement_table
        .statement_at(state, statement_index)
        .ok_or("boundary authority: missing call statement")?;
    let resolver = validation::CallFrameResolver::new(program)
        .ok_or("boundary authority: caller reference analysis unavailable")?;
    let places = match place.root {
        facts::PlaceRoot::Expression(expression) => resolver
            .reference_expression_origins_before_statement(machine, before, expression)
            .ok_or("boundary authority: returned reference has no exact caller-owned origin")?
            .into_iter()
            .map(|(root, mut segments)| {
                segments.extend_from_slice(&place.segments);
                validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments,
                }
            })
            .collect(),
        facts::PlaceRoot::Symbol(root)
            if program.symbols.get(root).kind == symbols::SymbolKind::Local =>
        {
            resolver
                .local_reference_origins_before_statement(machine, before, root)
                .ok_or("boundary authority: local reference has no exact live caller-owned origin")?
                .into_iter()
                .map(|(root, mut segments)| {
                    segments.extend_from_slice(&place.segments);
                    validation::CanonicalPlace {
                        root: facts::PlaceRoot::Symbol(root),
                        segments,
                    }
                })
                .collect()
        }
        _ => vec![place],
    };
    if places.is_empty() {
        return Err("boundary authority: reference has no possible caller-owned origin");
    }
    places
        .into_iter()
        .map(|place| {
            let root = place
                .root
                .as_symbol()
                .ok_or("boundary authority: origin has no declaration identity")?;
            let source = program
                .state_parameters(state)
                .iter()
                .find(|parameter| parameter.symbol == root && !parameter.relevance.is_erased())
                .ok_or("boundary authority: origin is not a retained caller parameter")?;
            let (reference, _) = super::argument_paths::projected_argument_path(
                program,
                state.symbol,
                statement_index,
                &place,
                None,
                None,
            )
            .ok_or("boundary authority: origin projection is not representable")?;
            // A Binding member identifies the requirement through its own carrier.
            // Provider selection and entry establishment are realization obligations.
            let target = match call_site {
                validation::CallSite::Expression { call, .. } => call.target_symbol,
                validation::CallSite::Statement(call) => call.target_symbol,
                validation::CallSite::TransitionNamed { .. } => {
                    return Err("boundary authority: terminal receiver is not retained");
                }
            };
            typed_trees::service::classify_exact_service_parameter_carrier(
                program,
                typed_trees::service::ServiceCarrierOccurrence {
                    type_reference: reference,
                    lexical_owner: state.symbol,
                    enclosing_data_applications: &[],
                },
            )
            .ok()
            .flatten()
            .filter(|carrier| {
                carrier
                    .carrier
                    .application(program, state.symbol, &[])
                    .is_some_and(|scope| {
                        scope.declaring_scope(program, requirement).is_some()
                            && program.closed_requirement_application(target).is_none_or(
                                |application| {
                                    program.requirement_application_matches_scope(
                                        application.signature.symbol,
                                        &scope,
                                    )
                                },
                            )
                    })
            })
            .map(|_| (source, place))
            .ok_or("boundary authority: origin does not carry the exact selected requirement")
        })
        .collect()
}

/// Failure-only explanation for an unclaimed boundary occurrence. This uses
/// the same admission relation as planning; it never supplies missing custody.
pub(crate) fn boundary_authority_refusal(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: &typed_trees::state::State,
    call: &checked_trees::FlowCallFact,
) -> Option<&'static str> {
    let owner = program.traits().iter().find(|owner| {
        owner.is_boundary
            && program
                .trait_machine_instances(owner)
                .any(|signature| signature.symbol == call.target_symbol)
    })?;
    if call.receiver_symbol == owner.symbol {
        return None;
    }
    let site = validation::find_call_site(
        program,
        machine,
        state.symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    resolve_parameter_binding_receiver_sources(
        program,
        state,
        call.statement_index,
        &site,
        owner.symbol,
    )
    .err()
}

/// A fixed array lends its initialized extent as the borrowed element view a
/// checked-body callee declares. This is the element-typed sibling of the byte
/// presentation above: the source keeps its literal backing, the target keeps
/// its declared `&[T]`/`&mut [T]` view, and the projected path names the
/// storage rather than materializing a view local.
/// Lending does not copy the array or its elements. Their multiplicity stays
/// with the backing owner; callers independently establish initialized storage,
/// qualifications and exact loan access before presenting this carrier.
pub(crate) fn fixed_array_slice_view_is_admitted(
    program: &dyn ::validation::CheckingInput,
    source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some((_, element_type, target_element)) =
        fixed_array_slice_view_parts(program, source, target)
    else {
        return false;
    };
    program.normalized_type_identity(*element_type)
        == program.normalized_type_identity(*target_element)
}

/// A fixed byte array lends initialized elements without becoming a bounded owner.
pub(crate) fn fixed_byte_array_view_is_admitted(
    program: &dyn ::validation::CheckingInput,
    source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let Some((source, element_type, target_element)) =
        fixed_array_slice_view_parts(program, source, target)
    else {
        return false;
    };
    [*element_type, *target_element].into_iter().all(|element| {
        matches!(
            program.type_reference_table.type_reference(element),
            TypeReferenceNode::Named { .. }
        ) && program.primitive_type_reference(element) == Some(PrimitiveType::U8)
    }) && crate::checks::type_multiplicity(program, source) == Multiplicity::Unrestricted
}

/// Peels one reference layer off `source` and destructures it as a literal
/// fixed array while `target` destructures as a shared-or-mutable reference to
/// a slice. Returns the peeled source and both element types when the source/
/// target shape is the fixed-array-to-slice view both admissions test.
fn fixed_array_slice_view_parts<'a>(
    program: &'a dyn ::validation::CheckingInput,
    mut source: typed_trees::types::TypeReferenceHandle,
    target: typed_trees::types::TypeReferenceHandle,
) -> Option<(
    typed_trees::types::TypeReferenceHandle,
    &'a typed_trees::types::TypeReferenceHandle,
    &'a typed_trees::types::TypeReferenceHandle,
)> {
    if let TypeReferenceNode::Reference { referee, .. } =
        program.type_reference_table.type_reference(source)
    {
        source = *referee;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: typed_trees::types::FixedArrayLength::Literal(_),
    } = program.type_reference_table.type_reference(source)
    else {
        return None;
    };
    let TypeReferenceNode::Reference {
        access: language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::Shared,
        referee,
        ..
    } = program.type_reference_table.type_reference(target)
    else {
        return None;
    };
    let TypeReferenceNode::Slice {
        element_type: target_element,
    } = program.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    Some((source, element_type, target_element))
}

/// The `self.<field>` receiver a boundary call site routes through, when the
/// call is carried by an attached provider field at all: the field's authored
/// name plus the resolved member symbol an expression-site receiver carries.
/// A direct `Boundary::method` call and a non-`self` receiver have neither.
fn provider_attachment_receiver(
    program: &dyn ::validation::CheckingInput,
    call_site: &validation::CallSite<'_>,
) -> Option<(typed_trees::name::Identifier, Option<SymbolHandle>)> {
    match call_site {
        validation::CallSite::Statement(call) => {
            let [self_name, field_name] = program.statement_table.name_path_members(call.receiver)
            else {
                return None;
            };
            if !self_name.is_self_receiver() {
                return None;
            }
            Some((field_name.clone(), None))
        }
        validation::CallSite::Expression { call, .. } => {
            let (_, Some(receiver)) = validation::call_receiver_parts(program, call.receiver)
            else {
                return None;
            };
            let [self_name, field_name] = receiver.members() else {
                return None;
            };
            if !self_name.is_self_receiver() {
                return None;
            }
            Some((field_name.clone(), Some(receiver.member_symbol(1))))
        }
        validation::CallSite::TransitionNamed { .. } => None,
    }
}

/// The receiver field's authored name alone, so a caller can tell whether a
/// call is routed through `self.<field>` at all.
pub(crate) fn provider_attachment_receiver_field(
    program: &dyn ::validation::CheckingInput,
    call_site: &validation::CallSite<'_>,
) -> Option<typed_trees::name::Identifier> {
    provider_attachment_receiver(program, call_site).map(|(name, _)| name)
}

pub(crate) fn provider_attachment_receiver_matches(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    call_site: &validation::CallSite<'_>,
    provider_symbol: SymbolHandle,
) -> bool {
    let Some((field_name, selected_field)) = provider_attachment_receiver(program, call_site)
    else {
        return false;
    };
    if !machine.attached_data_symbol.is_valid() {
        return false;
    }
    // The symbol, not the bare name: leaf-keyed attachments collide when two
    // modules declare same-named records (`left::Runner`/`right::Runner`), and
    // a name match would route `self.reader` to the sibling leaf's carrier.
    let Some(attached) = program.data_definition_by_symbol(machine.attached_data_symbol) else {
        return false;
    };
    program.data_members(attached).iter().any(|member| {
        let Some(field) = member.as_field() else {
            return false;
        };
        if field.name != field_name
            || field.relevance.is_erased()
            || selected_field.is_some_and(|symbol| symbol != field.symbol)
        {
            return false;
        }
        field_type_carries_requirement(program, field.type_reference, provider_symbol)
            || service_record_member_carries_requirement(
                program,
                field.type_reference,
                provider_symbol,
            )
    })
}

/// Whether `type_reference` presents `provider_symbol` as a direct boundary
/// carrier: an exact `Binding<requirement>` slot, or the provider field spelled
/// `&'a mut <boundary trait>` (unwrapping qualifications and the mutable
/// borrow to the trait symbol; a shared `&` borrow is not a provider handle).
fn field_type_carries_requirement(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    provider_symbol: SymbolHandle,
) -> bool {
    typed_trees::service::service_requirement_declaration(program, type_reference)
        == Some(provider_symbol)
        || crate::execution::terminal_unit::types::mutable_boundary_trait_symbol(
            program,
            type_reference,
        ) == Some(provider_symbol)
}

/// The ergonomic service-record spelling: `self.fs` is a `data` record that
/// holds the raw boundary binding as a member (`host: Binding<FilesystemHost>`)
/// rather than being the binding itself. The `self.<field>` receiver reaches
/// the requirement when the field's record type carries it exactly one level
/// down; deeper nesting stays refused so a named member cannot launder an
/// indirect boundary path past the attachment rule.
fn service_record_member_carries_requirement(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    provider_symbol: SymbolHandle,
) -> bool {
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(type_reference)
    else {
        return false;
    };
    let Some(definition) = program.data_definition_by_symbol(*symbol) else {
        return false;
    };
    program.data_members(definition).iter().any(|member| {
        let Some(field) = member.as_field() else {
            return false;
        };
        !field.relevance.is_erased()
            && field_type_carries_requirement(program, field.type_reference, provider_symbol)
    })
}
