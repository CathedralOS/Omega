//! Array construction shares the ordinary structural result namespace. Replay
//! rejoins each operand to its authored expression before emitting portable values.
//! Completion uses the sibling structural-completion checker; this module owns
//! array shape, storage and operand correspondence.

use super::*;
use checked_trees::CheckedArrayConstructionSource;
use checked_trees::CheckedUnitCallCoordinate;
use checked_trees::statement::StatementNode;
use checked_trees::{CheckedCallScalarArgument, CheckedUnitStructuralResultBindingPlan};

/// Bind only the local whose initializer owns this exact published result.
/// Nested call/argument arrays in the same statement have different owners and
/// must never replace its payload merely because their types happen to match.
pub(super) fn bind_local(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    operation: &CheckedUnitEffectOperationPlan,
    place: PlaceId,
    locals: &mut Vec<(symbols::SymbolHandle, terminal_psi::StructuralArgument)>,
) -> Result<(), LoweringError> {
    let (result, expression) = match operation {
        CheckedUnitEffectOperationPlan::EstablishScalarArray {
            source: CheckedArrayConstructionSource::Statement,
            result,
            ..
        } => (result, None),
        CheckedUnitEffectOperationPlan::StructuralCall {
            coordinate, result, ..
        } => {
            let authored = crate::call_source_custody::authored::locate_source(
                checked,
                machine.state,
                *coordinate,
            )?;
            let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
                authored.source_site
            else {
                return Ok(());
            };
            (result, Some(expression))
        }
        _ => return Ok(()),
    };
    let (_, state) = crate::scalar_source_custody::authored_state(checked, machine.state)?;
    let Some(StatementNode::LocalData(local)) = checked
        .statement_table
        .statements(state.statement_nodes)
        .get(result.statement_index as usize)
    else {
        return Ok(());
    };
    if expression.is_some_and(|expression| expression != local.initial_value)
        || !validation::is_closed_primitive_array_type(&checked.typed, local.type_reference)
    {
        return Ok(());
    }
    if local.is_mutable
        || !local.symbol.is_valid()
        || locals.iter().any(|(symbol, _)| *symbol == local.symbol)
        || checked
            .normalized_type_identity(local.type_reference)
            .as_str()
            != result.type_identity
    {
        return unsupported("array local differs from its published initializer binding");
    }
    locals.push((
        local.symbol,
        terminal_psi::StructuralArgument {
            place,
            access: terminal_psi::StructuralAccess::Owned,
            path: Vec::new(),
        },
    ));
    Ok(())
}

/// Reconstruct the constructor's expression and declared type from its owner.
/// Formal positions, rather than filtered array positions, distinguish equal
/// typed actuals. This join remains mandatory when an array has no leaves.
pub(crate) fn construction_expression(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement_index: u32,
    source: CheckedArrayConstructionSource,
) -> Option<(
    checked_trees::expression::ExpressionHandle,
    checked_trees::types::TypeReferenceHandle,
)> {
    let (owner, state) = crate::scalar_source_custody::authored_state(checked, state).ok()?;
    if owner.symbol != machine {
        return None;
    }
    let statements = checked
        .typed
        .statement_table
        .statements(state.statement_nodes);
    match source {
        CheckedArrayConstructionSource::Statement => {
            match statements.get(statement_index as usize)? {
                StatementNode::LocalData(local) if !local.is_mutable => {
                    Some((local.initial_value, local.type_reference))
                }
                StatementNode::Expression(expression)
                    if statement_index as usize + 1 == statements.len() =>
                {
                    Some((*expression, state.return_type))
                }
                _ => None,
            }
        }
        CheckedArrayConstructionSource::CallArgument {
            call_ordinal,
            parameter_position,
        } => {
            let call = crate::call_source_custody::authored::locate_source(
                checked,
                state.symbol,
                CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal,
                },
            )
            .ok()?;
            let target = crate::call_source_custody::authored::target_signature(
                checked,
                machine,
                call.source_target,
            )
            .ok()?;
            if target.boundary {
                return None;
            }
            let parameter = target.parameters.get(parameter_position as usize)?;
            if parameter.is_self
                || parameter.is_mutable
                || parameter.is_const
                || !validation::is_closed_primitive_array_type(
                    &checked.typed,
                    parameter.type_reference,
                )
            {
                return None;
            }
            let mut actuals = call
                .structural_arguments
                .iter()
                .filter(|(position, _)| *position == parameter_position);
            let (_, expression) = actuals.next()?;
            if actuals.next().is_some() {
                return None;
            }
            validation::scalar_array_elements(
                &checked.typed,
                machine,
                *expression,
                parameter.type_reference,
            )?;
            Some((*expression, parameter.type_reference))
        }
    }
}

pub(super) fn validate(
    checked: &CheckedTrees,
    machine: &CheckedUnitEffectMachinePlan,
    source: CheckedArrayConstructionSource,
    result: &CheckedUnitStructuralResultBindingPlan,
    elements: &[CheckedCallScalarArgument],
) -> Result<(), LoweringError> {
    let (expression, reference) = construction_expression(
        checked,
        machine.machine,
        machine.state,
        result.statement_index,
        source,
    )
    .ok_or(LoweringError::Unsupported(
        "array constructor does not rejoin its source owner",
    ))?;
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
        let role = CheckedScalarExpressionRole::ArrayElement {
            source,
            element_ordinal,
        };
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
        crate::scalar_source_custody::value_correspondence::validate(
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

pub(super) fn validate_shape(
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

pub(super) fn source_statement(operation: &CheckedUnitEffectOperationPlan) -> Option<u32> {
    match operation {
        CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } => {
            Some(result.statement_index)
        }
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
