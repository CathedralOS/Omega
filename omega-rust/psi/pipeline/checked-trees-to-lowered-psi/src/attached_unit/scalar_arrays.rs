//! Array construction shares the ordinary structural result namespace. Replay
//! rejoins each operand to its authored expression before emitting portable values.

use super::*;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{CheckedScalarArrayLiteral, CheckedUnitStructuralResultBindingPlan};

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[CheckedScalarArrayLiteral],
) -> Result<(), LoweringError> {
    let source = checked
        .typed
        .machines()
        .iter()
        .find(|source| source.symbol == machine.machine)
        .ok_or(LoweringError::Unsupported(
            "array constructor machine is absent",
        ))?;
    let state = checked
        .typed
        .machine_states(source)
        .iter()
        .find(|state| state.symbol == machine.state)
        .ok_or(LoweringError::Unsupported(
            "array constructor state is absent",
        ))?;
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    let (expression, reference) = match statements.get(result.statement_index as usize) {
        Some(StatementNode::LocalData(local)) if !local.is_mutable => {
            (local.initial_value, local.type_reference)
        }
        Some(StatementNode::Expression(expression))
            if result.statement_index as usize + 1 == statements.len() =>
        {
            (*expression, state.return_type)
        }
        _ => return unsupported("array constructor does not rejoin its source statement"),
    };
    if result.multiplicity != Multiplicity::Unrestricted
        || result.type_identity != checked.typed.normalized_type_identity(reference).as_str()
    {
        return unsupported("array constructor result differs from its declared type");
    }
    validate_shape(checked, reference)?;
    let leaves = validation::closed_constant_array_elements(
        &checked.typed,
        machine.machine,
        expression,
        reference,
    )
    .ok_or(LoweringError::Unsupported(
        "array constructor source is not an exact closed primitive array",
    ))?;
    let mut projection = expression;
    while let ExpressionNode::Indexed(indexed) =
        checked.typed.expression_table.expression(projection)
    {
        if let Some(selected) = checked.facts.operators.expression_use(projection)
            && (selected.spelling != language_core::OperatorSpelling::Index
                || selected.selected_operator_symbol.is_valid()
                || selected.candidate_count != 0
                || !matches!(
                    selected.status,
                    checked_trees::CheckedOperatorResolutionStatus::Missing
                        | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
                ))
        {
            return unsupported("array constructor indexing selection changed");
        }
        projection = indexed.collection;
    }
    if leaves.len() != elements.len()
        || !leaves
            .iter()
            .zip(elements)
            .all(|((leaf, primitive), element)| {
                match (checked.typed.expression_table.expression(*leaf), element) {
                    (
                        ExpressionNode::Integer(actual),
                        CheckedScalarArrayLiteral::Integer(expected),
                    ) => {
                        if actual.landing().is_some() {
                            actual == expected
                        } else {
                            validation::land_anonymous_integer_expression(
                                &checked.typed,
                                *leaf,
                                *primitive,
                                |_| false,
                            )
                            .as_ref()
                                == Some(expected)
                        }
                    }
                    (
                        ExpressionNode::Boolean(actual),
                        CheckedScalarArrayLiteral::Boolean(expected),
                    ) => actual == expected,
                    _ => false,
                }
            })
    {
        return unsupported("array constructor scalar operands changed");
    }
    Ok(())
}

pub(super) fn validate_result(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
) -> Result<(), LoweringError> {
    let source = checked
        .typed
        .machines()
        .iter()
        .find(|source| source.symbol == machine.machine)
        .ok_or(LoweringError::Unsupported(
            "structural result machine is absent",
        ))?;
    let [state] = checked.typed.machine_states(source) else {
        return unsupported("structural result state roster changed");
    };
    let Some(result) = &machine.structural_result else {
        let mut reference = state.return_type;
        while let checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
            checked.typed.type_reference_table.type_reference(reference)
        {
            reference = *base_type;
        }
        if !matches!(
            checked.typed.type_reference_table.type_reference(reference),
            checked_trees::types::TypeReferenceNode::Unit
        ) {
            return unsupported("Unit completion erases the authored result type");
        }
        return Ok(());
    };
    if state.symbol != machine.state
        || !source.body_is_present
        || result.multiplicity != Multiplicity::Unrestricted
        || !validation::is_closed_primitive_array_type(&checked.typed, state.return_type)
        || checked
            .typed
            .normalized_type_identity(state.return_type)
            .as_str()
            != result.type_identity
    {
        return unsupported("structural result signature changed");
    }
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    if !matches!(machine.operations.last(), Some(CheckedUnitEffectOperationPlan::Complete { statement_index, .. }) if *statement_index as usize == statements.len())
    {
        return unsupported("structural completion coordinate differs from source");
    }
    let Some(StatementNode::Expression(expression)) = statements.last() else {
        return unsupported("structural result source has no completion value");
    };
    if let ExpressionNode::Name(path) = checked.typed.expression_table.expression(*expression) {
        let Some(StatementNode::LocalData(local)) = statements.get(result.statement_index as usize)
        else {
            return unsupported("returned structural binding has no declaration");
        };
        if path.symbol != local.symbol {
            return unsupported("returned structural binding differs from source");
        }
    } else if result.statement_index as usize + 1 != statements.len() {
        return unsupported("returned constructor differs from source");
    }
    let has_exact_producer = machine.operations.iter().any(|operation| {
        matches!(operation,
            CheckedUnitEffectOperationPlan::EstablishScalarArray { result: candidate, .. }
            if candidate == result)
    });
    if !has_exact_producer {
        return unsupported("returned structural value has no exact producer");
    }
    // Every authored statement owes an operation, except a final binding use.
    for index in 0..statements.len() {
        if index + 1 == statements.len()
            && matches!(
                checked.typed.expression_table.expression(*expression),
                ExpressionNode::Name(_)
            )
        {
            continue;
        }
        if !machine
            .operations
            .iter()
            .any(|operation| source_statement(operation) == Some(index as u32))
        {
            return unsupported("structural result body omits an authored statement");
        }
    }
    Ok(())
}

