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
    let operation = validation::subslice_expression(&checked.typed, expression).ok_or(
        LoweringError::Unsupported("computed subslice lost its readable range expression"),
    )?;
    let ExpressionNode::Indexed(indexed) = checked.expression_table.expression(operation) else {
        return unsupported("computed subslice lost its indexed source");
    };
    let ExpressionNode::Range(range) = checked.expression_table.expression(indexed.index) else {
        return unsupported("computed subslice lost its range");
    };
    let mut captured =
        crate::emission::call_source_custody::projected_receivers::observation_place(
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
    validate_operand(
        checked,
        state,
        statement,
        target.type_reference,
        expression,
        argument,
        site,
    )?;
    let (caller, _) = super::super::authored_state(checked, state.symbol)?;
    let captured = captured_range(checked, caller.symbol, state.symbol, statement, expression)?;
    super::borrow_rows::validate_place_argument_at(
        checked,
        call,
        position,
        &captured,
        match argument.access {
            CheckedStructuralAccess::SharedBorrow => checked_trees::BorrowAccessKind::Read,
            CheckedStructuralAccess::MutableBorrow => checked_trees::BorrowAccessKind::Mutable,
            _ => return unsupported("subslice call has unreadable access"),
        },
    )
}

/// Replay the range operation independently of its consumer. A call additionally
/// rejoins its own captured argument loan; an immediate observation introduces
/// no call or destination-local authority.
pub(crate) fn validate_operand(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    statement: u32,
    target_type: checked_trees::types::TypeReferenceHandle,
    expression: ExpressionHandle,
    argument: &CheckedUnitStructuralArgumentPlan,
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
        || !matches!(
            argument.access,
            CheckedStructuralAccess::SharedBorrow | CheckedStructuralAccess::MutableBorrow
        )
    {
        return unsupported("computed subslice changed its expression or view custody");
    }
    let (caller, _) = super::super::authored_state(checked, state.symbol)?;
    let operation = validation::subslice_expression(&checked.typed, expression).ok_or(
        LoweringError::Unsupported("computed subslice lost its readable range expression"),
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
    let parameters = checked.state_parameters(state);
    let (symbol, reference, mutable_local_binding) = match root {
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
            // `is_mutable` also describes a parameter's mutable referent;
            // its current descriptor is selected by the activation bindings.
            // It is not evidence that the parameter binding was reassigned.
            (parameter.symbol, parameter.type_reference, false)
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
            access, referee, ..
        } = checked.type_reference_table.type_reference(reference)
        else {
            return None;
        };
        if !matches!(
            access,
            language_core::ReferenceAccess::Shared | language_core::ReferenceAccess::Mutable
        ) {
            return None;
        }
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
        Some((*referee, *element_type, is_bytes, *access))
    };
    let (target_slice, target_element, target_bytes, target_access) = view_type(target_type)
        .ok_or(LoweringError::Unsupported(
            "computed subslice formal is not a readable view",
        ))?;
    let expected_access = match target_access {
        language_core::ReferenceAccess::Shared => CheckedStructuralAccess::SharedBorrow,
        language_core::ReferenceAccess::Mutable => CheckedStructuralAccess::MutableBorrow,
        _ => return unsupported("range target access is unreadable"),
    };
    if argument.access != expected_access {
        return unsupported("range changed its formal access");
    }
    let source_element = if let Some((_, element, _, source_access)) = view_type(reference) {
        if target_access == language_core::ReferenceAccess::Mutable
            && source_access != target_access
        {
            return unsupported("range escalated shared descriptor authority");
        }
        if mutable_local_binding
            || !source.path.is_empty()
            || matches!(root, CheckedStorageRoot::OwnedLocal { .. })
        {
            return unsupported("computed subslice has no current view descriptor");
        }
        element
    } else {
        if matches!(root, CheckedStorageRoot::ViewLocal { .. }) {
            return unsupported("computed subslice view local lost its descriptor");
        }
        match validation::reference_shell(&checked.typed, reference) {
            Some(shell)
                if (target_access != language_core::ReferenceAccess::Mutable
                    || shell.access == target_access)
                    && matches!(root, CheckedStorageRoot::Parameter { .. })
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
    Ok(())
}
