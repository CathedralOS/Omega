//! Independent source occurrence custody for ordered projected scalar stores.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    crate::attached_unit::primitive_locals::validate_roster(checked, plan)?;
    let (machine, state) = crate::scalar_source_custody::authored_state(checked, plan.state)?;
    if machine.symbol != plan.machine {
        return unsupported("structural scalar store has a different authored machine");
    }
    let statements = checked.statement_table.statements(state.statement_nodes);
    let stores = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => Some(store),
            _ => None,
        })
        .collect::<Vec<_>>();
    // Select from the authored statement roster too: deleting every store
    // must not disable source coverage validation.
    for (statement_index, statement) in statements.iter().enumerate() {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let statement_index = u32::try_from(statement_index).map_err(|_| {
            LoweringError::Unsupported("structural scalar store statement ordinal exceeds u32")
        })?;
        let writes = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::ByteSequenceWrite(write)
                    if write.statement_index == statement_index =>
                {
                    Some(write)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if let [write] = writes.as_slice() {
            if plan.operations.iter().any(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
                    store.statement_index == statement_index
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
                    store.statement_index == statement_index
                }
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
                    store.statement_index == statement_index
                }
                CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    statement_index: ordinal,
                    ..
                } => *ordinal == statement_index,
                _ => false,
            }) {
                return unsupported("byte-view assignment has conflicting store custody");
            }
            crate::byte_sequence_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                write,
            )?;
            continue;
        } else if !writes.is_empty() {
            return unsupported("byte-view assignment has duplicate store custody");
        }
        let indexed_stores = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store)
                    if store.statement_index == statement_index =>
                {
                    Some(store)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let byte_stores = plan
            .operations
            .iter()
            .filter_map(|operation| match operation {
                CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store)
                    if store.statement_index == statement_index =>
                {
                    Some(store)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        if let [store] = indexed_stores.as_slice() {
            if !byte_stores.is_empty()
                || stores
                    .iter()
                    .any(|scalar| scalar.statement_index == statement_index)
            {
                return unsupported("indexed assignment has conflicting store custody");
            }
            crate::structural_byte_sequence_index_store::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                store,
            )?;
            continue;
        } else if !indexed_stores.is_empty() {
            return unsupported("indexed assignment has duplicate byte-store custody");
        }
        if let [store] = byte_stores.as_slice() {
            if stores
                .iter()
                .any(|scalar| scalar.statement_index == statement_index)
                || plan.operations.iter().any(|operation| matches!(operation,
                    CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { statement_index: ordinal, .. } if *ordinal == statement_index))
            {
                return unsupported("assignment has both byte and scalar store custody");
            }
            crate::structural_byte_sequence_store::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                store,
            )?;
            continue;
        }
        let primitive_stores = plan.operations.iter().filter(|operation| {
            matches!(operation, CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore { statement_index: ordinal, .. } if *ordinal == statement_index)
        }).count();
        if primitive_stores != 0 {
            if primitive_stores != 1
                || !byte_stores.is_empty()
                || stores
                    .iter()
                    .any(|store| store.statement_index == statement_index)
            {
                return unsupported("primitive assignment has duplicate store custody");
            }
            continue;
        }
        if construction_assignment_owner(checked, plan, statements, statement_index, assignment)
            .is_some()
        {
            continue;
        }
        let matching = stores
            .iter()
            .filter(|store| store.statement_index == statement_index)
            .collect::<Vec<_>>();
        let [store] = matching.as_slice() else {
            return unsupported(
                "structural scalar store roster omits or duplicates an authored assignment",
            );
        };
        validate_assignment(
            checked,
            plan.machine,
            plan.state,
            statement_index,
            assignment,
            store,
        )?;
    }
    for operation in &plan.operations {
        if let CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            value,
        } = operation
        {
            if let checked_trees::CheckedPrimitiveStoreDestination::Local { symbol } = destination {
                crate::attached_unit::primitive_locals::source(
                    checked,
                    plan,
                    *symbol,
                    *statement_index,
                )?;
                crate::primitive_store::validate_symbol_assignment(
                    checked,
                    plan.state,
                    *statement_index,
                    *symbol,
                    value,
                )?;
                continue;
            }
            let checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index } =
                destination
            else {
                return unsupported("primitive store has no destination");
            };
            let destination = plan
                .structural_parameters
                .get(*parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "primitive store destination is absent",
                ))?;
            crate::primitive_store::validate_assignment(
                checked,
                plan.state,
                *statement_index,
                destination,
                value,
            )?;
        }
    }
    for store in &stores {
        if !matches!(
            statements.get(store.statement_index as usize),
            Some(StatementNode::Assignment(_))
        ) {
            return unsupported("structural scalar store has no authored assignment");
        }
    }
    let byte_stores = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => Some(store),
            _ => None,
        })
        .collect::<Vec<_>>();
    for store in &byte_stores {
        let Some(StatementNode::Assignment(assignment)) =
            statements.get(store.statement_index as usize)
        else {
            return unsupported("byte-field store has no authored assignment");
        };
        crate::structural_byte_sequence_store::validate_assignment(
            checked,
            plan.machine,
            plan.state,
            assignment,
            store,
        )?;
    }
    let mut has_indexed_stores = false;
    for operation in &plan.operations {
        if let CheckedUnitEffectOperationPlan::ByteSequenceWrite(write) = operation {
            has_indexed_stores = true;
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(write.statement_index as usize)
            else {
                return unsupported("byte-view write has no authored assignment");
            };
            crate::byte_sequence_write::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                write,
            )?;
        }
        if let CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) =
            operation
        {
            has_indexed_stores = true;
            let Some(StatementNode::Assignment(assignment)) =
                statements.get(store.statement_index as usize)
            else {
                return unsupported("byte replacement has no authored assignment");
            };
            crate::structural_byte_sequence_index_store::validate_assignment(
                checked,
                plan.machine,
                plan.state,
                assignment,
                store,
            )?;
        }
    }
    if (!stores.is_empty() || !byte_stores.is_empty() || has_indexed_stores)
        && !matches!(plan.operations.last(),
        Some(CheckedUnitEffectOperationPlan::Complete { statement_index, .. })
            if usize::try_from(*statement_index).ok() == Some(statements.len()))
    {
        return unsupported("structural scalar store return lost its authored statement boundary");
    }
    Ok(())
}

