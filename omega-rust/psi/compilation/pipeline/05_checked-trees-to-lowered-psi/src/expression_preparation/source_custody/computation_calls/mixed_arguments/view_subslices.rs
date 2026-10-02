//! Computed view ranges retain exact array backing or a view and its borrow row.
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
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: ExpressionHandle,
) -> Result<checked_trees::CapturedPlace, LoweringError> {
    let operation = validation::shared_subslice_expression(&checked.typed, expression).ok_or(
        LoweringError::Unsupported("computed subslice lost its shared range expression"),
    )?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("computed subslice lost its indexed source");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("computed subslice lost its range");
    };
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        machine,
        state,
        statement as usize,
        indexed.collection,
    )?;
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
    let mut captured = source.captured_place().clone();
    captured.segments.push(segment);
    Ok(captured)
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
    if retained_expression != expression || argument.access != CheckedStructuralAccess::SharedBorrow
    {
        return unsupported("computed subslice changed its expression or view custody");
    }
    let (caller, _) = super::super::authored_state(checked, state.symbol)?;
    let operation = validation::shared_subslice_expression(&checked.typed, expression).ok_or(
        LoweringError::Unsupported("computed subslice lost its shared range expression"),
    )?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("computed subslice lost its indexed source");
    };
    let source = crate::emission::call_source_custody::projected_receivers::source(
        checked,
        caller.symbol,
        state.symbol,
        statement as usize,
        indexed.collection,
    )?;
    if source.path != argument.path {
        return unsupported("computed subslice substituted its projected path");
    }
    let captured = captured_range(checked, caller.symbol, state.symbol, statement, expression)?;
    let parameters = checked.state_parameters(state);
    let (symbol, reference, mutable_binding) = match root {
        CheckedStorageRoot::Parameter { index } => {
            let parameter = super::super::structural_parameter_at(
                checked,
                parameters,
                index,
                "computed subslice lost its source parameter",
            )?;
            if parameter.is_const
                || !super::super::parameter_binding_is_unique(checked, state, parameter.symbol)
            {
                return unsupported("computed subslice source parameter changed custody");
            }
            (
                parameter.symbol,
                parameter.type_reference,
                parameter.is_mutable,
            )
        }
        CheckedStorageRoot::ViewLocal { symbol } | CheckedStorageRoot::OwnedLocal { symbol } => {
            if parameters
                .iter()
                .any(|parameter| parameter.symbol == symbol)
            {
                return unsupported("computed subslice substituted its local owner");
            }
            let local = super::super::preceding_local(
                checked,
                state,
                symbol,
                statement as usize,
                "computed subslice lost its source local",
                "computed subslice has no exact prior storage establishment",
            )?;
            (local.symbol, local.type_reference, local.is_mutable)
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
        Some((*referee, *element_type, is_bytes))
    };
    let (target_slice, target_element, target_bytes) = view_type(target.type_reference).ok_or(
        LoweringError::Unsupported("computed subslice formal is not a shared view"),
    )?;
    let source_element = if let Some((_, element, _)) = view_type(reference) {
        if mutable_binding
            || !source.path.is_empty()
            || matches!(root, CheckedStorageRoot::OwnedLocal { .. })
        {
            return unsupported("computed subslice has no immutable view descriptor");
        }
        element
    } else {
        if matches!(root, CheckedStorageRoot::ViewLocal { .. }) {
            return unsupported("computed subslice view local lost its descriptor");
        }
        match validation::reference_shell(&checked.typed, reference) {
            Some(shell)
                if matches!(root, CheckedStorageRoot::Parameter { .. })
                    && matches!(
                        shell.access,
                        language_core::ReferenceAccess::Shared
                            | language_core::ReferenceAccess::Mutable
                    ) => {}
            Some(_) => return unsupported("computed subslice backing is not readable storage"),
            None => {}
        };
        let mut projected = validation::declared_place_type_raw(
            &checked.typed,
            caller,
            Some(state),
            indexed.collection,
        )
        .ok_or(LoweringError::Unsupported(
            "computed subslice backing lost its declared type",
        ))?;
        // Parameter custody already lowered one root reference. A reference
        // below a field needs a Referent crossing; it is not an array tip.
        if source.path.is_empty()
            && matches!(root, CheckedStorageRoot::Parameter { .. })
            && let Some(shell) = validation::reference_shell(&checked.typed, projected)
        {
            projected = shell.referee;
        }
        let mut element = None;
        for _ in 0..=checked.type_reference_table.type_reference_count() {
            match checked.type_reference_table.type_reference(projected) {
                TypeReferenceNode::Constrained { base_type, .. } => projected = *base_type,
                TypeReferenceNode::FixedArray { element_type, .. } => {
                    element = Some(*element_type);
                    break;
                }
                _ => break,
            }
        }
        element.ok_or(LoweringError::Unsupported(
            "computed subslice backing is not a fixed array",
        ))?
    };
    if symbol != source.root
        || checked.normalized_type_identity(source_element)
            != checked.normalized_type_identity(target_element)
        || checked.normalized_type_identity(target_slice).as_str() != argument.type_identity
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
        &source.path,
    )?;
    super::borrow_rows::validate_shared_place_argument_at(checked, call, position, &captured)
}
