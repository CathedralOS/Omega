//! Ordinary structural graph results, with exact authored field custody, and
//! primitive scalar results. A scalar-result state completes with the
//! ordinary single-state completion: the binding its final expression's
//! operation produced, or its value-only exit roster (`validate_scalar`).
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralFieldType, StructuralResultDeclaration,
};
use super::super::super::{
    CheckedScalarExpressionRole, CheckedUnitEffectOperationPlan, Multiplicity, Operation,
    OperationKind, PlaceId, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeShape, TerminalMachineResult, ValueDeclaration,
    allocate_dense, direct_expression_contains_short_circuit, emit_direct_expression,
    lookup_domain_id, lookup_type_id, obligation_id, place_id, terminal_scalar_type, unsupported,
    validate_direct_parameter_types, value_id,
};
use super::super::{CheckedTrees, LoweringError, catalogs};
use super::{CheckedComposedUnitControlMachinePlan, CheckedComposedUnitControlStatePlan};
use crate::emission::operation_emission::buffer::OperationBuffer;
use checked_trees::{CheckedControlResultPlan, data::DataMember, expression::ExpressionNode};

pub(super) fn signature_matches(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    result: &CheckedControlResultPlan,
) -> bool {
    match result {
        CheckedControlResultPlan::Unit => matches!(
            checked
                .type_reference_table
                .type_reference(source.return_type),
            checked_trees::types::TypeReferenceNode::Unit
        ),
        // A primitive result carries no refinement beyond an arithmetic
        // policy or one closed integer range, which the contract publishes
        // as a normal-return guarantee (`callable::scalar_guarantees`).
        CheckedControlResultPlan::Scalar { primitive_type } => {
            checked.primitive_type_reference(source.return_type) == Some(*primitive_type)
                && (matches!(
                    checked
                        .type_reference_table
                        .type_reference(source.return_type),
                    checked_trees::types::TypeReferenceNode::Named { .. }
                ) || validation::is_arithmetic_policy_only_integer(
                    &checked.typed,
                    source.return_type,
                ) || validation::closed_scalar_result_range(&checked.typed, source.return_type)
                    .is_some())
        }
        CheckedControlResultPlan::Structural(result) => {
            // A borrowed `&[T]` view result names the peeled slice carrier in
            // its identity — the contents live in the caller's frame, so the
            // owned-contents custody clause does not apply.
            if let Some(slice) = borrowed_view_slice(checked, source.return_type) {
                return checked.normalized_type_identity(slice).as_str() == result.type_identity
                    && checked.type_multiplicity(source.return_type) == result.multiplicity
                    && view_result_qualifications(checked, source.return_type).as_ref()
                        == Some(&result.qualifications)
                    && validation::structural_result_projected_qualifications(
                        &checked.typed,
                        source.return_type,
                    )
                    .ok()
                    .as_ref()
                        == Some(&result.projected_qualifications);
            }
            let Ok(carrier) = crate::unit::attached_unit::parameters::structural_carrier_type(
                checked,
                source.return_type,
            ) else {
                return false;
            };
            checked
                .normalized_type_identity(carrier)
                .as_str()
                == result.type_identity
                && checked.type_multiplicity(source.return_type) == result.multiplicity
                && validation::structural_result_qualifications(&checked.typed, source.return_type).ok().as_ref() == Some(&result.qualifications)
                && validation::structural_result_projected_qualifications(
                    &checked.typed,
                    source.return_type,
                )
                .ok()
                .as_ref()
                    == Some(&result.projected_qualifications)
                // Plain storage classification excludes linear roots by design.
                // Their admission instead requires exact entry/call/return claims.
                && (result.multiplicity == Multiplicity::Linear || validation::has_plain_owned_contents_with_numeric_constraints(
                    &checked.typed,
                    carrier,
                ))
        }
    }
}

/// The slice a shared-borrow `&[T]` result refers to, peeling constraint and
/// reference shells. Mutable and write-only borrows stay with their own
/// custody families.
fn borrowed_view_slice(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Option<checked_trees::types::TypeReferenceHandle> {
    let referee = loop {
        match checked.type_reference_table.type_reference(reference) {
            checked_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type
            }
            checked_trees::types::TypeReferenceNode::Reference {
                referee,
                access: language_core::ReferenceAccess::Shared,
                ..
            } => break *referee,
            _ => return None,
        }
    };
    matches!(
        checked.type_reference_table.type_reference(referee),
        checked_trees::types::TypeReferenceNode::Slice { .. }
    )
    .then_some(referee)
}

