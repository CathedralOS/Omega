//! Rejoin implicit receiver operands to their exact authored parameter place.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::CheckedUnitStructuralPathSegment;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::TypeReferenceNode;
use symbols::SymbolHandle;

pub(crate) mod aliases;
mod locals;

pub(crate) use locals::declaration as owned_local_declaration;

pub(crate) struct ReceiverSource {
    pub(crate) root: SymbolHandle,
    pub(crate) path: Vec<CheckedUnitStructuralPathSegment>,
    /// Authored carrier and capture remain distinct from normalized self paths.
    owner: SymbolHandle,
    captured_place: checked_trees::CapturedPlace,
}

impl ReceiverSource {
    /// Source borrow facts use this captured spelling, while Terminal arguments
    /// use the normalized parameter root and typed field path.
    pub(crate) fn captured_place(&self) -> &checked_trees::CapturedPlace {
        &self.captured_place
    }
}

/// Captured source spelling is independent of the authority to materialize a
/// receiver or store. Only those storage uses normalize borrowed aliases.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SourceUse {
    Receiver,
    Store,
    Observation,
}

/// Reconstruct an observation's original place without granting storage or
/// alias authority. Keep the result confined to the captured-place vocabulary.
pub(crate) fn observation_place(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Result<checked_trees::CapturedPlace, LoweringError> {
    Ok(resolve_source(
        checked,
        caller,
        state,
        expression,
        SourceUse::Observation,
        Some((statement_index, false)),
    )?
    .captured_place)
}

pub(crate) fn source(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Result<ReceiverSource, LoweringError> {
    resolve_source(
        checked,
        caller,
        state,
        expression,
        SourceUse::Receiver,
        Some((statement_index, false)),
    )
}

/// Assignment targets need no terminal Member stamp. A local destination also
/// needs its statement coordinate, so later declarations cannot supply storage.
pub(crate) fn store_destination(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    statement: Option<usize>,
    expression: ExpressionHandle,
) -> Result<ReceiverSource, LoweringError> {
    resolve_source(
        checked,
        caller,
        state,
        expression,
        SourceUse::Store,
        statement.map(|ordinal| (ordinal, false)),
    )
}

fn resolve_source(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
    purpose: SourceUse,
    statement_index: Option<(usize, bool)>,
) -> Result<ReceiverSource, LoweringError> {
    let (machine, state) = crate::declarations::state_declaration(checked, state)?;
    if machine.symbol != caller {
        return unsupported("projected receiver has a different authored caller");
    }
    let mut cursor = expression;
    let mut path = Vec::new();
    let mut visited = Vec::new();
    let mut captured_segments = Vec::new();
    let owner;
    let captured_root;
    let root = loop {
        if !checked.expression_table.expression_is_valid(cursor) || visited.contains(&cursor) {
            return unsupported("projected receiver has a stale or cyclic source");
        }
        visited.push(cursor);
        match checked.expression_table.expression(cursor) {
            ExpressionNode::Member(member)
                if purpose == SourceUse::Observation
                    && validation::collection_length_receiver(
                        &checked.typed,
                        machine,
                        Some(state),
                        cursor,
                    ) == Some(member.receiver) =>
            {
                // A collection extent is an observation, not a stored field.
                // Preserve the borrow ledger's exact pseudo-member coordinate;
                // no receiver/store path may acquire authority through it.
                captured_segments.push(facts::PlaceSegment::Field {
                    symbol: member.member_symbol,
                });
                cursor = member.receiver;
            }

            ExpressionNode::Member(member)
                if (purpose != SourceUse::Receiver
                    || cursor != expression
                    || member.case_variant.is_some()
                    || member.member_symbol.is_valid()) =>
            {
                let field = validation::exact_self_field(&checked.typed, machine, cursor)
                    .or_else(|| {
                        let reference = validation::declared_place_type_raw(
                            &checked.typed,
                            machine,
                            Some(state),
                            member.receiver,
                        )?;
                        let reference = checked.typed.unwrapped_type_reference(reference)?;
                        let owner = checked.typed.closed_data_definition(reference)?;
                        validation::exact_data_member_field(
                            &checked.typed,
                            owner,
                            member.member_symbol,
                            member.member.as_str(),
                            member.case_variant.as_ref().map(|case| case.as_str()),
                        )
                        .or_else(|| {
                            if member.case_variant.is_some() {
                                return None;
                            }
                            // Checked places select the concrete field, while
                            // authored members may retain the template symbol.
                            // Replay the exact correspondence, never a same-name
                            // field from another application or declaration.
                            checked.typed.closed_data_member_field(
                                reference,
                                member.member_symbol,
                                member.member.as_str(),
                            )
                        })
                    })
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver field has no exact declaration",
                    ))?;
                if field.relevance.is_erased() {
                    return unsupported("projected receiver cannot select an erased field");
                }
                path.push(field_segment(field));
                let selected_case = if member.case_variant.is_some() {
                    Some(crate::expression_preparation::source_custody::structural_places::payload_case(
                        checked, field.symbol,
                    )?)
                } else {
                    None
                };
                if let Some(case) = selected_case {
                    path.push(CheckedUnitStructuralPathSegment::Case(case.path_identity()));
                }
                captured_segments.push(facts::PlaceSegment::Field {
                    symbol: field.symbol,
                });
                if let Some(case) = selected_case {
                    captured_segments.push(facts::PlaceSegment::Case {
                        variant: case.symbol,
                    });
                }
                // Replay implicit dereferences from the receiver's declared
                // type, not from the producer's path. Root parameters already
                // denote borrowed storage; a projected reference field stores
                // a carrier that must be crossed before this field access.
                if !matches!(
                    checked.expression_table.expression(member.receiver),
                    ExpressionNode::Name(_)
                ) {
                    let mut reference = validation::declared_place_type_raw(
                        &checked.typed,
                        machine,
                        Some(state),
                        member.receiver,
                    )
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver lost its carrier type",
                    ))?;
                    while let Some(shell) = validation::reference_shell(&checked.typed, reference) {
                        path.push(CheckedUnitStructuralPathSegment::Referent);
                        reference = shell.referee;
                    }
                }
                cursor = member.receiver;
            }
            // An assignment destination's runtime selector is the statement's
            // `AssignmentIndex { depth }` scalar, numbered along the whole
            // assignment target: the store evaluates it, and Terminal re-proves
            // the element's bound.
            ExpressionNode::Indexed(indexed)
                if purpose != SourceUse::Receiver
                    && !matches!(
                        checked.expression_table.expression(indexed.index),
                        ExpressionNode::Integer(_) | ExpressionNode::Range(_)
                    )
                    // The checker's place facts fold a constant expression
                    // (`cells[3 - 1]`) to its element; a constant local's
                    // name stays the statement's runtime selector.
                    && checked
                        .expression_table
                        .constant_integer_value(indexed.index)
                        .is_none()
                    && let Some(depth) = assignment_selector_depth(
                        checked,
                        state,
                        statement_index,
                        indexed.index,
                    ) =>
            {
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(state),
                    cursor,
                ) {
                    return unsupported("store destination index has no builtin address meaning");
                }
                validate_indexed_collection(
                    checked,
                    machine,
                    state,
                    indexed.collection,
                    None,
                    purpose,
                )?;
                path.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth },
                ));
                captured_segments.push(facts::PlaceSegment::Index {
                    expression: indexed.index,
                });
                cursor = indexed.collection;
            }
            // A receiver's runtime selector is the calling state's own dense
            // scalar parameter: the value Terminal names is the immutable
            // integer operand at that position, and Terminal re-proves its
            // bound against the fixed array it selects in.
            ExpressionNode::Indexed(indexed)
                if let ExpressionNode::Name(name) =
                    checked.expression_table.expression(indexed.index)
                    && name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1
                    && let Some(position) = checked
                        .state_parameters(state)
                        .iter()
                        .position(|parameter| parameter.symbol == name.symbol)
                        .filter(|position| {
                            let parameter = &checked.state_parameters(state)[*position];
                            !parameter.is_mutable
                                && matches!(
                                    checked.primitive_type_reference(parameter.type_reference),
                                    Some(
                                        checked_trees::types::PrimitiveType::U8
                                            | checked_trees::types::PrimitiveType::U16
                                            | checked_trees::types::PrimitiveType::U32
                                            | checked_trees::types::PrimitiveType::U64
                                            | checked_trees::types::PrimitiveType::I8
                                            | checked_trees::types::PrimitiveType::I16
                                            | checked_trees::types::PrimitiveType::I32
                                            | checked_trees::types::PrimitiveType::I64
                                    )
                                )
                        })
                        .map(|position| {
                            checked.state_parameters(state)[..position]
                                .iter()
                                .filter(|parameter| {
                                    !parameter.relevance.is_erased()
                                        && checked
                                            .primitive_type_reference(parameter.type_reference)
                                            .is_some()
                                })
                                .count()
                        })
                        .and_then(|position| u32::try_from(position).ok()) =>
            {
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(state),
                    cursor,
                ) {
                    return unsupported("projected receiver index has no builtin address meaning");
                }
                validate_indexed_collection(
                    checked,
                    machine,
                    state,
                    indexed.collection,
                    None,
                    purpose,
                )?;
                path.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::Parameter { position },
                ));
                captured_segments.push(facts::PlaceSegment::Index {
                    expression: indexed.index,
                });
                cursor = indexed.collection;
            }
            // A field selector is read when this structural operand is
            // evaluated. Replay its source coordinate here; the operand
            // emitter owns the resulting snapshot and its bounds obligation.
            ExpressionNode::Indexed(indexed)
                if !matches!(
                    checked.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) && fixed_index(checked, state, indexed.index).is_none() =>
            {
                let selector = crate::expression_preparation::source_custody::structural_places::field_selector(
                    checked,
                    machine,
                    state,
                    indexed.index,
                )?;
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(state),
                    cursor,
                ) {
                    return unsupported("projected receiver index has no builtin address meaning");
                }
                validate_indexed_collection(
                    checked,
                    machine,
                    state,
                    indexed.collection,
                    None,
                    purpose,
                )?;
                path.push(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::StructuralField(selector),
                ));
                captured_segments.push(facts::PlaceSegment::Index {
                    expression: indexed.index,
                });
                cursor = indexed.collection;
            }
            ExpressionNode::Indexed(indexed) => {
                // Every selector `fixed_index` does not fold took a
                // runtime-selector arm above.
                let index = fixed_index(checked, state, indexed.index).ok_or(
                    LoweringError::Unsupported("projected receiver requires a literal fixed index"),
                )?;
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(state),
                    cursor,
                ) {
                    return unsupported("projected receiver index has no builtin address meaning");
                }
                validate_indexed_collection(
                    checked,
                    machine,
                    state,
                    indexed.collection,
                    Some(index),
                    purpose,
                )?;
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                // Materialization may fold an immutable local's value, but
                // capture retains that local read's exact selector occurrence.
                // Only a source-constant expression becomes a fixed capture.
                captured_segments.push(
                    match checked
                        .expression_table
                        .constant_integer_value(indexed.index)
                    {
                        Some(value) => facts::PlaceSegment::FixedIndex {
                            index: usize::try_from(value).map_err(|_| {
                                LoweringError::Unsupported("projected receiver index exceeds usize")
                            })?,
                        },
                        None => facts::PlaceSegment::Index {
                            expression: indexed.index,
                        },
                    },
                );
                cursor = indexed.collection;
            }
            ExpressionNode::Name(name)
                if name.symbol.is_valid()
                    && name.head_symbol == name.symbol
                    && checked
                        .expression_table
                        .name_path_members(name.members)
                        .len()
                        == 1 =>
            {
                if checked
                    .expression_table
                    .name_path_member_symbols(name.member_symbols)
                    .first()
                    .is_some_and(|symbol| *symbol != name.symbol)
                {
                    return unsupported("projected receiver root identity changed");
                }
                owner = name.symbol;
                if let Some(parameter) = checked
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.symbol == name.symbol)
                {
                    captured_root = name.symbol;
                    break parameter.symbol;
                }
                if purpose == SourceUse::Observation
                    && let Some((statement_index, _)) = statement_index
                    && checked
                        .state_local_declaration(state, name.symbol)
                        .is_some_and(|(ordinal, _)| ordinal < statement_index)
                {
                    captured_root = name.symbol;
                    break name.symbol;
                }
                if let Some((statement_index, _)) = statement_index
                    && locals::declaration(checked, state, statement_index, name.symbol).is_some()
                {
                    captured_root = name.symbol;
                    break name.symbol;
                }
                // An established borrowed carrier is observed under its own
                // symbol. Mutable slices retain their descriptor here, while
                // nominal mutable aliases keep the backing replay below.
                // Stores and alias formation do not gain carrier authority.
                if purpose == SourceUse::Receiver
                    && let Some((statement_index, _)) = statement_index
                    && locals::retained_borrowed_carrier(
                        checked,
                        state,
                        statement_index,
                        name.symbol,
                    )
                    .is_some()
                {
                    captured_root = name.symbol;
                    break name.symbol;
                }
                if let Some((statement_index, formation)) = statement_index
                    && let Some(alias) = aliases::parameter_source(
                        checked,
                        machine.symbol,
                        state.symbol,
                        statement_index,
                        name.symbol,
                        formation,
                    )?
                {
                    path.extend(alias.path.into_iter().rev());
                    captured_root = alias.captured_place.root_symbol;
                    captured_segments.extend(alias.captured_place.segments.into_iter().rev());
                    break alias.root;
                }
                return unsupported("projected receiver has no exact parameter root");
            }
            // A spelled recast or borrow is not the projected source itself:
            // the receiver is the borrowed place's own source. Walk through
            // the `as` wrapper and the `&` to the place they re-view; the
            // loan's own ledger proves the access it established.
            ExpressionNode::Cast(cast) if cast.form.is_recast() => {
                cursor = cast.value;
            }
            ExpressionNode::Borrow(borrow) => {
                cursor = borrow.target;
            }
            _ => {
                return unsupported(
                    "projected receiver is not a content-independent parameter place",
                );
            }
        }
    };
    path.reverse();
    captured_segments.reverse();
    Ok(ReceiverSource {
        root,
        path,
        owner,
        captured_place: checked_trees::CapturedPlace {
            root_symbol: captured_root,
            segments: captured_segments,
        },
    })
}