fn validate_shape(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Result<(), LoweringError> {
    for _ in 0..checked.typed.type_reference_table.type_reference_count() {
        let identity = checked.typed.normalized_type_identity(reference);
        let mut matches = checked
            .facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .filter(|plan| plan.identity == identity.as_str());
        let plan = matches
            .next()
            .ok_or(LoweringError::Unsupported("array source shape is absent"))?;
        if matches.next().is_some() {
            return unsupported("array source shape identity is duplicated");
        }
        match (
            checked.typed.type_reference_table.type_reference(reference),
            &plan.shape,
        ) {
            (
                checked_trees::types::TypeReferenceNode::FixedArray {
                    element_type,
                    length: checked_trees::types::FixedArrayLength::Literal(length),
                },
                checked_trees::CheckedUnitStructuralTypeShape::FixedArray {
                    element_type_identity,
                    length: actual_length,
                },
            ) if u64::try_from(*length).ok() == Some(*actual_length)
                && checked
                    .typed
                    .normalized_type_identity(*element_type)
                    .as_str()
                    == element_type_identity =>
            {
                reference = *element_type
            }
            (
                checked_trees::types::TypeReferenceNode::Named { .. },
                checked_trees::CheckedUnitStructuralTypeShape::PrimitiveScalar(primitive),
            ) if checked.typed.primitive_type_reference(reference) == Some(*primitive) => {
                return Ok(());
            }
            _ => return unsupported("array checked shape differs from its source carrier"),
        }
    }
    unsupported("array source carrier is cyclic")
}

fn source_statement(operation: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarArray { result, .. }
        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { result, .. }
        | CheckedUnitEffectOperationPlan::StructuralCall { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. } => {
            Some(result.statement_index)
        }
        CheckedUnitEffectOperationPlan::EstablishPrimitiveLocal {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::EstablishTrivialAffineLocal {
            statement_index, ..
        }
        | CheckedUnitEffectOperationPlan::EstablishAffineScalarRecordLocal {
            statement_index,
            ..
        } => Some(*statement_index),
        CheckedUnitEffectOperationPlan::CallUnit { coordinate, .. }
        | CheckedUnitEffectOperationPlan::ScalarCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::BoundaryScalarCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralScalarCall {
            coordinate, ..
        }
        | CheckedUnitEffectOperationPlan::SelectedOperatorStructuralCall { coordinate, .. }
        | CheckedUnitEffectOperationPlan::SelectedIeeeFloatFusedMultiplyAdd {
            coordinate, ..
        }
        | CheckedUnitEffectOperationPlan::PortWrite { coordinate, .. } => {
            Some(coordinate.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralScalarFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(store) => {
            Some(store.statement_index)
        }
        CheckedUnitEffectOperationPlan::ByteSequenceWrite(store) => Some(store.statement_index),
        CheckedUnitEffectOperationPlan::CallContinuationCleanup { .. }
        | CheckedUnitEffectOperationPlan::Complete { .. } => None,
    }
}

pub(super) fn emit(
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[CheckedScalarArrayLiteral],
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let mut values = Vec::with_capacity(elements.len());
    for element in elements {
        let (scalar_type, kind) = match element {
            CheckedScalarArrayLiteral::Integer(literal) => {
                let scalar_type = integer_landing_scalar_type(literal)?;
                (
                    scalar_type,
                    OperationKind::IntegerConstant {
                        value: integer_value(literal, scalar_type)?,
                    },
                )
            }
            CheckedScalarArrayLiteral::Boolean(value) => (
                ScalarType::Boolean,
                OperationKind::BooleanConstant { value: *value },
            ),
        };
        let value = ValueDeclaration {
            id: value_id(allocate_dense(next_value)?),
            scalar_type,
        };
        let id = operations.allocate();
        operations.push(Operation {
            id,
            result: OperationResult::Scalar(value),
            kind,
        });
        values.push(value.id);
    }
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    operations.push(Operation {
        id,
        result: OperationResult::Structural(StructuralOperationResult {
            place,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        }),
        kind: OperationKind::EstablishScalarArray { elements: values },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