/// Existing affine construction writes target a local, not a receiver field.
/// Delegate only the exact source element occurrence retained by that owner.
fn construction_assignment_owner(
    checked: &CheckedTrees,
    plan: &CheckedUnitEffectMachinePlan,
    statements: &[StatementNode],
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
) -> Option<()> {
    let ExpressionNode::Indexed(target) = checked.expression_table.expression(assignment.target)
    else {
        return None;
    };
    let ExpressionNode::Name(root) = checked.expression_table.expression(target.collection) else {
        return None;
    };
    let ExpressionNode::Integer(index) = checked.expression_table.expression(target.index) else {
        return None;
    };
    let index = index.value_bignum()?.to_u64()?;
    if !root.symbol.is_valid()
        || root.head_symbol != root.symbol
        || checked
            .expression_table
            .name_path_members(root.members)
            .len()
            != 1
    {
        return None;
    }
    let local = statements
        .get(..statement_index as usize)?
        .iter()
        .find_map(|statement| match statement {
            StatementNode::LocalData(local)
                if local.symbol == root.symbol
                    && local.is_mutable
                    && !local.initial_value.is_valid() =>
            {
                Some(local)
            }
            _ => None,
        })?;
    let checked_trees::types::TypeReferenceNode::FixedArray { element_type, .. } = checked
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } =
        checked.type_reference_table.type_reference(*element_type)
    else {
        return None;
    };
    let ExpressionNode::StructLiteral(literal) =
        checked.expression_table.expression(assignment.value)
    else {
        return None;
    };
    if literal.type_symbol != *symbol
        || literal.case_name.is_some()
        || !checked
            .expression_table
            .struct_fields(literal.fields)
            .is_empty()
    {
        return None;
    }
    let mut operations = plan
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
                statement_index: ordinal,
                declaration_ordinal,
                type_identity,
            } if *ordinal == statement_index => Some((*declaration_ordinal, type_identity)),
            _ => None,
        });
    let (ordinal, identity) = operations.next()?;
    if operations.next().is_some() {
        return None;
    }
    let mut declarations = plan
        .trivial_affine_locals
        .iter()
        .filter(|local| local.declaration_ordinal == ordinal);
    let declaration = declarations.next()?;
    if declarations.next().is_some()
        || declaration.type_identity != *identity
        || declaration.construction.as_ref()?.index != index
    {
        return None;
    }
    Some(())
}

