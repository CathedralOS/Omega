//! Rejoin implicit receiver operands to their exact authored parameter place.

use crate::{CheckedTrees, LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedUnitEffectMachinePlan, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};
use symbols::SymbolHandle;

mod aliases;

pub(crate) struct ReceiverSource {
    pub(crate) root: SymbolHandle,
    pub(crate) path: Vec<CheckedUnitStructuralPathSegment>,
    pub(crate) stamp: SymbolHandle,
    /// Authored carrier and capture remain distinct from normalized self paths.
    owner: SymbolHandle,
    captured_place: checked_trees::CapturedPlace,
    erased_alias: bool,
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
        true,
        Some((statement_index, false)),
    )
}

/// Assignment targets can retain a declared parameter field without stamping
/// the terminal Member node. Calls still require their captured endpoint stamp.
pub(crate) fn store_destination(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
) -> Result<ReceiverSource, LoweringError> {
    resolve_source(checked, caller, state, expression, false, None)
}

fn resolve_source(
    checked: &CheckedTrees,
    caller: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
    require_endpoint_stamp: bool,
    statement_index: Option<(usize, bool)>,
) -> Result<ReceiverSource, LoweringError> {
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, state)?;
    if machine.symbol != caller {
        return unsupported("projected receiver has a different authored caller");
    }
    let mut cursor = expression;
    let mut path = Vec::new();
    let mut visited = Vec::new();
    let mut stamp = SymbolHandle::invalid();
    let mut erased_alias = false;
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
                if member.case_variant.is_none()
                    && (!require_endpoint_stamp
                        || cursor != expression
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
                        let TypeReferenceNode::Named { symbol, .. } =
                            checked.type_reference_table.type_reference(reference)
                        else {
                            return None;
                        };
                        let owner = checked
                            .data_definitions()
                            .iter()
                            .find(|owner| owner.symbol == *symbol)?;
                        validation::exact_data_member_field(
                            &checked.typed,
                            owner,
                            member.member_symbol,
                            member.member.as_str(),
                            None,
                        )
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
                if matches!(checked.expression_table.expression(member.receiver), ExpressionNode::Name(name)
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
                cursor = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                let ExpressionNode::Integer(index) =
                    checked.expression_table.expression(indexed.index)
                else {
                    return unsupported("projected receiver requires a literal fixed index");
                };
                let index = index
                    .value_bignum()
                    .and_then(|value| value.to_u64())
                    .ok_or(LoweringError::Unsupported(
                        "projected receiver index exceeds u64",
                    ))?;
                if !validation::place_has_builtin_coordinates(
                    &checked.typed,
                    machine,
                    Some(state),
                    cursor,
                ) {
                    return unsupported("projected receiver index has no builtin address meaning");
                }
                let reference = validation::declared_place_type_raw(
                    &checked.typed,
                    machine,
                    Some(state),
                    indexed.collection,
                )
                .and_then(|reference| {
                    validation::unwrapped_type_reference(&checked.typed, reference)
                })
                .ok_or(LoweringError::Unsupported(
                    "projected receiver index has no declared collection",
                ))?;
                let TypeReferenceNode::FixedArray {
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                    ..
                } = checked.type_reference_table.type_reference(reference)
                else {
                    return unsupported("projected receiver index has no literal array length");
                };
                if usize::try_from(index)
                    .ok()
                    .is_none_or(|index| index >= *length)
                {
                    return unsupported("projected receiver index is out of bounds");
                }
                path.push(CheckedUnitStructuralPathSegment::FixedIndex(index));
                captured_segments.push(facts::PlaceSegment::FixedIndex {
                    index: usize::try_from(index).map_err(|_| {
                        LoweringError::Unsupported("projected receiver index exceeds usize")
                    })?,
                });
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
                if let Some(parameter) = checked.state_parameters(state).iter().find(|parameter| {
                    parameter.symbol == name.symbol
                        || (parameter.is_self && name.symbol == machine.symbol)
                }) {
                    captured_root = name.symbol;
                    break parameter.symbol;
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
    })
}

fn field_segment(field: &checked_trees::data::DataField) -> CheckedUnitStructuralPathSegment {
    CheckedUnitStructuralPathSegment::Field(
        field
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| field.name.as_str().to_owned()),
    )
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
    let local = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if local.symbol == call.receiver_root_symbol =>
            {
                Some(local)
            }
            _ => None,
        })
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
        erased_alias: true,
    })
}

pub(crate) fn validate(
    checked: &CheckedTrees,
    caller: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    target_parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
) -> Result<(), LoweringError> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        structural_arguments,
        ..
    } = operation
    else {
        return Ok(());
    };
    let authored = super::authored::locate_source(checked, caller.state, *coordinate)?;
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
                caller.machine,
                caller.state,
                coordinate.statement_index as usize,
                call.receiver,
            )?
        }
        Some(checked_trees::NominalMachineUseSite::Statement(_)) => {
            let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
            let Some(checked_trees::statement::StatementNode::Call(call)) = checked
                .statement_table
                .statements(state.statement_nodes)
                .get(coordinate.statement_index as usize)
            else {
                return unsupported("receiver alias lost its authored statement call");
            };
            let Some(alias) = aliases::parameter_source(
                checked,
                caller.machine,
                caller.state,
                coordinate.statement_index as usize,
                call.receiver_root_symbol,
                false,
            )?
            else {
                return Ok(());
            };
            statement_alias_source(checked, state, call, alias)?
        }
        None => return Ok(()),
    };
    let (_, state) = crate::scalar_source_custody::authored_state(checked, caller.state)?;
    let position = checked
        .state_parameters(state)
        .iter()
        .position(|parameter| parameter.symbol == source.root)
        .ok_or(LoweringError::Unsupported(
            "projected receiver lost its source parameter",
        ))?;
    let parameter = caller
        .structural_parameters
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
