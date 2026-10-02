//! Direct byte loans retain their authored field, array, or borrowed-view root.
//! Derived range descriptors belong to ordered argument staging, whose shared
//! range owner replays their source and endpoints before producing the operand.

use super::{authored, projected_receivers};
use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use checked_trees::CheckedTrees;
use checked_trees::expression::ExpressionNode;
use checked_trees::types::{PrimitiveType, TypeReferenceNode};
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitCallCoordinate, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
};
use symbols::SymbolHandle;

#[cfg(test)]
mod tests;

/// Return the operand ordinals whose complete storage projection was replayed.
/// A byte window is an adapted view, not a plain storage projection: the common
/// call owner consumes this result instead of asking another path reader to
/// reinterpret the range. Unstaged direct loans and staged descriptors retain
/// their independent source, access and bounds checks.
pub(super) fn validate(
    checked: &CheckedTrees,
    caller_state: SymbolHandle,
    caller_parameters: &[CheckedUnitStructuralParameterPlan],
    coordinate: CheckedUnitCallCoordinate,
    call: &authored::AuthoredCall,
    operation: &CheckedUnitEffectOperationPlan,
) -> Result<Vec<usize>, LoweringError> {
    let mut replayed = Vec::new();
    let Some(planned) = operation.call() else {
        return Ok(replayed);
    };
    let arguments = &planned.structural_arguments;
    let (machine, state) =
        crate::expression_preparation::source_custody::authored_state(checked, caller_state)?;
    let signature = authored::target_signature(checked, machine.symbol, call.source_target)?;
    let byte_view_access = |parameter: &checked_trees::signature::StateParameter| {
        let TypeReferenceNode::Reference {
            access, referee, ..
        } = checked
            .type_reference_table
            .type_reference(parameter.type_reference)
        else {
            return None;
        };
        let TypeReferenceNode::Slice { element_type } =
            checked.type_reference_table.type_reference(*referee)
        else {
            return None;
        };
        (checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
            && matches!(
                access,
                language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
            ))
        .then_some(*access)
    };
    if !signature
        .parameters
        .iter()
        .any(|parameter| byte_view_access(parameter).is_some())
    {
        return Ok(replayed);
    }
    let positions = crate::unit::attached_unit::call_parameters::structural_positions(
        checked, planned, &signature,
    )?;
    for (ordinal, (position, argument)) in positions.into_iter().zip(arguments).enumerate() {
        // Both ordinary and composed call schedules send these operands through
        // byte_subslices::emit: locate rejoins the authored argument and access,
        // then view_ranges checks storage identity, operator meaning and bound
        // evidence. A consumer without that staging rejects the missing operand
        // place. Reinterpreting the descriptor here as a direct backing loan
        // would demand a different source and discard its endpoint evidence.
        if crate::unit::attached_unit::view_ranges::range(argument).is_some() {
            continue;
        }
        let Some(expected_access) = byte_view_access(&signature.parameters[position as usize])
        else {
            continue;
        };
        let shared = expected_access == language_core::ReferenceAccess::Shared;
        let fixed_window = argument.path.iter().any(|segment| {
            matches!(
                segment,
                checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { .. }
            )
        });
        let expression = call
            .structural_arguments
            .iter()
            .find(|(source_position, _)| *source_position == position)
            .map(|(_, expression)| *expression)
            .ok_or(LoweringError::Unsupported(
                "boundary byte loan has no authored argument",
            ))?;
        let (target, access) = match checked.expression_table.expression(expression) {
            ExpressionNode::Borrow(borrow) => (borrow.target, borrow.access),
            // Forwarding retains the formal's access; exact source custody
            // below and structural replay still reject access escalation.
            ExpressionNode::Name(_) => (expression, expected_access),
            ExpressionNode::Indexed(_) if shared => (expression, expected_access),
            _ if shared && !fixed_window => continue,
            _ => return unsupported("boundary byte loan lost its authored borrow"),
        };
        let (backing, range) = match checked.expression_table.expression(target) {
            ExpressionNode::Indexed(indexed) => {
                match checked.expression_table.expression(indexed.index) {
                    ExpressionNode::Range(range) => (indexed.collection, Some(range)),
                    _ => (target, None),
                }
            }
            _ => (target, None),
        };
        let declared_backing =
            validation::declared_place_type_raw(&checked.typed, machine, Some(state), backing);
        // A qualified bounded owner has a live extent, despite spelling its
        // source carrier as an array. It is not a plain fixed backing or a
        // reference alias. Result-use replay still joins its binding ordinal
        // to the exact establishment; here replay the authored whole-place
        // loan and its captured shared access.
        if shared
            && !fixed_window
            && range.is_none()
            && declared_backing.is_some_and(|reference| {
                validation::bounded_byte_buffer_capacity(&checked.typed, reference).is_some()
            })
            && matches!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. }
            )
        {
            validate_owned_byte_loan(
                checked,
                machine.symbol,
                state,
                coordinate,
                call.source_target,
                backing,
                access,
                argument,
            )?;
            continue;
        }
        let backing_type = declared_backing
            .and_then(|reference| validation::unwrapped_type_reference(&checked.typed, reference));
        // Existing shared literals and live view descriptors have their own
        // source-custody rules. This owner additionally replays fixed backing
        // and its call window, including the selected range operation.
        if shared
            && !fixed_window
            && !backing_type.is_some_and(|reference| {
                matches!(
                    checked.type_reference_table.type_reference(reference),
                    TypeReferenceNode::FixedArray { .. }
                )
            })
        {
            continue;
        }
        let mut source = projected_receivers::store_destination(
            checked,
            machine.symbol,
            state.symbol,
            Some(coordinate.statement_index as usize),
            backing,
        )?;
        if let Some(range) = range {
            // Reconstruct bounds from authored source, not the producer's path.
            // This remains a call loan of the backing rather than a store into
            // a synthetic array whose size happens to match the window.
            if range.end_inclusive
                || !validation::has_builtin_subslice_meaning(
                    &checked.typed,
                    machine,
                    Some(state),
                    target,
                )
            {
                return unsupported("byte window lost builtin exclusive-end range meaning");
            }
            let backing_type = backing_type.ok_or(LoweringError::Unsupported(
                "byte window has no declared backing",
            ))?;
            let TypeReferenceNode::FixedArray {
                element_type,
                length: checked_trees::types::FixedArrayLength::Literal(extent),
            } = checked.type_reference_table.type_reference(backing_type)
            else {
                return unsupported("byte window requires fixed-array backing");
            };
            let extent = u64::try_from(*extent)
                .map_err(|_| LoweringError::Unsupported("byte backing extent exceeds u64"))?;
            let endpoint = |expression, omitted| -> Result<u64, LoweringError> {
                if !checked.expression_table.expression_is_valid(expression) {
                    return Ok(omitted);
                }
                let ExpressionNode::Integer(value) =
                    checked.expression_table.expression(expression)
                else {
                    return unsupported("byte window needs independently retained dynamic bounds");
                };
                value.value_bignum().and_then(|value| value.to_u64()).ok_or(
                    LoweringError::Unsupported("byte window endpoint exceeds u64"),
                )
            };
            let start = endpoint(range.start, 0)?;
            let end = endpoint(range.end, extent)?;
            if extent == 0
                || start > end
                || end > extent
                || checked.primitive_type_reference(*element_type) != Some(PrimitiveType::U8)
            {
                return unsupported("byte window exceeds its exact byte backing");
            }
            source.path.push(
                checked_trees::CheckedUnitStructuralPathSegment::FixedByteRange { start, end },
            );
        }
        match argument.source {
            CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
                let parameter = caller_parameters.get(parameter_index as usize).ok_or(
                    LoweringError::Unsupported(
                        "boundary byte loan lost its retained source parameter",
                    ),
                )?;
                let source_parameter = checked
                    .state_parameters(state)
                    .get(parameter.position as usize)
                    .ok_or(LoweringError::Unsupported(
                        "boundary byte loan lost its authored source parameter",
                    ))?;
                // A qualified bounded owner has a live length, not the fixed
                // extent of its array spelling. A whole shared observation
                // retains that original formal and only attenuates access.
                let whole_bounded_loan = shared
                    && !fixed_window
                    && range.is_none()
                    && source.path.is_empty()
                    && validation::loaned_bounded_byte_buffer_capacity(
                        &checked.typed,
                        source_parameter.type_reference,
                    )
                    .is_some()
                    && {
                        let carrier = crate::unit::attached_unit::structural_carrier_type(
                            checked,
                            source_parameter.type_reference,
                        )?;
                        let declared_access =
                            match checked.type_reference_table.type_reference(carrier) {
                                TypeReferenceNode::Reference {
                                    access: language_core::ReferenceAccess::Shared,
                                    ..
                                } => Some(CheckedStructuralAccess::SharedBorrow),
                                TypeReferenceNode::Reference {
                                    access: language_core::ReferenceAccess::Mutable,
                                    ..
                                } => Some(CheckedStructuralAccess::MutableBorrow),
                                TypeReferenceNode::FixedArray { .. } => {
                                    Some(CheckedStructuralAccess::Owned)
                                }
                                _ => None,
                            };
                        declared_access == Some(parameter.access)
                    };
                if source_parameter.symbol != source.root
                    || (source.path.is_empty()
                        && byte_view_access(source_parameter) != Some(expected_access)
                        && !is_fixed_byte_array(checked, source_parameter, shared)
                        && !whole_bounded_loan)
                {
                    return unsupported("boundary byte loan differs from its source parameter");
                }
            }
            CheckedUnitStructuralArgumentSourcePlan::StructuralResult { .. } => {
                // Result-use custody independently joins the binding ordinal
                // to this authored local. Here replay the local's storage and
                // access, just as the parameter branch does for entry storage.
                let mut locals = checked
                    .statement_table
                    .statements(state.statement_nodes)
                    .iter()
                    .take(coordinate.statement_index as usize)
                    .filter_map(|statement| match statement {
                        checked_trees::statement::StatementNode::LocalData(local)
                            if local.symbol == source.root =>
                        {
                            Some(local)
                        }
                        _ => None,
                    });
                let local = locals.next().ok_or(LoweringError::Unsupported(
                    "boundary byte loan has no earlier local backing",
                ))?;
                if locals.next().is_some()
                    || (!shared && !local.is_mutable)
                    || !matches!(
                        checked.expression_table.expression(expression),
                        ExpressionNode::Borrow(_)
                    )
                    || !backing_type.is_some_and(|reference| {
                        matches!(
                            checked.type_reference_table.type_reference(reference),
                            TypeReferenceNode::FixedArray {
                                element_type,
                                length: checked_trees::types::FixedArrayLength::Literal(_),
                            } if checked.primitive_type_reference(*element_type)
                                == Some(PrimitiveType::U8)
                        )
                    })
                {
                    return unsupported("boundary byte loan changes its local backing or access");
                }
            }
            _ => return unsupported("boundary byte loan has no retained backing storage"),
        }
        if access != expected_access
            || argument.access
                != if shared {
                    CheckedStructuralAccess::SharedBorrow
                } else {
                    CheckedStructuralAccess::MutableBorrow
                }
            || source.path != argument.path
        {
            return unsupported("boundary byte loan differs from its authored backing");
        }
        replayed.push(ordinal);
    }
    Ok(replayed)
}