pub(crate) fn validate_assignment(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    statement_index: u32,
    assignment: &checked_trees::statement::TableAssignment,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
) -> Result<(), LoweringError> {
    let (owner, state) = crate::scalar_source_custody::authored_state(checked, state_symbol)?;
    if owner.symbol != machine || statement_index != store.statement_index {
        return unsupported("structural scalar store has a different authored machine");
    }
    let source = crate::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement_index as usize),
        assignment.target,
    )?;
    let destination = match store.destination {
        checked_trees::CheckedStructuralScalarFieldStoreDestination::Parameter { position } => {
            checked
                .state_parameters(state)
                .get(position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural scalar store has no authored destination parameter",
                ))?
                .symbol
        }
        checked_trees::CheckedStructuralScalarFieldStoreDestination::Local { symbol } => {
            local_destination(checked, state, statement_index, symbol)?;
            symbol
        }
    };
    let mut path = store.carrier_path.clone();
    path.push(CheckedUnitStructuralPathSegment::Field(
        store.field_identity.clone(),
    ));
    if source.root != destination || source.path != path {
        return unsupported("structural scalar store destination drifted from its authored place");
    }
    if computation_root(checked, machine, state_symbol, store)?.is_some() {
        return Ok(());
    }
    let plans = &checked.facts.values.scalar_expressions;
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let expressions = plans
        .expressions
        .iter()
        .filter(|expression| {
            expression.state == state_symbol
                && expression.statement_ordinal == statement_index
                && expression.role == role
        })
        .collect::<Vec<_>>();
    let [expression] = expressions.as_slice() else {
        return unsupported("structural scalar store has no unique selected RHS");
    };
    if Some(&expression.expression) != store.value.as_pure() {
        return unsupported("structural scalar store RHS drifted from its selected expression");
    }
    let bindings = plans
        .source_bindings
        .iter()
        .filter(|(_, binding)| {
            binding.state == state_symbol
                && binding.statement_ordinal == statement_index
                && binding.role == role
        })
        .map(|(_, binding)| binding)
        .collect::<Vec<_>>();
    let [binding] = bindings.as_slice() else {
        return unsupported("structural scalar store has no unique RHS source binding");
    };
    let destination = match checked.expression_table.expression(assignment.target) {
        ExpressionNode::Name(name) => name.symbol,
        _ => symbols::SymbolHandle::invalid(),
    };
    if binding.expression != assignment.value || binding.destination != destination {
        return unsupported(
            "structural scalar store RHS binding drifted from its authored assignment",
        );
    }
    let primitive_type = store
        .value
        .as_pure()
        .and_then(|value| value.primitive_type())
        .ok_or(LoweringError::Unsupported(
            "structural scalar store RHS has no retained primitive carrier",
        ))?;
    crate::scalar_source_custody::validate_pure(
        checked,
        binding,
        terminal_scalar_type(primitive_type)?,
    )
}

/// A local store names a unique, earlier owned declaration. Its current live
/// home is resolved separately by the ordered lowering namespace, so a move
/// cannot be undone merely by finding the old declaration again.
/// Source mutability gates whole-local rebinding, not field writes.
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
        || !validation::has_plain_owned_contents_with_numeric_constraints(
            &checked.typed,
            local.type_reference,
        )
        || checked
            .primitive_type_reference(local.type_reference)
            .is_some()
    {
        return unsupported("record store lost its owned structural declaration");
    }
    Ok(local)
}

/// Select a computation only through its exact authored assignment coordinate.
pub(crate) fn computation_root(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    store: &checked_trees::CheckedStructuralScalarFieldStorePlan,
) -> Result<Option<checked_trees::CheckedScalarComputationHandle>, LoweringError> {
    let role = CheckedScalarExpressionRole::AssignmentValue;
    let plans = &checked.facts.values.scalar_computations;
    let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
        root.state == state && root.statement_ordinal == store.statement_index && root.role == role
    });
    let root = roots.next();
    if roots.next().is_some() {
        return unsupported("field assignment has duplicate computation roots");
    }
    let checked_trees::CheckedStructuralScalarFieldStoreValue::Computation(handle) = store.value
    else {
        return if root.is_none() {
            Ok(None)
        } else {
            unsupported("field assignment replaced its computation with a pure value")
        };
    };
    let root = root.ok_or(LoweringError::Unsupported(
        "field assignment has no computation root",
    ))?;
    if root.machine != machine || root.root != handle || !plans.nodes.is_valid(handle) {
        return unsupported("field assignment computation root has different custody");
    }
    let source = crate::scalar_source_custody::locate(checked, state, store.statement_index, role)?;
    let node = plans.nodes.get(handle);
    if source.machine != machine
        || source.expression != node.authored_root
        || source.primitive_type != node.primitive_type
        || node.primitive_type != store.primitive_type
        || source.destination.is_valid()
    {
        return unsupported("field assignment computation differs from its authored RHS");
    }
    crate::scalar_source_custody::validate_computation_calls(
        checked,
        machine,
        state,
        store.statement_index,
        handle,
        source.expression,
    )?;
    Ok(Some(handle))
}
