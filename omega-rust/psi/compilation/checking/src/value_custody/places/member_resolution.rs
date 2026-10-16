//! Resolve expression projections to declaration identities used by canonical places.
//! Case payload spelling is not unique within a data declaration. Reconstruct
//! qualified fields inside the selected case so borrow and Terminal consumers
//! see the same Case/Field path; never repair a conflicting retained identity.

use super::declared_symbols::first_valid_name_path_symbol;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use symbols::SymbolHandle;

/// Where a demanded member path currently points: a stored type reference
/// that still retains the reaching generic application's arguments, or a
/// declaration reached without one (an attached `self` datum, a struct
/// literal's type), or a half-open window taken from a collection. This
/// mirrors the typed-tree lowerer's `SubjectPosition`
/// and the checker-side partition replay: a `Box<Context>` leaf keeps its
/// `T` argument bound so a member whose declared type is `T` resumes the
/// walk at `Context` instead of stopping at an opaque leaf.
///
/// `Sliced` carries the window's element reference rather than the window's
/// own type: no slice type-reference node exists to mint for it, and the
/// distinction is load-bearing — an index hop resumes at the element while
/// a member demand resolves nothing, because a slice declares no fields.
///
/// Position equality is identity equality on the stored evidence — the same
/// reference handle, the same declaration, or the same element reference —
/// not type equality: two positions naming equal types through different
/// stored rows do not join, because neither is evidence for the other.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum MemberPosition {
    Reference(typed_trees::types::TypeReferenceHandle),
    Declaration(SymbolHandle),
    Sliced(typed_trees::types::TypeReferenceHandle),
}

/// The declaration symbol a position's leaf names. Substituted arguments are
/// already folded into a `Reference` leaf by the projection replay. Closed
/// generic applications name their retained instance, not the open template;
/// other positions retain the ordinary `type_symbol` mapping.
pub(super) fn position_leaf_symbol(
    program: &dyn crate::CheckingInput,
    position: MemberPosition,
) -> SymbolHandle {
    match position {
        MemberPosition::Reference(reference) => closed_generic_owner(program, reference)
            .map(|owner| owner.symbol)
            .unwrap_or_else(|| program.type_reference_table.type_symbol(reference)),
        MemberPosition::Declaration(symbol) => symbol,
        // A window's leaf is the slice, which owns no declaration identity:
        // only a further index hop names the element a member could live on.
        MemberPosition::Sliced(_) => SymbolHandle::invalid(),
    }
}

fn closed_generic_owner(
    program: &dyn crate::CheckingInput,
    mut reference: typed_trees::types::TypeReferenceHandle,
) -> Option<&typed_trees::data::DataDefinition> {
    while let typed_trees::types::TypeReferenceNode::Reference { referee, .. }
    | typed_trees::types::TypeReferenceNode::Constrained {
        base_type: referee, ..
    } = program.type_reference_table.type_reference(reference)
    {
        reference = *referee;
    }
    let owner = program.closed_data_definition(reference)?;
    owner.generic_instance.is_some().then_some(owner)
}

/// A closed instance's field is selected by its receiver application. A
/// retained template selection may translate only along that instance's
/// validated member correspondence; a foreign or sibling-instance symbol
/// remains unresolved. `None` leaves the existing open-generic route intact.
pub(super) fn closed_generic_member_symbol(
    program: &dyn crate::CheckingInput,
    position: Option<MemberPosition>,
    member_name: &str,
    retained: SymbolHandle,
) -> Option<SymbolHandle> {
    let MemberPosition::Reference(reference) = position? else {
        return None;
    };
    closed_generic_owner(program, reference)?;
    let reference = program
        .type_reference_table
        .peeled_type_reference(reference);
    // The application is the receiver, not its erased specialization origin.
    // Reopening that origin can select another prepared field namespace even
    // when the receiver already retains the exact closed member selection.
    let selected = program
        .closed_data_member_field(reference, retained, member_name)
        .map(|field| field.symbol);
    Some(selected.unwrap_or_else(SymbolHandle::invalid))
}

