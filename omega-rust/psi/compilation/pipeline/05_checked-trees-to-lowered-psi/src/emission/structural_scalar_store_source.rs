//! Independent source occurrence custody for ordered projected scalar stores.
use super::{
    CheckedScalarExpressionRole, CheckedTrees, CheckedUnitStructuralPathSegment, LoweringError,
    unsupported,
};
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

mod case_fields;

/// Rejoin a structural local to its unique earlier declaration. This is source
/// identity, not a transfer or mutation permission: a carrier may hold reference
/// fields without owning their backing. Consumers retain the exact declared
/// type and resolve its current live storage in the ordered namespace, then
/// check the selected operation's access, qualifications and field geometry.
/// Finding this declaration cannot restore a moved home or strengthen a loan.
pub(crate) fn local_destination<'a>(
    checked: &'a CheckedTrees,
    state: &checked_trees::state::State,
    statement: u32,
    symbol: symbols::SymbolHandle,
) -> Result<&'a checked_trees::statement::TableLocalData, LoweringError> {
    let statements = checked.statement_table.statements(state.statement_nodes);
    let mut locals =
        statements
            .iter()
            .enumerate()
            .filter_map(|(ordinal, statement)| match statement {
                StatementNode::LocalData(local) if local.symbol == symbol => Some((ordinal, local)),
                _ => None,
            });
    let (ordinal, local) = locals.next().ok_or(LoweringError::Unsupported(
        "record store has no source local",
    ))?;
    if !symbol.is_valid()
        || locals.next().is_some()
        || ordinal >= statement as usize
        || !local.type_reference.is_valid()
        || checked
            .primitive_type_reference(local.type_reference)
            .is_some()
    {
        return unsupported("record store lost its earlier structural declaration");
    }
    Ok(local)
}

/// A `Local` store destination roots two ways: an owned structural local
/// (`let mut`) is its own root with an empty prefix, while a borrowed-view
/// local writes through its loan — the authored target resolves through
/// that same loan, so the destination's root and path must continue the
/// place the view's own initializer resolves to. Returns the root and the
/// path prefix the store's carrier path completes.
pub(crate) fn local_store_destination(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: &checked_trees::state::State,
    state_symbol: symbols::SymbolHandle,
    statement: u32,
    symbol: symbols::SymbolHandle,
) -> Result<(symbols::SymbolHandle, Vec<CheckedUnitStructuralPathSegment>), LoweringError> {
    let local = local_destination(checked, state, statement, symbol)?;
    if !matches!(
        checked.expression_table.expression(local.initial_value),
        ExpressionNode::Borrow(_) | ExpressionNode::Cast(_)
    ) {
        return Ok((symbol, Vec::new()));
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    let declaration = statements
        .iter()
        .position(|statement| {
            matches!(statement, StatementNode::LocalData(local) if local.symbol == symbol)
        })
        .ok_or(LoweringError::Unsupported(
            "record store has no source local",
        ))?;
    let source = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(declaration),
        local.initial_value,
    )?;
    Ok((source.root, source.path))
}

/// Select a computation only through its exact authored assignment coordinate.
/// A whole-record replacement selects each field through the literal's own
/// `RecordField` root rather than the assignment's role, so the returned role
/// is the one the resolved root actually carries.
pub(crate) fn computation_root(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    value: &checked_trees::CheckedStoreValue,
    primitive_type: typed_trees::types::PrimitiveType,
    field_identity: Option<&str>,
) -> Result<
    Option<(
        checked_trees::CheckedScalarComputationHandle,
        CheckedScalarExpressionRole,
    )>,
    LoweringError,
> {
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == statement_index && root.role == role
    });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("field assignment has duplicate computation roots");
    }
    let checked_trees::CheckedStoreValue::Computation(handle) = value else {
        return if root.is_none() {
            Ok(None)
        } else {
            unsupported("field assignment replaced its computation with a pure value")
        };
    };
    let handle = *handle;
    let root = match root {
        Some(root) => root,
        None => {
            let mut record_roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                root.state == state
                    && root.statement_ordinal == statement_index
                    && root.root == handle
                    && matches!(root.role, CheckedScalarExpressionRole::RecordField { .. })
            });
            let root = record_roots.next().ok_or(LoweringError::Unsupported(
                "field assignment has no computation root",
            ))?;
            if record_roots.next().is_some() {
                return unsupported("field assignment has duplicate record computation roots");
            }
            root
        }
    };
    if root.machine != machine || root.root != handle || !plans.nodes.is_valid(handle) {
        return unsupported("field assignment computation root has different custody");
    }
    if let CheckedScalarExpressionRole::RecordField {
        expression,
        field_ordinal,
    } = root.role
    {
        // The store's declared field must be exactly the literal position the
        // computation root names; an ordinal pointing at a sibling field would
        // otherwise write that field's value into this store's member.
        let ExpressionNode::StructLiteral(literal) =
            checked.expression_table.expression(expression)
        else {
            return unsupported("record store field has no authored literal owner");
        };
        let field = checked
            .expression_table
            .struct_fields(literal.fields)
            .get(field_ordinal as usize)
            .ok_or(LoweringError::Unsupported(
                "record store field ordinal escaped its literal",
            ))?;
        let identity = checked
            .data_definitions()
            .iter()
            .find_map(|data| {
                checked
                    .data_members(data)
                    .iter()
                    .find_map(|member| match member {
                        checked_trees::data::DataMember::Field(declaration)
                            if declaration.symbol == field.field_symbol =>
                        {
                            Some(declaration.path_identity())
                        }
                        _ => None,
                    })
            })
            .ok_or(LoweringError::Unsupported(
                "record store field declaration is missing",
            ))?;
        let field_identity = field_identity.ok_or(LoweringError::Unsupported(
            "record store field has no field identity",
        ))?;
        if identity != field_identity {
            return unsupported("record store field drifted from its literal ordinal");
        }
    }
    let source = crate::expression_preparation::source_custody::locate(
        checked,
        state,
        statement_index,
        root.role,
    )?;
    let node = plans.nodes.get(handle);
    if source.machine != machine
        || source.expression != node.authored_root
        || source.primitive_type != node.primitive_type
        || node.primitive_type != primitive_type
        || source.destination.is_valid()
    {
        return unsupported("field assignment computation differs from its authored RHS");
    }
    Ok(Some((handle, root.role)))
}
