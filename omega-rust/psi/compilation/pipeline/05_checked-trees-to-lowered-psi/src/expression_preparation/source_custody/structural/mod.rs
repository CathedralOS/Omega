//! Reconstruct construction, selected ownership and ordered dispatch from authored nodes.
//! Establishing a local place does not necessarily establish a new value origin:
//! whole affine moves preserve the original provenance. Replay traces the authored
//! initializer chain separately from checking each place's establishment and transfer.
//!
//! `validate` rejoins an establishment to its destination, then `validate_value`
//! walks the retained value nodes as a worklist. Records, case
//! payloads and structural selections replay in `record`, `structural_case`
//! and `dispatch`, which queue their structural children back onto the same
//! worklist through `Replay`.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedStructuralValueHandle;
use checked_trees::expression::{ExpressionHandle, ExpressionNode, MatchPattern};
use checked_trees::statement::StatementNode;
use checked_trees::types::{PrimitiveType, TypeReferenceHandle};
use checked_trees::{CheckedScalarExpressionRole, CheckedTrees};
use symbols::SymbolHandle;

pub(crate) mod byte_literals;
mod dispatch;
mod local_disposition;
mod owned_places;
mod owned_selection;
mod record;
mod shared_borrow;
mod slice_view;
mod structural_case;
mod view_element_copy;

/// The exact authored value supplied by a source consumer, independently of
/// whether that consumer publishes a local destination. The caller rejoins
/// the registered root and declared type to its authored source before replay.
/// Operand calls belong to this value, and a selection receipt belongs to its
/// own destination, never merely to another value in the same statement.
pub(crate) struct StructuralValueSource {
    pub expression: ExpressionHandle,
    pub type_reference: TypeReferenceHandle,
    pub value: CheckedStructuralValueHandle,
}

/// Rejoin the retained occurrence to its authored destination or exact call formal.
fn authored_destination(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_index: u32,
    retained_expression: ExpressionHandle,
    operand_position: &Option<checked_trees::CheckedArrayConstructionSource>,
) -> Result<(ExpressionHandle, TypeReferenceHandle), LoweringError> {
    let (owner, source) = crate::declarations::state_declaration(checked, state)?;
    Ok(
        if let Some(checked_trees::CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            parameter_position,
        }) = operand_position
        {
            let call = crate::emission::call_source_custody::authored::locate_source(
                checked,
                state,
                checked_trees::CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal: *call_ordinal,
                },
            )?;
            let signature =
                crate::declarations::target_signature(checked, machine, call.source_target)?;
            let parameter = signature
                .parameters
                .get(*parameter_position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural construction lost its authored formal",
                ))?;
            let argument = call
                .structural_argument(*parameter_position, parameter.is_self)
                .filter(|argument| *argument == retained_expression)
                .ok_or(LoweringError::Unsupported(
                    "structural construction lost its authored destination",
                ))?;
            (argument, parameter.type_reference)
        } else {
            match checked
                .statement_table
                .statements(source.statement_nodes)
                .get(statement_index as usize)
            {
                // A `let mut` declaration without an initializer keys its
                // zeroed establishment on the invalid handle; the destination
                // type is the local's own declared type either way.
                Some(StatementNode::LocalData(local)) => {
                    (local.initial_value, local.type_reference)
                }
                Some(StatementNode::Expression(expression)) => (*expression, source.return_type),
                // `place = <construction>` establishes the value that replaces a
                // structural field; its destination type is the authored place's own
                // declared type, reconstructed here rather than read from the root.
                // Peeling a policy here would certify a different value type even
                // when the underlying storage shape is identical. The one shell
                // assignment itself removes is a reference: writing through
                // `referent: &mut T` replaces the referent, a `T`.
                Some(StatementNode::Assignment(assignment)) => (
                    assignment.value,
                    validation::declared_place_type_raw(
                        &checked.typed,
                        owner,
                        Some(source),
                        assignment.target,
                    )
                    .map(|destination| {
                        validation::assignment_value_type(&checked.typed, destination)
                    })
                    .ok_or(LoweringError::Unsupported(
                        "structural construction lost its authored destination",
                    ))?,
                ),
                Some(StatementNode::Transition(transition))
                    if transition.exit == checked_trees::statement::TransitionExit::Ordinary =>
                {
                    let mut selected = [transition.target, transition.continuation]
                        .into_iter()
                        .filter(|target| target.is_valid())
                        .filter_map(|target| {
                            match checked.statement_table.transition_target(target) {
                                checked_trees::statement::TransitionTargetNode::Value(
                                    expression,
                                ) if *expression == retained_expression => {
                                    Some((*expression, source.return_type))
                                }
                                checked_trees::statement::TransitionTargetNode::Named {
                                    path,
                                    arguments,
                                    ..
                                } => {
                                    let argument_position = checked
                                        .statement_table
                                        .expression_handles(*arguments)
                                        .iter()
                                        .position(|expression| {
                                            *expression == retained_expression
                                        })?;
                                    let target_state = checked
                                        .typed
                                        .machine_states(owner)
                                        .iter()
                                        .find(|state| state.symbol == path.symbol)?;
                                    let parameter = checked
                                        .typed
                                        .state_parameters(target_state)
                                        .iter()
                                        .filter(|parameter| !parameter.is_self)
                                        .nth(argument_position)?;
                                    Some((retained_expression, parameter.type_reference))
                                }
                                _ => None,
                            }
                        });
                    let destination = selected.next().ok_or(LoweringError::Unsupported(
                        "structural return exchanged its authored destination",
                    ))?;
                    if selected.next().is_some() {
                        return unsupported("structural return has ambiguous source destinations");
                    }
                    destination
                }
                _ => return unsupported("structural construction lost its authored destination"),
            }
        },
    )
}