fn validate_owned_byte_loan(
    checked: &CheckedTrees,
    machine: SymbolHandle,
    state: &checked_trees::state::State,
    coordinate: CheckedUnitCallCoordinate,
    target: SymbolHandle,
    expression: checked_trees::expression::ExpressionHandle,
    access: language_core::ReferenceAccess,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Result<(), LoweringError> {
    let ExpressionNode::Name(name) = checked.expression_table.expression(expression) else {
        return unsupported("owned byte loan lost its whole local source");
    };
    let (ordinal, local) =
        checked
            .state_local_declaration(state, name.symbol)
            .ok_or(LoweringError::Unsupported(
                "owned byte loan lost its local declaration",
            ))?;
    if !name.symbol.is_valid()
        || ordinal >= coordinate.statement_index as usize
        || !local.initial_value.is_valid()
        || validation::bounded_byte_buffer_capacity(&checked.typed, local.type_reference).is_none()
        || access != language_core::ReferenceAccess::Shared
        || argument.access != CheckedStructuralAccess::SharedBorrow
        || !argument.path.is_empty()
    {
        return unsupported("owned byte loan changes its declaration, access, or path");
    }
    let captured = crate::expression_preparation::source_custody::borrow_occurrences::call(
        checked,
        machine,
        state.symbol,
        coordinate,
        target,
    )?;
    let borrow = &checked.facts.borrow;
    let accesses =
        borrow
            .argument_accesses
            .span(captured.accesses)
            .ok_or(LoweringError::Unsupported(
                "owned byte loan has a stale access roster",
            ))?;
    let mut occurrences = accesses
        .iter()
        .filter(|row| row.root_symbol == local.symbol)
        .peekable();
    if occurrences.peek().is_none()
        || occurrences.any(|row| {
            row.kind != checked_trees::BorrowAccessKind::Read
                || borrow
                    .access_segments
                    .span(row.segments)
                    .is_none_or(|path| !path.is_empty())
        })
    {
        return unsupported("owned byte loan differs from its captured shared place");
    }
    Ok(())
}

fn is_fixed_byte_array(
    checked: &CheckedTrees,
    parameter: &checked_trees::signature::StateParameter,
    shared: bool,
) -> bool {
    let TypeReferenceNode::Reference {
        access, referee, ..
    } = checked
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return false;
    };
    if *access != language_core::ReferenceAccess::Mutable
        && !(shared && *access == language_core::ReferenceAccess::Shared)
    {
        return false;
    }
    let TypeReferenceNode::FixedArray {
        element_type,
        length: checked_trees::types::FixedArrayLength::Literal(_),
    } = checked.type_reference_table.type_reference(*referee)
    else {
        return false;
    };
    matches!(
        checked.type_reference_table.type_reference(*element_type),
        TypeReferenceNode::Named { .. }
    ) && checked.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
}