/// Domain qualifications collected above a view result's reference shell,
/// mirroring `parameter_qualifications` in the checked side: constraints
/// inside the borrow belong to the carrier's own shape and stop the slice
/// resolution before this point is ever reached.
fn view_result_qualifications(
    checked: &CheckedTrees,
    mut reference: checked_trees::types::TypeReferenceHandle,
) -> Option<Vec<language_semantics::SemanticDomainId>> {
    let mut qualifications = Vec::new();
    while let checked_trees::types::TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = checked.type_reference_table.type_reference(reference)
    {
        let retained = checked.type_reference_table.constraints(*constraints);
        if retained.len() != constraints.len() {
            return None;
        }
        for constraint in retained {
            let checked_trees::types::TypeConstraintNode::Domain(domain) = constraint else {
                return None;
            };
            if !domain.semantic_id.is_valid() {
                return None;
            }
            qualifications.push(domain.semantic_id);
        }
        reference = *base_type;
    }
    qualifications.sort_by_key(|domain| domain.0);
    qualifications.dedup();
    Some(qualifications)
}

pub(super) fn validate(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedControlResultPlan::Structural(_) = &plan.result else {
        return unsupported("case return has no structural result signature");
    };
    let CheckedComposedUnitControlTerminatorPlan::ReturnCase { result } = &state.terminator else {
        return unsupported("case return terminator absent");
    };
    validate_case(checked, source, state, ordinal, result)
}

pub(super) fn validate_case(
    checked: &CheckedTrees,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
    result: &checked_trees::CheckedStructuralCaseReturnPlan,
) -> Result<(), LoweringError> {
    let checked_trees::CheckedStructuralCaseReturnPlan {
        statement_ordinal,
        case_identity,
        fields,
    } = result;
    if *statement_ordinal as usize != ordinal {
        return unsupported("case return statement moved");
    }
    let checked_trees::types::TypeReferenceNode::Named { symbol, .. } = checked
        .type_reference_table
        .type_reference(source.return_type)
    else {
        return unsupported("case result is not a nominal sum");
    };
    let expression =
        crate::expression_preparation::source_custody::guarded_exits::completion_expression(
            checked, source, ordinal,
        )?;
    let (case_symbol, authored) = match checked.expression_table.expression(expression) {
        ExpressionNode::StructLiteral(literal) if literal.type_symbol == *symbol => (
            literal.case_symbol.ok_or(LoweringError::Unsupported(
                "case constructor has no selected case",
            ))?,
            checked.expression_table.struct_fields(literal.fields),
        ),
        ExpressionNode::Name(path)
            if path.head_symbol == *symbol
                && checked
                    .expression_table
                    .name_path_members(path.members)
                    .len()
                    == 2 =>
        {
            (path.symbol, &[][..])
        }
        _ => return unsupported("case return does not construct its declared result"),
    };
    let declaration = checked
        .data_definitions()
        .iter()
        .find(|data| data.symbol == *symbol)
        .ok_or(LoweringError::Unsupported(
            "case result declaration missing",
        ))?;
    let variant = checked
        .data_members(declaration)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == case_symbol => Some(variant),
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "case return selected a foreign case",
        ))?;
    let identity = variant
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| variant.name.as_str().to_owned());
    let declarations = checked.data_payload_fields(variant);
    if identity != *case_identity
        || authored.len() != fields.len()
        || fields.len() != declarations.len()
    {
        return unsupported("case return field or case roster drifted");
    }
    for (field_index, (authored, field)) in authored.iter().zip(fields).enumerate() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.symbol == authored.field_symbol)
            .ok_or(LoweringError::Unsupported(
                "case return field has a foreign declaration",
            ))?;
        let identity = declaration
            .identity
            .map(|identity| format!("#{identity}"))
            .unwrap_or_else(|| declaration.name.as_str().to_owned());
        if field.field_ordinal as usize != field_index
            || identity != field.field_identity
            || checked.primitive_type_reference(declaration.type_reference)
                != Some(field.primitive_type)
            || fields[..field_index]
                .iter()
                .any(|previous| previous.field_identity == field.field_identity)
        {
            return unsupported("case return field identity or evaluation order drifted");
        }
        let (binding, retained) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(
                state.state,
                *statement_ordinal,
                CheckedScalarExpressionRole::ReturnCaseField {
                    field_ordinal: field.field_ordinal,
                },
            )
            .ok_or(LoweringError::Unsupported(
                "case return field lost its checked expression",
            ))?;
        if binding.expression != authored.value
            || binding.destination.is_valid()
            || retained != &field.expression
        {
            return unsupported("case return field expression disagrees with source");
        }
        crate::expression_preparation::source_custody::validate_pure(
            checked,
            binding,
            terminal_scalar_type(field.primitive_type)?,
        )?;
    }
    Ok(())
}