/// Retain each authored operand beside the type declared by its exact formal.
/// Receiver elision changes argument positions, never the destination type.
fn append_argument_scopes(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    target: SymbolHandle,
    arguments: &[ExpressionHandle],
    receiver: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, TypeReferenceHandle)>,
) -> Result<(), LoweringError> {
    let signature = crate::declarations::target_signature(checked, machine, target)?;
    let nonself_count = signature
        .parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .count();
    let explicit_self = arguments.len() > nonself_count;
    if arguments.len()
        != if explicit_self {
            signature.parameters.len()
        } else {
            nonself_count
        }
    {
        return unsupported("structural operand scope changed its authored argument count");
    }
    let mut explicit = arguments.iter();
    for parameter in signature.parameters {
        let expression = if parameter.is_self && !explicit_self {
            if !receiver.is_valid() {
                continue;
            }
            receiver
        } else {
            *explicit.next().ok_or(LoweringError::Unsupported(
                "structural operand argument missing",
            ))?
        };
        pending.push((expression, parameter.type_reference));
    }
    Ok(())
}

/// Locate semantic operands of actual constructors and matches beneath this
/// value, including erased initializers. Relevance controls runtime emission,
/// not whether retained source computations owe exact authored custody.
/// A retained role cannot authorize reading an unrelated expression in the arena.
pub(crate) fn operand_source(
    checked: &CheckedTrees,
    state: SymbolHandle,
    statement: u32,
    role: CheckedScalarExpressionRole,
) -> Result<(ExpressionHandle, PrimitiveType), LoweringError> {
    let (machine, source) = crate::declarations::state_declaration(checked, state)?;
    let mut pending = Vec::new();
    match checked
        .statement_table
        .statements(source.statement_nodes)
        .get(statement as usize)
    {
        Some(StatementNode::LocalData(local)) => {
            pending.push((local.initial_value, local.type_reference));
        }
        Some(StatementNode::Assignment(assignment)) => pending.push(authored_destination(
            checked,
            machine.symbol,
            state,
            statement,
            assignment.value,
            &None,
        )?),
        Some(StatementNode::Expression(expression)) => {
            pending.push((*expression, source.return_type));
        }
        // A call statement's operands are its authored arguments; a
        // construction passed by value is evaluated there.
        Some(StatementNode::Call(call)) => append_argument_scopes(
            checked,
            machine.symbol,
            call.target_symbol,
            checked.statement_table.expression_handles(call.arguments),
            ExpressionHandle::invalid(),
            &mut pending,
        )?,
        Some(StatementNode::Transition(transition))
            if transition.exit == checked_trees::statement::TransitionExit::Ordinary =>
        {
            for target in [transition.target, transition.continuation]
                .into_iter()
                .filter(|target| target.is_valid())
            {
                match checked.statement_table.transition_target(target) {
                    checked_trees::statement::TransitionTargetNode::Value(expression) => {
                        pending.push((*expression, source.return_type));
                    }
                    checked_trees::statement::TransitionTargetNode::Named {
                        path,
                        arguments,
                        ..
                    } => append_argument_scopes(
                        checked,
                        machine.symbol,
                        path.symbol,
                        checked.statement_table.expression_handles(*arguments),
                        ExpressionHandle::invalid(),
                        &mut pending,
                    )?,
                    _ => {}
                }
            }
        }
        _ => return unsupported("structural operand has no authored value scope"),
    };
    // A computed receiver is an ordinary value consumer too. Rejoin its exact
    // registered operation edge before adding its declared value scope; this
    // also covers receivers nested beneath scalar arithmetic or other fields.
    for receiver in
        super::record_fields::field_receiver_sources(checked, machine.symbol, state, statement)?
    {
        pending.push((receiver.expression, receiver.type_reference));
    }
    let mut visited = Vec::new();
    while let Some((expression, declared)) = pending.pop() {
        if visited.contains(&expression) {
            return unsupported("structural operand has cyclic authored scope");
        }
        visited.push(expression);
        if !checked.expression_table.expression_is_valid(expression) {
            return unsupported("structural operand has stale authored scope");
        }
        if role == (CheckedScalarExpressionRole::RepresentationViewOffset { expression }) {
            let region =
                validation::slice_recast_byte_region(&checked.typed, machine, source, expression)
                    .ok_or(LoweringError::Unsupported(
                    "representation offset lost its authored recast",
                ))?;
            let primitive = validation::expression_result_type_reference(
                &checked.typed,
                machine,
                source,
                region.start,
            )
            .and_then(|reference| checked.primitive_type_reference(reference))
            .or_else(|| {
                matches!(
                    checked.expression_table.expression(region.start),
                    ExpressionNode::Integer(_)
                )
                .then_some(PrimitiveType::U64)
            })
            .ok_or(LoweringError::Unsupported(
                "representation offset lost its scalar carrier",
            ))?;
            return Ok((region.start, primitive));
        }
        if let CheckedScalarExpressionRole::StructuralArrayElement {
            expression: array,
            element_ordinal,
        } = role
            && array == expression
        {
            // The value's actual destination supplies its carrier whether it
            // is an argument, local, return or aggregate child. The array
            // owner alone reconstructs its ordered scalar leaf roster.
            let leaves = validation::scalar_array_elements(
                &checked.typed,
                machine.symbol,
                expression,
                declared,
            )
            .filter(|leaves| leaves.projections.is_empty())
            .ok_or(LoweringError::Unsupported(
                "scalar array operand lost its closed carrier",
            ))?;
            return leaves
                .elements
                .get(element_ordinal as usize)
                .copied()
                .ok_or(LoweringError::Unsupported(
                    "scalar array leaf ordinal escaped its array",
                ));
        }
        if let CheckedScalarExpressionRole::StructuralValueField {
            expression: constructor,
            field_ordinal,
        } = role
            && constructor == expression
        {
            // The constructor may carry structural payload siblings, so the
            // tolerant roster resolves first; the selected field still owes a
            // primitive carrier.
            let source =
                validation::structural_case_constructor(&checked.typed, expression, declared)
                    .ok_or(LoweringError::Unsupported(
                        "structural field lost its authored constructor",
                    ))?;
            return source
                .fields
                .get(field_ordinal as usize)
                .and_then(|(_, expression, declared)| {
                    checked
                        .primitive_type_reference(*declared)
                        .map(|primitive| (*expression, primitive))
                })
                .ok_or(LoweringError::Unsupported(
                    "structural field ordinal escaped its constructor",
                ));
        }
        if let CheckedScalarExpressionRole::RecordField {
            expression: owner,
            field_ordinal,
        } = role
            && owner == expression
            && let ExpressionNode::StructLiteral(literal) =
                checked.expression_table.expression(expression)
        {
            let field = checked
                .expression_table
                .struct_fields(literal.fields)
                .get(field_ordinal as usize)
                .ok_or(LoweringError::Unsupported("record operand ordinal missing"))?;
            let declaration = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == literal.type_symbol)
                .and_then(|data| {
                    checked
                        .data_members(data)
                        .iter()
                        .find_map(|member| match member {
                            checked_trees::data::DataMember::Field(declaration)
                                if declaration.symbol == field.field_symbol =>
                            {
                                Some(declaration)
                            }
                            _ => None,
                        })
                })
                .ok_or(LoweringError::Unsupported(
                    "record operand declaration missing",
                ))?;
            return checked
                .primitive_type_reference(declaration.type_reference)
                .map(|primitive| (field.value, primitive))
                .ok_or(LoweringError::Unsupported(
                    "record operand has no scalar carrier",
                ));
        }
        // An initializer or return call evaluates constructors in its own
        // receiver/arguments. Reach only those authored children before
        // comparing the retained constructor and field identities.
        if let ExpressionNode::Call(call) = checked.expression_table.expression(expression) {
            append_argument_scopes(
                checked,
                machine.symbol,
                call.target_symbol,
                checked.expression_table.expression_handles(call.arguments),
                call.receiver,
                &mut pending,
            )?;
            continue;
        }
        // A nested record's scalar fields belong to that constructor, not the
        // enclosing field's ordinal. Reach them through authored field values
        // before comparing the retained owner/ordinal above on the next visit.
        if let ExpressionNode::StructLiteral(literal) =
            checked.expression_table.expression(expression)
        {
            let data = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == literal.type_symbol)
                .ok_or(LoweringError::Unsupported(
                    "structural operand record declaration missing",
                ))?;
            for field in checked
                .expression_table
                .struct_fields(literal.fields)
                .iter()
                .rev()
            {
                let declaration = validation::exact_data_member_field(
                    &checked.typed,
                    data,
                    field.field_symbol,
                    field.name.as_str(),
                    literal.case_name.as_ref().map(|name| name.as_str()),
                )
                .ok_or(LoweringError::Unsupported(
                    "structural operand member declaration missing",
                ))?;
                pending.push((field.value, declaration.type_reference));
            }
            continue;
        }
        // Structural array children inherit the exact declared element type;
        // their own constructors retain their own operand identities.
        if let ExpressionNode::ArrayLiteral(elements) =
            checked.expression_table.expression(expression)
        {
            let reference = checked.typed.unwrapped_type_reference(declared).ok_or(
                LoweringError::Unsupported("structural array operand lost its type"),
            )?;
            let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } =
                checked.type_reference_table.type_reference(reference)
            else {
                return unsupported("structural array operand changed its carrier");
            };
            pending.extend(
                checked
                    .expression_table
                    .expression_handles(*elements)
                    .iter()
                    .rev()
                    .map(|expression| (*expression, *element_type)),
            );
            continue;
        }
        let ExpressionNode::Match(dispatch) = checked.expression_table.expression(expression)
        else {
            continue;
        };
        let primitive = validation::expression_result_type_reference(
            &checked.typed,
            machine,
            source,
            dispatch.subject,
        )
        .and_then(|reference| checked.typed.unwrapped_type_reference(reference))
        .and_then(|reference| checked.primitive_type_reference(reference))
        .or_else(|| validation::match_subject_primitive_type(&checked.typed, dispatch));
        if role == (CheckedScalarExpressionRole::StructuralValueSubject { expression }) {
            return primitive
                .map(|primitive| (dispatch.subject, primitive))
                .ok_or(LoweringError::Unsupported(
                    "structural subject has no scalar carrier",
                ));
        }
        for (ordinal, arm) in checked
            .expression_table
            .match_arms(dispatch.arms)
            .iter()
            .enumerate()
        {
            let source_arm = arena::Handle::from_parts(
                dispatch
                    .arms
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(ordinal).map_err(|_| {
                        LoweringError::Unsupported("structural arm ordinal overflow")
                    })?)
                    .ok_or(LoweringError::Unsupported(
                        "structural arm identity overflow",
                    ))?,
                dispatch.arms.start().generation(),
            );
            if role == (CheckedScalarExpressionRole::StructuralValuePattern { source_arm })
                && let MatchPattern::Value(pattern) = arm.pattern
            {
                return primitive.map(|primitive| (pattern, primitive)).ok_or(
                    LoweringError::Unsupported("structural pattern has no scalar carrier"),
                );
            }
            pending.push((arm.value, declared));
        }
    }
    unsupported("structural operand role escaped its authored value scope")
}

