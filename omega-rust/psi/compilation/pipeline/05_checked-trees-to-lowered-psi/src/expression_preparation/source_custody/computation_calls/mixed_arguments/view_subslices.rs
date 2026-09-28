//! Computed view ranges retain a particular established view and borrow row.
//! Range replay validates operator meaning and endpoint custody without running
//! either endpoint. Descriptor production belongs to ordered call evaluation.
use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::expression::{ExpressionHandle, ExpressionNode};
use checked_trees::types::TypeReferenceNode;
use checked_trees::{
    CheckedStorageRoot, CheckedStructuralAccess, CheckedSubsliceSite, CheckedTrees,
    CheckedUnitStructuralArgumentPlan, CheckedUnitStructuralArgumentSourcePlan,
};

pub(super) fn captured_range(
    checked: &CheckedTrees,
    expression: ExpressionHandle,
) -> Result<checked_trees::CapturedPlace, LoweringError> {
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(expression) else {
        return unsupported("computed subslice lost its indexed source");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("computed subslice lost its range");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(indexed.collection) else {
        return unsupported("computed subslice requires an established named view");
    };
    if range.end_inclusive
        || !name.symbol.is_valid()
        || name.head_symbol != name.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
    {
        return unsupported("computed subslice changed its exclusive range or source name");
    }
    // The borrow ledger canonicalizes closed constant bounds as a fixed
    // half-open range. An omitted end still observes the runtime view length,
    // so it retains the authored index identity rather than a guessed extent.
    let fixed_bounds = (|| {
        let start = if range.start.is_valid() {
            usize::try_from(
                checked
                    .expression_table
                    .constant_integer_value(range.start)?,
            )
            .ok()?
        } else {
            0
        };
        if !range.end.is_valid() {
            return None;
        }
        let end =
            usize::try_from(checked.expression_table.constant_integer_value(range.end)?).ok()?;
        Some((start, end))
    })();
    let segment = match fixed_bounds {
        Some((start, end)) => facts::PlaceSegment::FixedRange { start, end },
        None => facts::PlaceSegment::Index {
            expression: indexed.index,
        },
    };
    Ok(checked_trees::CapturedPlace {
        root_symbol: name.symbol,
        segments: vec![segment],
    })
}

pub(super) fn validate(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    statement: u32,
    target: &checked_trees::signature::StateParameter,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
    call: &checked_trees::BorrowCallFact,
    position: usize,
    site: CheckedSubsliceSite,
) -> Result<(), LoweringError> {
    let (root, retained_expression, start, end, bytes) = match &argument.source {
        CheckedUnitStructuralArgumentSourcePlan::ByteSequenceSubslice {
            root,
            expression,
            start,
            end,
        } => (*root, *expression, start, end, true),
        CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
            root,
            expression,
            start,
            end,
        } => (*root, *expression, start, end, false),
        _ => return unsupported("computed subslice has no retained range"),
    };
    if retained_expression != expression
        || !argument.path.is_empty()
        || argument.access != CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("computed subslice changed its expression or view custody");
    }
    let captured = captured_range(checked, expression)?;
    let (symbol, reference) = match root {
        CheckedStorageRoot::Parameter { index } => {
            let parameter = checked
                .state_parameters(state)
                .iter()
                .filter(|parameter| {
                    !parameter.relevance.is_erased()
                        && checked
                            .primitive_type_reference(parameter.type_reference)
                            .is_none()
                })
                .nth(index as usize)
                .ok_or(LoweringError::Unsupported(
                    "computed subslice lost its source parameter",
                ))?;
            if parameter.is_const || parameter.is_mutable {
                return unsupported("computed subslice source parameter changed custody");
            }
            (parameter.symbol, parameter.type_reference)
        }
        CheckedStorageRoot::ViewLocal { symbol } => {
            let mut locals = checked
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
                .filter_map(|(ordinal, statement)| match statement {
                    checked_trees::statement::StatementNode::LocalData(local)
                        if local.symbol == symbol =>
                    {
                        Some((ordinal, local))
                    }
                    _ => None,
                });
            let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(
                "computed subslice lost its view local",
            ))?;
            if locals.next().is_some()
                || ordinal >= statement as usize
                || local.is_mutable
                || !checked
                    .expression_table
                    .expression_is_valid(local.initial_value)
            {
                return unsupported("computed subslice has no exact prior view establishment");
            }
            (local.symbol, local.type_reference)
        }
        CheckedStorageRoot::OwnedLocal { .. } => {
            return unsupported("computed subslice cannot narrow owned storage");
        }
    };
    let view_type = |reference| {
        let TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Shared,
            referee,
            ..
        } = checked.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        let TypeReferenceNode::Slice { element_type } =
            checked.type_reference_table.type_reference(*referee)
        else {
            return None;
        };
        let is_bytes = matches!(
            checked.type_reference_table.type_reference(*element_type),
            TypeReferenceNode::Named { .. }
        ) && checked.primitive_type_reference(*element_type)
            == Some(checked_trees::types::PrimitiveType::U8);
        Some((checked.normalized_type_identity(*referee), is_bytes))
    };
    let (identity, source_bytes) = view_type(reference).ok_or(LoweringError::Unsupported(
        "computed subslice source is not a shared view",
    ))?;
    let (target_identity, target_bytes) = view_type(target.type_reference).ok_or(
        LoweringError::Unsupported("computed subslice formal is not a shared view"),
    )?;
    if symbol != captured.root_symbol
        || identity != target_identity
        || identity.as_str() != argument.type_identity
        || source_bytes != bytes
        || target_bytes != bytes
    {
        return unsupported("computed subslice substituted its root, type, or view family");
    }
    crate::unit::attached_unit::view_ranges::replay(
        checked,
        &crate::unit::attached_unit::view_ranges::ViewRangeSite {
            state: state.symbol,
            statement,
            site,
            expression,
            retained: Some((start, end)),
        },
        symbol,
        &[],
    )?;
    super::borrow_rows::validate_shared_place_argument_at(checked, call, position, &captured)
}
