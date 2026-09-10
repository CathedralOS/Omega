//! Array construction shares the ordinary structural result namespace. Replay
//! rejoins each operand to its authored expression before emitting portable values.
//! Returning a call result instead uses the shared call-source and callee-body
//! checks. A return names either its exact operation producer or an incoming
//! parameter slot; equal array types cannot substitute for source correspondence.

use super::*;
use checked_trees::CheckedStructuralAccess;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{CheckedCallScalarArgument, CheckedUnitStructuralResultBindingPlan};

mod operands;

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[CheckedCallScalarArgument],
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
    let array =
        validation::scalar_array_elements(&checked.typed, machine.machine, expression, reference)
            .ok_or(LoweringError::Unsupported(
            "array constructor source is not an exact primitive array",
        ))?;
    for projection in array.projections {
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
    }
    let leaves = array.elements;
    if leaves.len() != elements.len() {
        return unsupported("array constructor scalar operand count changed");
    }
    for (ordinal, ((leaf, primitive), element)) in leaves.iter().zip(elements).enumerate() {
        let element_ordinal = u32::try_from(ordinal)
            .map_err(|_| LoweringError::Unsupported("array element ordinal exceeds u32"))?;
        let role = CheckedScalarExpressionRole::ArrayElement { element_ordinal };
        match element {
            CheckedCallScalarArgument::Pure(value) => {
                let (binding, retained) = checked
                    .facts
                    .values
                    .scalar_expressions
                    .bound_expression_at(machine.state, result.statement_index, role)
                    .ok_or(LoweringError::Unsupported(
                        "array element has no exact pure source binding",
                    ))?;
                if binding.expression != *leaf || retained != value {
                    return unsupported("array element differs from its pure source binding");
                }
                crate::scalar_source_custody::validate_pure(
                    checked,
                    binding,
                    terminal_scalar_type(*primitive)?,
                )?;
                if checked
                    .facts
                    .values
                    .scalar_computations
                    .roots
                    .iter()
                    .any(|(_, root)| {
                        root.state == machine.state
                            && root.statement_ordinal == result.statement_index
                            && root.role == role
                    })
                {
                    return unsupported("array element has conflicting pure and computed owners");
                }
            }
            CheckedCallScalarArgument::Computation(handle) => {
                let plans = &checked.facts.values.scalar_computations;
                let mut roots = plans.roots.iter().map(|(_, root)| root).filter(|root| {
                    root.state == machine.state
                        && root.statement_ordinal == result.statement_index
                        && root.role == role
                });
                let root = roots.next().ok_or(LoweringError::Unsupported(
                    "array element has no exact computation root",
                ))?;
                if roots.next().is_some()
                    || root.machine != machine.machine
                    || root.root != *handle
                    || !plans.nodes.is_valid(*handle)
                    || plans.nodes.get(*handle).authored_root != *leaf
                    || plans.nodes.get(*handle).primitive_type != *primitive
                    || checked
                        .facts
                        .values
                        .scalar_expressions
                        .expressions
                        .iter()
                        .any(|value| {
                            value.state == machine.state
                                && value.statement_ordinal == result.statement_index
                                && value.role == role
                        })
                {
                    return unsupported("array computation differs from its source element");
                }
                crate::scalar_source_custody::validate_computation_calls(
                    checked,
                    machine.machine,
                    machine.state,
                    result.statement_index,
                    *handle,
                    *leaf,
                )?;
            }
        }
        operands::validate(
            checked,
            machine.state,
            result.statement_index,
            *leaf,
            *primitive,
            element,
        )?;
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
    match result.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            let mut producers = machine
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        result: candidate,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::StructuralCall {
                        result: candidate, ..
                    } if candidate.binding_ordinal == binding_ordinal => Some(candidate),
                    _ => None,
                });
            let binding = producers.next().ok_or(LoweringError::Unsupported(
                "returned structural value has no exact producer",
            ))?;
            if producers.next().is_some()
                || binding.type_identity != result.type_identity
                || binding.multiplicity != result.multiplicity
            {
                return unsupported("returned structural producer is ambiguous or changed");
            }
            if let ExpressionNode::Name(path) =
                checked.typed.expression_table.expression(*expression)
            {
                let Some(StatementNode::LocalData(local)) =
                    statements.get(binding.statement_index as usize)
                else {
                    return unsupported("returned structural binding has no declaration");
                };
                if path.symbol != local.symbol
                    || path.head_symbol != local.symbol
                    || checked
                        .typed
                        .expression_table
                        .name_path_members(path.members)
                        .len()
                        != 1
                {
                    return unsupported("returned structural binding differs from source");
                }
            } else if binding.statement_index as usize + 1 != statements.len() {
                return unsupported("returned constructor differs from source");
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = machine
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "returned structural parameter is absent",
                ))?;
            let source_parameter = checked
                .typed
                .state_parameters(state)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "returned structural source parameter is absent",
                ))?;
            if parameter.access != CheckedStructuralAccess::Owned
                || source_parameter.is_const
                || source_parameter.is_mutable
                || source_parameter.is_self
                || parameter.multiplicity != Multiplicity::Unrestricted
                || !parameter.qualifications.is_empty()
                || parameter.is_self
                || parameter.fused_service_erasure.is_some()
                || parameter.type_identity != result.type_identity
                || checked
                    .typed
                    .normalized_type_identity(source_parameter.type_reference)
                    .as_str()
                    != result.type_identity
                || !matches!(checked.typed.expression_table.expression(*expression),
                    ExpressionNode::Name(path) if path.symbol == source_parameter.symbol
                        && path.head_symbol == source_parameter.symbol
                        && checked.typed.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("returned structural parameter differs from its owned source");
            }
            validate_shape(checked, source_parameter.type_reference)?;
        }
        _ => return unsupported("structural return source is not an owned whole value"),
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
    elements: &[ValueDeclaration],
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
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
        kind: OperationKind::EstablishScalarArray {
            elements: elements.iter().map(|element| element.id).collect(),
        },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}