/// Fixed arrays own a declared extent; element views own a live bound witness.
/// Selector spelling changes neither collection identity nor addressability.
/// This source join grants no bound or mutation permission: the operation's
/// independent storage/loan and index obligations still establish those.
fn validate_indexed_collection(
    checked: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
    state: &checked_trees::state::State,
    collection: ExpressionHandle,
    fixed_index: Option<u64>,
    purpose: SourceUse,
) -> Result<(), LoweringError> {
    let reference =
        validation::declared_place_type_raw(&checked.typed, machine, Some(state), collection)
            .and_then(|reference| checked.typed.unwrapped_type_reference(reference))
            .ok_or(LoweringError::Unsupported(
                "projected index has no declared collection",
            ))?;
    match checked.type_reference_table.type_reference(reference) {
        TypeReferenceNode::FixedArray {
            length: checked_trees::types::FixedArrayLength::Literal(length),
            ..
        } => {
            if fixed_index.is_some_and(|index| {
                usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
            }) {
                return unsupported("projected receiver index is out of bounds");
            }
        }
        // Observing a byte element retains the same captured selector. Its
        // executable storage operation remains separate from receiver/store
        // materialization, which still excludes the byte-view carrier.
        TypeReferenceNode::Slice { element_type }
            if purpose == SourceUse::Observation
                || checked.primitive_type_reference(*element_type)
                    != Some(checked_trees::types::PrimitiveType::U8) => {}
        _ => {
            return unsupported("projected index has neither fixed-array nor element-view storage");
        }
    }
    Ok(())
}