/// The referent of a shared-borrow `&T` result/local type: `T` must be a named
/// record or a primitive, independently of whether a record contains loans.
/// A shared borrow observes the referent instead of moving its contents;
/// contained loans and linear claims stay with the referent's owner and never
/// become owned obligations of this join. A
/// primitive referent has no data declaration to resolve; its structural
/// carrier is the `PrimitiveScalar` shape the type catalog already registers,
/// and the borrow still denotes the original storage rather than a snapshot.
/// `None` for owned types, `&mut` carriers (which keep their own
/// `reference_result_custody` lane), and sums. Shared by source replay and
/// attached-unit registration, so it lives here rather than in a machine
/// producer.
pub(crate) fn shared_borrow_record_referent(
    checked: &CheckedTrees,
    reference: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let checked_trees::types::TypeReferenceNode::Reference {
        referee, access, ..
    } = checked.type_reference_table.type_reference(reference)
    else {
        return None;
    };
    if *access != language_semantics::ReferenceAccess::Shared {
        return None;
    }
    if checked.primitive_type_reference(*referee).is_some() {
        return validation::has_linear_owned_contents(&checked.typed, *referee).then_some(*referee);
    }
    // `&[T]` names the slice view itself as the borrowed record: element custody
    // belongs to the view, so there are no plain owned contents to require.
    if let checked_trees::types::TypeReferenceNode::Slice { .. } =
        checked.type_reference_table.type_reference(*referee)
    {
        return Some(*referee);
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(*referee)
    else {
        return None;
    };
    let record = checked
        .data_definitions()
        .iter()
        .find(|record| record.symbol == *symbol)?;
    if checked
        .data_members(record)
        .iter()
        .any(|member| matches!(member, checked_trees::data::DataMember::Variant(_)))
    {
        return None;
    }
    Some(*referee)
}

/// The carrier a mutable slice binding joins. Both ByteSequence and ElementView
/// catalogs identify the slice beneath the reference shell; access and affine
/// descriptor debt are independently retained on its result. Named and primitive
/// referees keep their reference shell, so this is not generic dereferencing.
pub(crate) fn mutable_view_carrier(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    validation::slice_view_shell(&checked.typed, reference, &[])
        .filter(|view| view.access == language_semantics::ReferenceAccess::Mutable)
        .map(|view| view.slice)
}

/// The carrier of an actual shared slice view, including byte slices. This
/// queries source access independently of whether Terminal uses ByteSequence
/// or ElementView storage; mutable and write-only references cannot lend it.
pub(crate) fn shared_slice_view_referent(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    validation::slice_view_shell(&checked.typed, reference, &[])
        .filter(|view| view.access == language_semantics::ReferenceAccess::Shared)
        .map(|view| view.slice)
}

/// The type a view-binding `let` declares. Both authored spellings bind the
/// same slice: `let v: &[u8] = coll[a..b]` names the view directly, while
/// `let v: u8 = coll[a..b]` names only its element — the binding's declared
/// type is then the expression's own slice-view result, exactly as the
/// checker resolved it. A non-view declaration is returned unchanged.
pub(crate) fn declared_view_binding_type(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    declared: TypeReferenceHandle,
    initial_value: ExpressionHandle,
) -> TypeReferenceHandle {
    if validation::reference_shell(&checked.typed, declared).is_some() {
        return declared;
    }
    validation::expression_result_type_reference(&checked.typed, machine, state, initial_value)
        .filter(|result| validation::slice_view_shell(&checked.typed, *result, &[]).is_some())
        .unwrap_or(declared)
}