/// Rejoin a scalar-result state's completion with its authored tail. An exit
/// roster is the ordinary single-state one and must begin exactly at this
/// state's terminator. A binding completion returns the final expression
/// through the operation that binds it (whose value `body::validate`
/// rejoins), or a final name through the earlier immutable binding it reads.
pub(super) fn validate_scalar(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    completion: &checked_trees::CheckedScalarReturnPlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    let CheckedControlResultPlan::Scalar { primitive_type } = plan.result else {
        return unsupported("scalar graph return has no scalar result signature");
    };
    let binding = match completion {
        checked_trees::CheckedScalarReturnPlan::Exits(exits) => {
            if exits.primitive_type != primitive_type
                || crate::unit::attached_unit::scalar_completion::control::validate_exits(
                    checked,
                    state.state,
                    exits,
                )? != ordinal
            {
                return unsupported("scalar graph exits drifted from their authored tail");
            }
            return Ok(());
        }
        checked_trees::CheckedScalarReturnPlan::Binding(binding) => binding,
    };
    let statements = checked.statement_table.statements(source.statement_nodes);
    let (Some([checked_trees::statement::StatementNode::Expression(expression)]), true) = (
        statements.get(ordinal..),
        binding.primitive_type == primitive_type,
    ) else {
        return unsupported("scalar graph return has no final authored expression");
    };
    let mut producers = state.operations.iter().filter(|operation| {
        matches!(operation,
            CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, .. }
            | CheckedUnitEffectOperationPlan::ScalarCall { result, .. } if result == binding)
    });
    if producers.next().is_none() || producers.next().is_some() {
        return unsupported("scalar graph return has no unique producer");
    }
    if binding.statement_index as usize == ordinal {
        return Ok(());
    }
    let Some(checked_trees::statement::StatementNode::LocalData(local)) =
        statements.get(binding.statement_index as usize)
    else {
        return unsupported("scalar graph return name has no immutable binding");
    };
    let ExpressionNode::Name(name) = checked.expression_table.expression(*expression) else {
        return unsupported("scalar graph return reads neither its producer nor a local");
    };
    if local.is_mutable
        || !local.symbol.is_valid()
        || name.symbol != local.symbol
        || name.head_symbol != local.symbol
        || checked
            .expression_table
            .name_path_members(name.members)
            .len()
            != 1
        || checked.primitive_type_reference(local.type_reference) != Some(binding.primitive_type)
    {
        return unsupported("scalar graph return name differs from its exact binding");
    }
    Ok(())
}