/// One member hop through a position: the member's declared type reference
/// with the receiver's reaching generic application still bound, so
/// `Box<Context>::item` resumes at `Context`. Falls back to the member's own
/// declared position when the receiver does not replay (a declaration
/// position, an opaque leaf, or a member outside the replayed declaration) —
/// the exact contract the bare-symbol walk had. A closed generic receiver
/// instead refuses a field outside its selected instance. The caller supplies the
/// receiver's already-resolved position: a member chain recomputing it here
/// would re-walk the whole receiver expression once per hop.
fn member_type_position(
    program: &dyn crate::CheckingInput,
    receiver_position: Option<MemberPosition>,
    member_symbol: SymbolHandle,
) -> Option<MemberPosition> {
    if let Some(MemberPosition::Reference(reference)) = receiver_position
        && let Some(projected) = super::projection_types::project_type_reference_from_segments(
            program,
            reference,
            &[facts::PlaceSegment::Field {
                symbol: member_symbol,
            }],
        )
    {
        return Some(MemberPosition::Reference(projected));
    }
    if matches!(receiver_position, Some(MemberPosition::Reference(reference))
        if closed_generic_owner(program, reference).is_some())
    {
        return None;
    }
    symbol_type_position(program, member_symbol)
}

/// The container a member-parented symbol was looked up in.
enum ContainerOutcome {
    /// The parent mapped to a container and the member resolved inside it.
    Position(MemberPosition),
    /// The parent mapped to a container but holds no such member — definite
    /// miss, since symbol storage is keyed to the declaring container.
    MappedMiss,
    /// The parent names no jump container (roots, trait signatures,
    /// operators, machine symbols themselves); the caller must scan.
    Unmapped,
}

