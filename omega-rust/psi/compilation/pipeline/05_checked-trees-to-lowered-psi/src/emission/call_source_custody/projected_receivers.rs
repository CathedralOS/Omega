//! Rejoin implicit receiver operands to their exact authored parameter place.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::{TypeReferenceHandle, TypeReferenceNode};
use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment};
use symbols::SymbolHandle;

pub(crate) mod aliases;
mod locals;

pub(crate) use locals::declaration as owned_local_declaration;

pub(crate) struct ReceiverSource {
    pub(crate) root: SymbolHandle,
    pub(crate) path: Vec<CheckedUnitStructuralPathSegment>,
    pub(crate) stamp: SymbolHandle,
    /// Authored carrier and capture remain distinct from normalized self paths.
    owner: SymbolHandle,
    captured_place: checked_trees::CapturedPlace,
    erased_alias: bool,
    /// An erased alias (`let v = &mut place[...] as &mut T`) folds the view's
    /// declared type into the path without a segment. Each entry names the
    /// final-path position where that alias's declared referee takes over
    /// from the captured prefix's own type.
    alias_bridges: Vec<(usize, TypeReferenceHandle)>,
}

impl ReceiverSource {
    /// Source borrow facts use this captured spelling, while Terminal arguments
    /// use the normalized parameter root and typed field path.
    pub(crate) fn captured_place(&self) -> &checked_trees::CapturedPlace {
        &self.captured_place
    }

    /// Positions where a folded alias's declared referee replaces the type
    /// the captured prefix replay selected.
    pub(crate) fn alias_bridges(&self) -> &[(usize, TypeReferenceHandle)] {
        &self.alias_bridges
    }

    /// Storage is selected by `root`; the call captures the authored endpoint.
    /// Whole self may spell the machine namespace while its root is a formal.
    pub(crate) fn endpoint(&self) -> SymbolHandle {
        if self.stamp.is_valid() {
            self.stamp
        } else {
            self.owner
        }
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
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state)?;
    if machine.symbol != caller {
        return unsupported("projected receiver has a different authored caller");
    }
    let mut cursor = expression;
    let mut path = Vec::new();
    let mut visited = Vec::new();
    let mut stamp = SymbolHandle::invalid();
    let mut erased_alias = false;
    let mut alias_bridges = Vec::new();
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
                        let reference =
                            validation::unwrapped_type_reference(&checked.typed, reference)?;
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
                if cursor == expression {
                    stamp = field.symbol;
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
                if selected_case.is_none()
                    && matches!(checked.expression_table.expression(member.receiver), ExpressionNode::Name(name)
                    if name.symbol == machine.symbol && name.head_symbol == name.symbol)
                {
                    owner = field.symbol;
                    captured_root = field.symbol;
                    break checked
                        .state_parameters(state)
                        .iter()
                        .find(|parameter| parameter.is_self)
                        .ok_or(LoweringError::Unsupported(
                            "projected receiver has no borrowed self",
                        ))?
                        .symbol;
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
                if cursor == expression {
                    stamp = name.symbol;
                }
                if let Some(parameter) = checked.state_parameters(state).iter().find(|parameter| {
                    parameter.symbol == name.symbol
                        || (parameter.is_self && name.symbol == machine.symbol)
                }) {
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
                    erased_alias = true;
                    let alias_prefix = alias.path.len();
                    alias_bridges.extend(alias.alias_bridges);
                    if let Some((_, local)) = checked.state_local_declaration(state, name.symbol) {
                        let declared =
                            validation::reference_shell(&checked.typed, local.type_reference)
                                .map(|shell| shell.referee)
                                .unwrap_or(local.type_reference);
                        alias_bridges.push((alias_prefix, declared));
                    }
                    path.extend(alias.path.into_iter().rev());
                    captured_root = alias.captured_place.root_symbol;
                    captured_segments.extend(alias.captured_place.segments.into_iter().rev());
                    break alias.root;
                }
                let field = validation::exact_attached_field(
                    &checked.typed,
                    machine,
                    name.symbol,
                    checked.symbols.name(name.symbol),
                )
                .ok_or(LoweringError::Unsupported(
                    "projected receiver has no exact parameter root",
                ))?;
                if field.relevance.is_erased() {
                    return unsupported("projected receiver cannot select an erased root field");
                }
                path.push(field_segment(field));
                // Bare names capture their resolved inherited occurrence.
                // The exact data declaration normalizes the call path, not
                // the separately retained source loan root.
                captured_root = name.symbol;
                break checked
                    .state_parameters(state)
                    .iter()
                    .find(|parameter| parameter.is_self)
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver has no borrowed self",
                    ))?
                    .symbol;
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
        stamp,
        owner,
        captured_place: checked_trees::CapturedPlace {
            root_symbol: captured_root,
            segments: captured_segments,
        },
        erased_alias,
        alias_bridges,
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
            .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference))
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

