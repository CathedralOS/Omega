//! Initialized primitive referents retain authored identity apart from SSA locals.

use super::*;
use checked_trees::statement::{StatementNode, TableLocalData};

mod borrows;

pub(crate) struct PrimitiveLocal {
    pub symbol: symbols::SymbolHandle,
    pub declaration: StructuralPlaceDeclaration,
    pub scalar_type: ScalarType,
}

pub(crate) fn source<'checked>(
    checked: &'checked CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    symbol: symbols::SymbolHandle,
    before: u32,
) -> Result<&'checked TableLocalData, LoweringError> {
    let (_, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let mut matches = checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(ordinal, statement)| match statement {
            StatementNode::LocalData(local)
                if local.symbol == symbol && ordinal < before as usize =>
            {
                Some(local)
            }
            _ => None,
        });
    let local = matches.next().ok_or(LoweringError::Unsupported(
        "primitive local has no preceding authored declaration",
    ))?;
    if matches.next().is_some()
        || !symbol.is_valid()
        || !local.is_mutable
        || !matches!(
            checked
                .type_reference_table
                .type_reference(local.type_reference),
            checked_trees::types::TypeReferenceNode::Named { .. }
        )
        || checked
            .primitive_type_reference(local.type_reference)
            .is_none()
    {
        return unsupported("primitive local does not have exact mutable primitive custody");
    }
    Ok(local)
}

pub(crate) fn validate_roster(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let (_, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    for (ordinal, statement) in checked
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let StatementNode::LocalData(local) = statement else {
            continue;
        };
        if !local.is_mutable
            || checked
                .primitive_type_reference(local.type_reference)
                .is_none()
        {
            continue;
        }
        if plan
            .operations
            .iter()
            .filter(|operation| {
                matches!(operation,
            CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal { statement_index, .. }
                if *statement_index as usize == ordinal)
            })
            .count()
            != 1
        {
            return unsupported(
                "primitive local roster omits or duplicates an authored declaration",
            );
        }
    }
    for operation in &plan.operations {
        let CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            statement_index,
            symbol,
            type_identity,
            primitive_type,
            value,
        } = operation
        else {
            continue;
        };
        let next = statement_index
            .checked_add(1)
            .ok_or(LoweringError::Unsupported(
                "primitive local statement ordinal exhausted",
            ))?;
        let local = source(checked, plan, *symbol, next)?;
        if !matches!(checked.statement_table.statements(state.statement_nodes).get(*statement_index as usize),
            Some(StatementNode::LocalData(actual)) if actual.symbol == *symbol)
            || checked.primitive_type_reference(local.type_reference) != Some(*primitive_type)
            || checked
                .typed
                .normalized_type_identity(local.type_reference)
                .into_string()
                != *type_identity
        {
            return unsupported("primitive local substituted its authored type or declaration");
        }
        let (binding, expression) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                plan.state,
                *statement_index,
                CheckedScalarExpressionRole::StorageInitializer,
            )
            .ok_or(LoweringError::Unsupported(
                "primitive local lost its exact initializer",
            ))?;
        if binding.expression != local.initial_value
            || binding.destination != *symbol
            || expression != value
        {
            return unsupported("primitive local substituted its initializer");
        }
        crate::scalar_source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(*primitive_type)?,
        )?;
    }
    Ok(())
}

pub(super) fn validate_argument_source(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    source_target: symbols::SymbolHandle,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
    expression: checked_trees::expression::ExpressionHandle,
) -> Result<bool, LoweringError> {
    let checked_trees::CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } =
        argument.source
    else {
        return Ok(false);
    };
    let local = source(checked, plan, symbol, coordinate.statement_index)?;
    let (machine, _) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    let (root, path, access) =
        parameters::source_path(checked, machine, local.type_reference, expression)?;
    if root != symbol
        || !path.is_empty()
        || !argument.path.is_empty()
        || access != Some(argument.access)
        || argument.access == checked_trees::CheckedStructuralAccess::Owned
        || argument.type_identity
            != checked
                .typed
                .normalized_type_identity(local.type_reference)
                .into_string()
    {
        return unsupported("primitive local argument substituted its authored borrow");
    }
    borrows::validate(
        checked,
        plan,
        coordinate,
        source_target,
        symbol,
        argument.access,
    )?;
    Ok(true)
}

pub(super) fn find(
    locals: &[PrimitiveLocal],
    symbol: symbols::SymbolHandle,
) -> Result<&PrimitiveLocal, LoweringError> {
    let mut matching = locals.iter().filter(|local| local.symbol == symbol);
    let local = matching.next().ok_or(LoweringError::Unsupported(
        "primitive local has not been established",
    ))?;
    if matching.next().is_some() {
        return unsupported("primitive local establishment is duplicated");
    }
    Ok(local)
}

pub(super) fn emit(
    symbol: symbols::SymbolHandle,
    structural_type: StructuralTypeId,
    value: &LoweredDirectExpression,
    values: &[ValueDeclaration],
    next_place: &mut u64,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<PrimitiveLocal, LoweringError> {
    if direct_expression_contains_short_circuit(value) {
        return unsupported("primitive initializer requires expanded Boolean control");
    }
    validate_direct_parameter_types(
        value,
        &values
            .iter()
            .map(|value| value.scalar_type)
            .collect::<Vec<_>>(),
    )?;
    let scalar_type = value.scalar_type();
    let value = emit_direct_expression(value, values, next_value, operations);
    let producer = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    operations.push(Operation {
        id: producer,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishPrimitiveLocal { value },
    });
    Ok(PrimitiveLocal {
        symbol,
        scalar_type,
        declaration: StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            },
        },
    })
}