/// The member-position lookup `symbol_type_position` performs, restricted to
/// the one container `parent` names. Symbol storage is keyed to the
/// declaring parent, so a machine-parented member resolves through that
/// machine's attached and owned rows, a state-parented member through that
/// state's parameters and locals, a data-parented member through its
/// declaration's fields, and a variant-parented member through that
/// variant's payload fields — each reproducing the row the whole-program
/// scan's first match yields. Parents with no container role here (roots,
/// trait signatures, operators, machines themselves) are `Unmapped` and the
/// caller scans.
fn container_member_type_position(
    program: &dyn crate::CheckingInput,
    parent: SymbolHandle,
    symbol: SymbolHandle,
) -> ContainerOutcome {
    if !parent.is_valid() {
        return ContainerOutcome::Unmapped;
    }
    // The declaring parent's kind names its container class exactly: member
    // storage is keyed to the declaring container, so a machine-parented
    // member lives in machine rows and a state member in state rows — never
    // two classes at once. Dispatching on the row's kind jumps each lookup
    // straight to its one scan instead of walking every container family in
    // sequence (the previous order ran up to seven linear scans per call).
    // Parents whose kind owns no member storage — roots, modules, traits,
    // domains, signatures' own rows — map to Unmapped, the same result the
    // all-miss walk produced.
    let grandparent = program.symbols.get(parent).parent;
    match program.symbols.get(parent).kind {
        symbols::SymbolKind::Machine => {
            let Some(machine) = program.machine_by_symbol(parent) else {
                return ContainerOutcome::Unmapped;
            };
            if let Some(field) =
                crate::exact_attached_field(program, machine, symbol, program.symbols.name(symbol))
            {
                return ContainerOutcome::Position(MemberPosition::Reference(field.type_reference));
            }
            program
                .machine_owned_data_by_symbol(machine, symbol)
                .map(|owned| {
                    ContainerOutcome::Position(MemberPosition::Reference(owned.type_reference))
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        symbols::SymbolKind::Data => {
            let Some(data) = program.data_definition_by_symbol(parent) else {
                return ContainerOutcome::Unmapped;
            };
            program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Field(field) if field.symbol == symbol => Some(
                        ContainerOutcome::Position(MemberPosition::Reference(field.type_reference)),
                    ),
                    _ => None,
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        // A payload field's parent is its variant and the variant's parent
        // is the data row, so two hops reach the same field list the scan
        // walks.
        symbols::SymbolKind::Variant => {
            if !grandparent.is_valid() {
                return ContainerOutcome::Unmapped;
            }
            let Some(data) = program.data_definition_by_symbol(grandparent) else {
                return ContainerOutcome::Unmapped;
            };
            let Some(variant) = program
                .data_members(data)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant) if variant.symbol == parent => {
                        Some(variant)
                    }
                    _ => None,
                })
            else {
                return ContainerOutcome::Unmapped;
            };
            program
                .data_payload_field_by_symbol(variant, symbol)
                .map(|field| {
                    ContainerOutcome::Position(MemberPosition::Reference(field.type_reference))
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        // States and trait signatures share SymbolKind::State; the row's own
        // parent separates them — a signature's parent names its trait, a
        // state member's parent names its state.
        symbols::SymbolKind::State
            if grandparent.is_valid()
                && program.symbols.get(grandparent).kind == symbols::SymbolKind::Trait =>
        {
            let Some(trait_definition) = program.trait_definition_by_symbol(grandparent) else {
                return ContainerOutcome::Unmapped;
            };
            let Some(signature) =
                program.trait_machine_signature_by_symbol(trait_definition, parent)
            else {
                return ContainerOutcome::Unmapped;
            };
            program
                .state_signature_parameters(signature)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .map(|parameter| {
                    ContainerOutcome::Position(MemberPosition::Reference(parameter.type_reference))
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        symbols::SymbolKind::State => {
            let Some(state) = program.state_by_symbol(parent) else {
                return ContainerOutcome::Unmapped;
            };
            if let Some(parameter) = program.state_parameter_by_symbol(state, symbol) {
                return ContainerOutcome::Position(MemberPosition::Reference(
                    parameter.type_reference,
                ));
            }
            program
                .state_local_declaration(state, symbol)
                .map(|(_, local_data)| {
                    ContainerOutcome::Position(MemberPosition::Reference(local_data.type_reference))
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        // Domain operators chain through the domain row the way signatures
        // chain through traits; a top-level operator's parameters resolve on
        // the operator itself.
        symbols::SymbolKind::Operator
            if grandparent.is_valid()
                && program.symbols.get(grandparent).kind == symbols::SymbolKind::Domain =>
        {
            let Some(domain) = program.domain_definition(grandparent) else {
                return ContainerOutcome::Unmapped;
            };
            let Some(operator) = program
                .domain_operators(domain)
                .iter()
                .find(|operator| operator.symbol == parent)
            else {
                return ContainerOutcome::Unmapped;
            };
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .map(|parameter| {
                    ContainerOutcome::Position(MemberPosition::Reference(parameter.type_reference))
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        symbols::SymbolKind::Operator => {
            let Some(operator) = program.operator_definition_by_symbol(parent) else {
                return ContainerOutcome::Unmapped;
            };
            program
                .operator_parameters(operator)
                .iter()
                .find(|parameter| parameter.symbol == symbol)
                .map(|parameter| {
                    ContainerOutcome::Position(MemberPosition::Reference(parameter.type_reference))
                })
                .unwrap_or(ContainerOutcome::MappedMiss)
        }
        _ => ContainerOutcome::Unmapped,
    }
}

/// The position a resolved symbol's declared type points at. Unlike
/// `symbol_type_symbol`, this retains the type reference so a generic
/// application's arguments remain bound for the next member hop.
pub(super) fn symbol_type_position(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<MemberPosition> {
    if !symbol.is_valid() {
        return None;
    }

    // A declared symbol's parent names its owning container, so the scan can
    // jump straight to it: an attached or owned member resolves inside its
    // machine, a state member inside its state's parameters and locals, a
    // data member inside its declaration, a payload member inside its
    // variant's fields — the same row the whole-program first-match scan
    // produced, without walking every unrelated machine, signature,
    // operator, and data row. A parent carrying no container role (roots,
    // signatures, operators) or a miss inside the jump keeps the full scan.
    let parent = program.symbols.get(symbol).parent;
    match container_member_type_position(program, parent, symbol) {
        ContainerOutcome::Position(position) => return Some(position),
        // A symbol whose parent is one of the containers the jump indexes
        // cannot hold a scanned position elsewhere: symbol storage is keyed
        // to the declaring container, so a miss inside the mapped parent is
        // definite and the scan is skipped entirely. Only unmapped parents
        // (roots, trait signatures, operators, machine symbols themselves)
        // still scan.
        ContainerOutcome::MappedMiss => return None,
        ContainerOutcome::Unmapped => {}
    }
    // Only member-capable kinds can hold a scan position: parameters, state
    // locals, data and payload fields, and the machine's own symbol (its
    // attached data declaration). Every other kind — type parameters,
    // roots, states, signatures, operators, traits, domains, variants —
    // stores no member row, so the scan is provably a miss for them.
    match program.symbols.get(symbol).kind {
        symbols::SymbolKind::Machine => {
            // A machine's position is its attached data declaration; resolve
            // that row directly instead of walking every unrelated machine.
            let machine = program.machine_by_symbol(symbol)?;
            if machine.attached_data_application.is_valid() {
                return Some(MemberPosition::Reference(machine.attached_data_application));
            }
            return program
                .attached_data_definition(machine)
                .map(|data| MemberPosition::Declaration(data.symbol));
        }
        symbols::SymbolKind::Parameter
        | symbols::SymbolKind::Local
        | symbols::SymbolKind::Field => {}
        _ => return None,
    }
    whole_program_member_type_position(program, symbol)
}

/// The original whole-program member scan: machines and their states, trait
/// signatures, operators, and data rows, first match wins.
fn whole_program_member_type_position(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<MemberPosition> {
    for machine in program.machines() {
        if machine.symbol == symbol {
            // The retained attached application keeps the owner's generic
            // arguments bound; a nongeneric attachment names its data
            // declaration directly.
            if machine.attached_data_application.is_valid() {
                return Some(MemberPosition::Reference(machine.attached_data_application));
            }
            if let Some(data) = program.attached_data_definition(machine) {
                return Some(MemberPosition::Declaration(data.symbol));
            }
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return Some(MemberPosition::Reference(parameter.type_reference));
                }
            }
            for statement in program.statement_table.statements(state.statement_nodes) {
                if let Some(local_data) = statement.as_local_data()
                    && local_data.symbol == symbol
                {
                    return Some(MemberPosition::Reference(local_data.type_reference));
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(MemberPosition::Reference(owned.type_reference));
            }
        }
    }

    // Trait requirements are checked state signatures rather than executable
    // machine states. Their parameter symbols still own exact declared types
    // and must participate in member-place resolution for requirement
    // contracts.
    for definition in program.traits() {
        for signature in program.trait_machine_signatures(definition) {
            for parameter in program.state_signature_parameters(signature) {
                if parameter.symbol == symbol {
                    return Some(MemberPosition::Reference(parameter.type_reference));
                }
            }
        }
    }

    // Operator contracts are declaration-owned proof expressions rather than
    // executable state bodies. Their parameters nevertheless have ordinary
    // typed member-place semantics and must resolve through their exact
    // operator declaration.
    for operator in program.all_operators() {
        for parameter in program.operator_parameters(operator) {
            if parameter.symbol == symbol {
                return Some(MemberPosition::Reference(parameter.type_reference));
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            match member {
                typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return Some(MemberPosition::Reference(field.type_reference));
                }
                typed_trees::data::DataMember::Variant(variant) => {
                    if let Some(field) = program.data_payload_field_by_symbol(variant, symbol) {
                        return Some(MemberPosition::Reference(field.type_reference));
                    }
                }
                _ => {}
            }
        }
    }

    None
}

/// The position an expression's type points at, keeping generic application
/// arguments bound through member hops so `b.item` under `b: &Box<Context>`
/// stands on `Context`, not on `Box`'s unbound `T`.
pub(super) fn expression_type_position(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<MemberPosition> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => expression_type_position(program, inner.target),
        // An atomic expression's value is its operand's evaluation (the
        // checked interpreter lowers the node the same way), so the position
        // is the operand's position rather than an opaque leaf.
        ExpressionNode::Atomic(atomic) => expression_type_position(program, atomic.value),
        ExpressionNode::Call(call) => {
            let retained = program
                .expression_table
                .constructed_value_type_reference(expression);
            retained
                .is_valid()
                .then_some(retained)
                .or_else(|| crate::call_target_return_type(program, call.target_symbol))
                .map(MemberPosition::Reference)
        }
        ExpressionNode::Name(path) => {
            let symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            symbol_type_position(program, symbol)
        }
        ExpressionNode::Indexed(indexed) => {
            // The index hop lands on an element, not on the collection itself:
            // replay the same segment canonicalization pushes for this
            // expression so `values[i].item` resumes at `Box<Context>` with
            // its generic argument still bound. A collection whose position
            // does not project to an element keeps no position rather than
            // minting the collection's own for the element.
            let segment = super::canonical_projection::index_place_segment(program, indexed.index);
            match expression_type_position(program, indexed.collection) {
                Some(MemberPosition::Reference(reference)) => match segment {
                    // A range hop yields a window over the same element —
                    // `values[0..2]` is a slice of `Box<Context>`, not one
                    // element — so the position becomes the window and only
                    // a further index hop resumes at the element.
                    facts::PlaceSegment::FixedRange { .. } => {
                        super::projection_types::collection_element_type_reference(
                            program, reference,
                        )
                        .map(MemberPosition::Sliced)
                    }
                    _ => super::projection_types::project_type_reference_from_segments(
                        program,
                        reference,
                        &[segment],
                    )
                    .map(MemberPosition::Reference),
                },
                Some(MemberPosition::Sliced(element)) => match segment {
                    // Re-windowing a window keeps the same element; an index
                    // hop names one element and resumes at its position.
                    facts::PlaceSegment::FixedRange { .. } => Some(MemberPosition::Sliced(element)),
                    _ => Some(MemberPosition::Reference(element)),
                },
                position => position,
            }
        }
        ExpressionNode::Member(member) => {
            let receiver_position = expression_type_position(program, member.receiver);
            let symbol = effective_member_symbol_from_position(
                program,
                member.receiver,
                receiver_position,
                member,
            );
            member_type_position(program, receiver_position, symbol)
        }
        ExpressionNode::StructLiteral(literal) => {
            let reference = program
                .expression_table
                .constructed_value_type_reference(expression);
            if reference.is_valid() {
                Some(MemberPosition::Reference(reference))
            } else {
                literal
                    .type_symbol
                    .is_valid()
                    .then_some(MemberPosition::Declaration(literal.type_symbol))
            }
        }
        // A cast stores its complete normalized result qualification on the
        // node, so the leaf's position is that reference outright. An
        // unresolved stored result (zero means unnormalized) keeps none.
        ExpressionNode::Cast(cast) => cast
            .result_type
            .is_valid()
            .then_some(MemberPosition::Reference(cast.result_type)),
        // The proof-only zero value of `T` is a `T`; the node stores the
        // exact reference, so its position is that reference outright.
        ExpressionNode::ZeroValue(reference) => reference
            .is_valid()
            .then_some(MemberPosition::Reference(*reference)),
        ExpressionNode::ArrayLiteral(elements) => {
            let retained = program
                .expression_table
                .constructed_value_type_reference(expression);
            if retained.is_valid() {
                Some(MemberPosition::Reference(retained))
            } else {
                array_literal_position(
                    program,
                    program.expression_table.expression_handles(*elements),
                )
            }
        }
        ExpressionNode::Match(dispatch) => match_result_position(program, dispatch),
        // The remaining leaves produce scalar values — literals, unary and
        // binary results, range operands — whose types own no declaration
        // identity to resume at, so they keep no position rather than
        // borrowing a same-shaped row.
        _ => None,
    }
}

/// An array literal is a fixed collection over one element type: the
/// position is `Sliced` over that element — an index hop resumes at the
/// element, a range keeps the same window, and a member demand resolves
/// nothing because an array declares no fields. Every element must agree on
/// one exact `Reference` position; an opaque element or a different stored
/// handle keeps the literal unproven rather than naming one element's type
/// for the whole collection. Elements whose positions are not references —
/// a declaration leaf, another window — carry no element handle the window
/// could reuse, so the literal keeps no position there either.
fn array_literal_position(
    program: &dyn crate::CheckingInput,
    elements: &[ExpressionHandle],
) -> Option<MemberPosition> {
    let mut positions = elements
        .iter()
        .map(|element| expression_type_position(program, *element));
    let Some(Some(first)) = positions.next() else {
        return None;
    };
    let MemberPosition::Reference(element) = first else {
        return None;
    };
    positions
        .all(|position| position == Some(first))
        .then_some(MemberPosition::Sliced(element))
}

/// Replay retained place segments against a member position, producing the
/// position the walk resumes at after the hops — the shared fold both member
/// resolution (`resolve_member_symbol_from_place`) and reference-type
/// reconstruction (`expression_place_type_reference`) commit to, so a member
/// demand and a declared-type question never disagree about where a segment
/// leaves the walk.
///
/// A `Case` segment qualifies spelling only; the position is unchanged. A
/// `Field` hop on a reference position replays the reaching generic
/// application's own substitution (see `project_type_reference_from_segments`),
/// so a field whose declared type is a bound parameter resumes at the
/// supplied argument — `Box<Context>::item` continues at `Context`. When the
/// hop does not replay (a declaration position, an opaque leaf, or a field
/// outside the replayed declaration) the field's own declared position
/// applies, as before. A window declares no fields: keeping the element
/// position would mint the element's member for the slice itself, so a field
/// demanded on a `Sliced` position stays unresolved.
///
/// An index hop lands on an element, not on the collection itself: a
/// collection reference projects to its element through the same
/// collection-to-element projection `expression_type_position` applies to an
/// `Indexed` node, so `values[i]` resumes at the element position with the
/// collection's generic arguments still bound. A position that does not
/// project to an element keeps no position rather than minting the
/// collection's own for the element; a declaration position names a record,
/// which has no element to resume at. An index into a window names one of
/// its elements, so the walk resumes at the element the window was taken
/// over.
///
/// A range hop produces a slice of the collection, not one element: the
/// position becomes the window over the same element, so a later index hop
/// still resumes at the element while a member demand resolves nothing — a
/// slice declares no fields to answer it with. Re-windowing a window keeps
/// its element.
pub(super) fn member_position_after_segments(
    program: &dyn crate::CheckingInput,
    position: MemberPosition,
    segments: &[facts::PlaceSegment],
) -> Option<MemberPosition> {
    let mut position = position;
    for segment in segments {
        position = match segment {
            facts::PlaceSegment::Case { .. } => position,
            facts::PlaceSegment::Field { symbol } => match position {
                MemberPosition::Reference(reference) => {
                    match super::projection_types::project_type_reference_from_segments(
                        program,
                        reference,
                        std::slice::from_ref(segment),
                    ) {
                        Some(projected) => MemberPosition::Reference(projected),
                        None if closed_generic_owner(program, reference).is_some() => return None,
                        None => symbol_type_position(program, *symbol)?,
                    }
                }
                MemberPosition::Declaration(_) => symbol_type_position(program, *symbol)?,
                MemberPosition::Sliced(_) => return None,
            },
            facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                match position {
                    MemberPosition::Reference(reference) => MemberPosition::Reference(
                        super::projection_types::project_type_reference_from_segments(
                            program,
                            reference,
                            std::slice::from_ref(segment),
                        )?,
                    ),
                    MemberPosition::Sliced(element) => MemberPosition::Reference(element),
                    MemberPosition::Declaration(_) => return None,
                }
            }
            facts::PlaceSegment::FixedRange { .. } | facts::PlaceSegment::OpaqueRange => {
                match position {
                    MemberPosition::Reference(reference) => MemberPosition::Sliced(
                        super::projection_types::collection_element_type_reference(
                            program, reference,
                        )?,
                    ),
                    MemberPosition::Sliced(element) => MemberPosition::Sliced(element),
                    MemberPosition::Declaration(_) => return None,
                }
            }
        };
    }
    Some(position)
}

/// The declared type reference a place rooted at `expression` names after
/// replaying its retained segments — the reference-world counterpart of the
/// position walk's member resolution. A call root resumes at the declared
/// return type, a stored-type leaf (a cast's normalized result, a zero
/// value's materialized reference) at its own stored reference, an array or
/// match leaf at its joined position, and an index hop at the element a
/// window was taken over — each through the same stored evidence
/// `expression_type_position` already proves. A declaration position rejoins
/// an existing named reference by its exact symbol, just as constructor result
/// typing does. An absent declaration row or a sliced position cannot mint a
/// type reference from a same-shaped row or an enclosing expected type.
pub fn expression_place_type_reference(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    segments: &[facts::PlaceSegment],
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let position = expression_type_position(program, expression)?;
    match member_position_after_segments(program, position, segments)? {
        MemberPosition::Reference(reference) => Some(reference),
        MemberPosition::Declaration(symbol) => program
            .type_reference_table
            .find_named_type_reference(symbol),
        MemberPosition::Sliced(_) => None,
    }
}

/// A match's value is whichever arm produces it, so every arm must agree on
/// one exact position — the same stored reference, declaration, or window
/// element. An opaque arm or a different stored row keeps the dispatch
/// unproven rather than letting one arm's leaf stand in for the others';
/// an arm spelled through a distinct handle to an equal type is that same
/// disagreement, not a join.
fn match_result_position(
    program: &dyn crate::CheckingInput,
    dispatch: &typed_trees::expression::TableMatchExpression,
) -> Option<MemberPosition> {
    let mut positions = program
        .expression_table
        .match_arms(dispatch.arms)
        .iter()
        .map(|arm| expression_type_position(program, arm.value));
    let Some(Some(first)) = positions.next() else {
        return None;
    };
    positions
        .all(|position| position == Some(first))
        .then_some(first)
}

pub fn effective_member_symbol(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    effective_member_symbol_from_position(
        program,
        receiver,
        expression_type_position(program, receiver),
        member,
    )
}

/// Resolve a member against the declaration supplied by a contextual place
/// owner, preserving the same case and retained-symbol checks as expression
/// type resolution.
pub fn effective_member_symbol_from_type_symbol(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
    type_symbol: SymbolHandle,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    effective_member_symbol_from_position(
        program,
        receiver,
        Some(MemberPosition::Declaration(type_symbol)),
        member,
    )
}

/// `effective_member_symbol` with the receiver's type position already
/// resolved — member-chain walks share one receiver walk between the symbol
/// answer and the position hop instead of resolving it once per use.
pub(super) fn effective_member_symbol_from_position(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
    receiver_position: Option<MemberPosition>,
    member: &typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if member.case_variant.is_none()
        && let Some(symbol) = closed_generic_member_symbol(
            program,
            receiver_position,
            member.member.as_str(),
            member.member_symbol,
        )
    {
        return symbol;
    }
    // A case-qualified projection must not select the first same-named field
    // in another case. Preserve the selected variant when reconstructing its
    // canonical place; an inconsistent retained symbol is not repairable here.
    if let Some(case_name) = &member.case_variant {
        let selected = receiver_position
            .map(|position| position_leaf_symbol(program, position))
            .and_then(|type_symbol| {
                let declaration = program.data_definition_by_symbol(type_symbol)?;
                let variant =
                    program
                        .data_members(declaration)
                        .iter()
                        .find_map(|row| match row {
                            typed_trees::data::DataMember::Variant(variant)
                                if variant.name == *case_name =>
                            {
                                Some(variant)
                            }
                            _ => None,
                        })?;
                program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name == member.member)
                    .map(|field| field.symbol)
            });
        return selected
            .filter(|symbol| !member.member_symbol.is_valid() || member.member_symbol == *symbol)
            .unwrap_or_else(SymbolHandle::invalid);
    }
    if let ExpressionNode::StructLiteral(literal) = program.expression_table.expression(receiver)
        && let Some(field) = program
            .expression_table
            .struct_fields(literal.fields)
            .iter()
            .find(|field| field.name == member.member)
        && field.field_symbol.is_valid()
    {
        return field.field_symbol;
    }

    if let Some(symbol) =
        resolve_member_symbol_from_position(program, receiver_position, member.member.as_str())
    {
        return symbol;
    }

    if member.member_symbol.is_valid() {
        return member.member_symbol;
    }

    SymbolHandle::invalid()
}

/// A case-qualified member hop resolves inside the selected variant only:
/// payload spellings repeat across cases, so the demanded field is the one
/// the named case declares — never the first same-named field a sibling
/// variant carries. This is the place-route counterpart of the selected-case
/// lookup `effective_member_symbol` performs when the receiver's type is
/// known; a variant that is absent or declares no such payload field yields
/// no symbol rather than borrowing a same-shaped row.
pub(super) fn resolve_case_member_symbol_from_type_symbol(
    program: &dyn crate::CheckingInput,
    type_symbol: SymbolHandle,
    case_name: &str,
    member_name: &str,
) -> Option<SymbolHandle> {
    let declaration = program.data_definition_by_symbol(type_symbol)?;
    let variant = program
        .data_members(declaration)
        .iter()
        .find_map(|row| match row {
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == case_name =>
            {
                Some(variant)
            }
            _ => None,
        })?;
    program
        .data_payload_fields(variant)
        .iter()
        .find(|field| field.name.as_str() == member_name)
        .map(|field| field.symbol)
}

pub fn resolve_member_symbol_from_type_symbol(
    program: &dyn crate::CheckingInput,
    type_symbol: SymbolHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    if let Some(data) = program.data_definition_by_symbol(type_symbol) {
        if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
            return Some(symbol);
        }
    }

    if let Some(machine) = program.machine_by_symbol(type_symbol) {
        if let Some(data) = program.attached_data_definition(machine) {
            if let Some(symbol) = data_member_symbol_by_name(program, data, member_name) {
                return Some(symbol);
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
    }

    None
}

fn resolve_member_symbol_from_position(
    program: &dyn crate::CheckingInput,
    receiver_position: Option<MemberPosition>,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = receiver_position.map(|position| position_leaf_symbol(program, position))?;
    resolve_member_symbol_from_type_symbol(program, type_symbol, member_name)
}

pub fn expression_type_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    expression_type_position(program, expression)
        .map(|position| position_leaf_symbol(program, position))
}

pub fn symbol_type_symbol(
    program: &dyn crate::CheckingInput,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    symbol_type_position(program, symbol).map(|position| position_leaf_symbol(program, position))
}

/// Member or payload field a name resolves to inside a data definition —
/// fields and variants share the namespace.
fn data_member_symbol_by_name(
    program: &dyn crate::CheckingInput,
    data: &typed_trees::data::DataDefinition,
    member_name: &str,
) -> Option<SymbolHandle> {
    for member in program.data_members(data) {
        match member {
            typed_trees::data::DataMember::Field(field) if field.name.as_str() == member_name => {
                return Some(field.symbol);
            }
            typed_trees::data::DataMember::Variant(variant)
                if variant.name.as_str() == member_name =>
            {
                return Some(variant.symbol);
            }
            typed_trees::data::DataMember::Variant(variant) => {
                if let Some(field) = program
                    .data_payload_fields(variant)
                    .iter()
                    .find(|field| field.name.as_str() == member_name)
                {
                    return Some(field.symbol);
                }
            }
            _ => {}
        }
    }
    None
}