/// Statement calls retain the root and endpoint symbols, with the intervening
/// field spellings. Resolve that suffix beneath the already-replayed alias
/// declaration using the same exact field owner as expression receivers.
fn statement_alias_source(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    call: &checked_trees::statement::TableCall,
    alias: ReceiverSource,
) -> Result<ReceiverSource, LoweringError> {
    let members = checked.statement_table.name_path_members(call.receiver);
    let (_, local) = checked
        .state_local_declaration(state, call.receiver_root_symbol)
        .ok_or(LoweringError::Unsupported(
            "receiver alias statement lost its declaration",
        ))?;
    if members.first() != Some(&local.name) || !call.receiver_symbol.is_valid() {
        return unsupported("receiver alias statement changed its root or endpoint identity");
    }
    let mut reference = local.type_reference;
    let mut endpoint = local.symbol;
    let mut path = alias.path;
    let mut captured_place = alias.captured_place;
    for (position, member) in members.iter().enumerate().skip(1) {
        let nominal = validation::unwrapped_type_reference(&checked.typed, reference).ok_or(
            LoweringError::Unsupported("receiver alias field lost its nominal type"),
        )?;
        let TypeReferenceNode::Named { symbol, .. } =
            checked.type_reference_table.type_reference(nominal)
        else {
            return unsupported("receiver alias field has no declared record owner");
        };
        let owner = checked
            .data_definitions()
            .iter()
            .find(|owner| owner.symbol == *symbol)
            .ok_or(LoweringError::Unsupported(
                "receiver alias field owner is absent",
            ))?;
        // Intermediate field symbols are not separately retained by TableCall.
        // Its exact nominal owner must select one field, and the final field
        // must additionally match the captured receiver endpoint.
        let retained = if position + 1 == members.len() {
            call.receiver_symbol
        } else {
            SymbolHandle::invalid()
        };
        let field = validation::exact_data_member_field(
            &checked.typed,
            owner,
            retained,
            member.as_str(),
            None,
        )
        .ok_or(LoweringError::Unsupported(
            "receiver alias field disagrees with its declaration",
        ))?;
        if field.relevance.is_erased() {
            return unsupported("receiver alias cannot select an erased field");
        }
        path.push(field_segment(field));
        captured_place.segments.push(facts::PlaceSegment::Field {
            symbol: field.symbol,
        });
        reference = field.type_reference;
        endpoint = field.symbol;
    }
    if endpoint != call.receiver_symbol {
        return unsupported("receiver alias statement changed its captured endpoint");
    }
    Ok(ReceiverSource {
        root: alias.root,
        path,
        stamp: endpoint,
        owner: local.symbol,
        captured_place,
        erased_alias: alias.erased_alias,
        alias_bridges: alias.alias_bridges,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate(
    checked: &CheckedTrees,
    caller_machine: SymbolHandle,
    caller_state: SymbolHandle,
    caller_operations: &[CheckedUnitEffectOperationPlan],
    caller_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    // Only ordinary calls rejoin a projected receiver here.
    let Some(planned) = operation.call().filter(|planned| !planned.is_boundary()) else {
        return Ok(());
    };
    let coordinate = planned.coordinate;
    let structural_arguments = &planned.structural_arguments;
    let authored = super::authored::locate_source(checked, caller_state, coordinate)?;
    let mut targets = target_parameters
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self);
    let Some((index, target)) = targets.next() else {
        // An unused receiver may be erased by the existing attachment planner.
        // No implicit operand remains to grant storage or borrow authority.
        return Ok(());
    };
    let source = match authored.source_site {
        Some(checked_trees::NominalMachineUseSite::Expression(expression)) => {
            let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
                return unsupported("receiver call lost its authored expression");
            };
            if !matches!(
                checked.expression_table.expression(call.receiver),
                ExpressionNode::Indexed(_) | ExpressionNode::Member(_) | ExpressionNode::Name(_)
            ) {
                return Ok(());
            }
            source(
                checked,
                caller_machine,
                caller_state,
                coordinate.statement_index as usize,
                call.receiver,
            )?
        }
        Some(checked_trees::NominalMachineUseSite::Statement(_)) => {
            let (_, state) = crate::expression_preparation::source_custody::authored_state(
                checked,
                caller_state,
            )?;
            let Some(checked_trees::statement::StatementNode::Call(call)) = checked
                .statement_table
                .statements(state.statement_nodes)
                .get(coordinate.statement_index as usize)
            else {
                return unsupported("receiver alias lost its authored statement call");
            };
            let local = locals::declaration(
                checked,
                state,
                coordinate.statement_index as usize,
                call.receiver_root_symbol,
            )
            .map(|_| ReceiverSource {
                root: call.receiver_root_symbol,
                path: Vec::new(),
                stamp: call.receiver_root_symbol,
                owner: call.receiver_root_symbol,
                captured_place: checked_trees::CapturedPlace {
                    root_symbol: call.receiver_root_symbol,
                    segments: Vec::new(),
                },
                erased_alias: false,
                alias_bridges: Vec::new(),
            });
            let alias = if local.is_some() {
                local
            } else {
                aliases::parameter_source(
                    checked,
                    caller_machine,
                    caller_state,
                    coordinate.statement_index as usize,
                    call.receiver_root_symbol,
                    false,
                )?
            };
            let Some(alias) = alias else {
                return Ok(());
            };
            statement_alias_source(checked, state, call, alias)?
        }
        None => return Ok(()),
    };
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    if let Some((ordinal, local)) = locals::returned_reference(
        checked,
        state,
        coordinate.statement_index as usize,
        source.root,
    ) {
        if targets.next().is_some() {
            return unsupported("returned-reference receiver selected multiple self parameters");
        }
        let argument = structural_arguments
            .get(index)
            .ok_or(LoweringError::Unsupported(
                "returned-reference receiver argument missing",
            ))?;
        return locals::validate_returned_reference(
            checked,
            caller_machine,
            caller_state,
            caller_operations,
            coordinate,
            planned.target.state,
            authored.source_target,
            target,
            argument,
            &source,
            ordinal,
            local,
        );
    }
    if locals::declaration(
        checked,
        state,
        coordinate.statement_index as usize,
        source.root,
    )
    .is_some()
    {
        if targets.next().is_some() {
            return unsupported("local receiver selected multiple self parameters");
        }
        let argument = structural_arguments
            .get(index)
            .ok_or(LoweringError::Unsupported(
                "local receiver argument missing",
            ))?;
        return locals::validate(
            checked,
            caller_machine,
            caller_state,
            caller_operations,
            coordinate,
            authored.source_target,
            target,
            argument,
            &source,
        );
    }
    let position = checked
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == source.root)
        .ok_or(LoweringError::Unsupported(
            "projected receiver lost its source parameter",
        ))?;
    let parameter = caller_parameters
        .iter()
        .position(|parameter| parameter.position as usize == position)
        .ok_or(LoweringError::Unsupported(
            "projected receiver lost its retained parameter",
        ))?;
    let argument = structural_arguments
        .get(index)
        .ok_or(LoweringError::Unsupported(
            "projected receiver operand is absent",
        ))?;
    if targets.next().is_some()
        || (source.erased_alias
            && target.access != checked_trees::CheckedStructuralAccess::WriteOnlyBorrow)
        || argument.source_parameter_index() != u32::try_from(parameter).ok()
        || argument.path != source.path
        || argument.type_identity != target.type_identity
        || argument.access != target.access
    {
        return unsupported("projected receiver operand disagrees with its authored place");
    }
    Ok(())
}