/// The element ordinal a fixed selector denotes: an integer literal, an
/// immutable local bound to a constant, or a constant integer expression such
/// as `3 - 1`. The checker's place facts fold that last form the same way, so
/// the checked path names `FixedIndex(2)` rather than a runtime selector.
fn fixed_index(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    index: ExpressionHandle,
) -> Option<u64> {
    match checked.expression_table.expression(index) {
        ExpressionNode::Integer(index) => index.value_bignum().and_then(|value| value.to_u64()),
        ExpressionNode::Name(name) => {
            crate::expression_preparation::source_custody::structural_places::constant_local_index(
                checked,
                state,
                name.symbol,
            )
        }
        _ => checked
            .expression_table
            .constant_integer_value(index)
            .and_then(|value| u64::try_from(value).ok()),
    }
}

/// The `AssignmentIndex` depth of `index` among the selectors of the
/// statement's assignment target, when the statement is an assignment whose
/// target selects through it.
fn assignment_selector_depth(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    statement_index: Option<(usize, bool)>,
    index: ExpressionHandle,
) -> Option<u32> {
    let (statement_index, _) = statement_index?;
    let checked_trees::statement::StatementNode::Assignment(assignment) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?
    else {
        return None;
    };
    let depth = validation::assignment_target_selectors(&checked.typed, assignment.target)
        .iter()
        .position(|selector| *selector == index)?;
    u32::try_from(depth).ok()
}

fn field_segment(field: &checked_trees::data::DataField) -> CheckedUnitStructuralPathSegment {
    CheckedUnitStructuralPathSegment::Field(field.path_identity())
}