pub(super) fn validate_structural(
    checked: &CheckedTrees,
    plan: &CheckedComposedUnitControlMachinePlan,
    source: &checked_trees::state::State,
    state: &CheckedComposedUnitControlStatePlan,
    ordinal: usize,
) -> Result<(), LoweringError> {
    use checked_trees::CheckedUnitStructuralArgumentSourcePlan;
    let (
        CheckedControlResultPlan::Structural(signature),
        CheckedComposedUnitControlTerminatorPlan::ReturnStructural { result },
    ) = (&plan.result, &state.terminator)
    else {
        return unsupported("structural return has no result signature");
    };
    if signature.type_identity != result.type_identity
        || signature.multiplicity != result.multiplicity
    {
        return unsupported("structural return changed its declared result custody");
    }
    let Some(checked_trees::statement::StatementNode::Expression(expression)) = checked
        .statement_table
        .statements(source.statement_nodes)
        .get(ordinal)
    else {
        return unsupported("structural return has no authored completion");
    };
    match result.source {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            let mut producers = state
                .operations
                .iter()
                .filter_map(|operation| match operation {
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                        result,
                        discard_result_on_return: false,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::StructuralCall {
                        result,
                        discard_result_on_return: false,
                        ..
                    }
                    | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                        result,
                        discard_result_on_return: false,
                        ..
                    } if result.binding_ordinal == binding_ordinal => Some((operation, result)),
                    _ => None,
                });
            let (operation, binding) = producers.next().ok_or(LoweringError::Unsupported(
                "structural return producer absent",
            ))?;
            if producers.next().is_some()
                || binding.type_identity != result.type_identity
                || binding.multiplicity != result.multiplicity
                || binding.statement_index as usize > ordinal
            {
                return unsupported("structural return producer custody drifted");
            }
            if binding.statement_index as usize == ordinal {
                if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::StructuralCall { .. }
                        | CheckedUnitEffectOperationPlan::BoundaryStructuralCall { .. }
                ) {
                    crate::unit::attached_unit::structural_calls::validate_custody(
                        checked,
                        plan.machine,
                        state.state,
                        operation,
                    )?;
                    crate::emission::call_source_custody::validate_operation(
                        checked,
                        plan.machine,
                        state.state,
                        operation,
                        &state.structural_parameters,
                    )?;
                } else if matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::EstablishStructuralValue { .. }
                ) {
                    crate::expression_preparation::source_custody::structural::validate(
                        checked,
                        plan.machine,
                        state.state,
                        operation,
                    )?;
                } else {
                    return unsupported(
                        "structural return tail producer requires exact value custody",
                    );
                }
            } else {
                let Some(checked_trees::statement::StatementNode::LocalData(local)) = checked
                    .statement_table
                    .statements(source.statement_nodes)
                    .get(binding.statement_index as usize)
                else {
                    return unsupported("structural return does not name its produced local");
                };
                if !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(path) if path.symbol == local.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
                {
                    return unsupported("structural return exchanged its produced local");
                }
            }
        }
        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter = state
                .structural_parameters
                .get(parameter_index as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural return parameter absent",
                ))?;
            let declaration = checked
                .state_parameters(source)
                .get(parameter.position as usize)
                .ok_or(LoweringError::Unsupported(
                    "structural return source parameter absent",
                ))?;
            if parameter.type_identity != result.type_identity
                || parameter.multiplicity != result.multiplicity
                || !matches!(
                    parameter.access,
                    checked_trees::CheckedStructuralAccess::Owned
                        | checked_trees::CheckedStructuralAccess::SharedBorrow
                )
                || !matches!(checked.expression_table.expression(*expression), ExpressionNode::Name(path) if path.symbol == declaration.symbol && checked.expression_table.name_path_members(path.members).len() == 1)
            {
                return unsupported("structural return exchanged its owned parameter");
            }
        }
        _ => return unsupported("structural return requires whole established custody"),
    }
    Ok(())
}

pub(super) fn result(
    plan: &CheckedControlResultPlan,
    catalogs: &mut catalogs::ComposedCatalogs,
    places: &mut Vec<StructuralPlaceDeclaration>,
) -> Result<TerminalMachineResult, LoweringError> {
    let result = match plan {
        CheckedControlResultPlan::Unit => return Ok(TerminalMachineResult::Unit),
        CheckedControlResultPlan::Scalar { primitive_type } => {
            return Ok(TerminalMachineResult::Scalar(ValueDeclaration {
                qualifications: Default::default(),
                id: value_id(allocate_dense(&mut catalogs.next_value)?),
                scalar_type: terminal_scalar_type(*primitive_type)?,
            }));
        }
        CheckedControlResultPlan::Structural(result) => result,
    };
    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
    places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::Result,
    });
    Ok(TerminalMachineResult::Structural(
        StructuralResultDeclaration {
            reference_sources: Vec::new(),
            place,
            structural_type: lookup_type_id(&catalogs.type_ids, &result.type_identity)?,
            multiplicity: match result.multiplicity {
                Multiplicity::Affine => StructuralMultiplicity::Affine,
                Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                Multiplicity::Linear => StructuralMultiplicity::Linear,
            },
            qualifications: result
                .qualifications
                .iter()
                .map(|domain| lookup_domain_id(&catalogs.domain_ids, *domain))
                .collect::<Result<Vec<_>, _>>()?,
            projected_qualifications:
                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                    &result.projected_qualifications,
                    &catalogs.domain_ids,
                )?,
        },
    ))
}

pub(super) fn emit(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    result: &TerminalMachineResult,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    catalogs: &mut catalogs::ComposedCatalogs,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<PlaceId>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ReturnCase {
        result: construction,
    } = &state.terminator
    else {
        return Ok(None);
    };
    emit_case(
        checked,
        state,
        construction,
        result,
        bindings,
        catalogs,
        values,
        next_value,
        operations,
    )
}

pub(super) fn emit_case(
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    construction: &checked_trees::CheckedStructuralCaseReturnPlan,
    result: &TerminalMachineResult,
    bindings: &crate::expression_preparation::bindings::ScalarBindings,
    catalogs: &mut catalogs::ComposedCatalogs,
    values: &[ValueDeclaration],
    next_value: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<Option<PlaceId>, LoweringError> {
    let checked_trees::CheckedStructuralCaseReturnPlan {
        statement_ordinal,
        case_identity,
        fields,
    } = construction;
    let result = result.structural().ok_or(LoweringError::Unsupported(
        "case construction result signature missing",
    ))?;
    let declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == result.structural_type)
        .ok_or(LoweringError::Unsupported(
            "case construction result type missing",
        ))?;
    let StructuralTypeShape::Sum { cases } = &declaration.shape else {
        return unsupported("case construction result is not a sum");
    };
    let case = cases
        .iter()
        .find(|case| case.identity == *case_identity)
        .ok_or(LoweringError::Unsupported(
            "case construction declaration missing",
        ))?;
    if fields.len() != case.fields.len() {
        return unsupported("case construction declaration roster drifted");
    }
    let mut evaluated = Vec::new();
    for field in fields {
        let declaration = case
            .fields
            .iter()
            .find(|declaration| declaration.identity == field.field_identity)
            .ok_or(LoweringError::Unsupported(
                "case construction field declaration missing",
            ))?;
        let expression = bindings.expression_at(
            checked,
            state.state,
            *statement_ordinal,
            CheckedScalarExpressionRole::ReturnCaseField {
                field_ordinal: field.field_ordinal,
            },
        )?;
        if declaration.field_type.scalar_type() != Some(expression.scalar_type())
            || direct_expression_contains_short_circuit(&expression)
        {
            return unsupported("case construction needs an exact branch-free scalar operand");
        }
        validate_direct_parameter_types(
            &expression,
            &values
                .iter()
                .map(|value| value.scalar_type)
                .collect::<Vec<_>>(),
        )?;
        let value = emit_direct_expression(&expression, values, next_value, operations);
        let range_obligation = if matches!(
            declaration.field_type,
            StructuralFieldType::BoundedInteger(_)
        ) {
            Some(obligation_id(allocate_dense(
                &mut catalogs.scalar_calls.next_call_obligation,
            )?))
        } else {
            None
        };
        evaluated.push(terminal_psi::ScalarCaseField {
            field: declaration.id,
            value,
            range_obligation,
        });
    }
    evaluated.sort_by_key(|field| field.field);
    let operation = operations.allocate();
    let place = place_id(allocate_dense(&mut catalogs.next_place)?);
    catalogs.result_places.push(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: operation,
            structural_type: result.structural_type,
        },
    });
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id: operation,
        result: terminal_psi::OperationResult::Structural(
            terminal_psi::StructuralOperationResult {
                qualification_establishments: Vec::new(),
                place,
                structural_type: result.structural_type,
                multiplicity: result.multiplicity,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
        ),
        kind: OperationKind::EstablishScalarCase {
            result_case: case.id,
            fields: evaluated,
        },
    });
    Ok(Some(place))
}
