//! Selected payload bindings feed the ordinary successor argument path.

use super::*;

pub(super) struct PreparedCase<'a> {
    pub successor: &'a CheckedStructuralControlSuccessorPlan,
    pub identity: StructuralCaseId,
    pub fields: Vec<StructuralFieldId>,
    pub values: Vec<(u32, ValueDeclaration)>,
}

pub(super) struct PreparedCases<'a> {
    pub source: PlaceId,
    pub cases: Vec<PreparedCase<'a>>,
}

pub(super) fn prepare<'a>(
    state: &'a CheckedComposedUnitControlStatePlan,
    catalogs: &catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    operations: &OperationBuffer,
    next_value: &mut u64,
) -> Result<Option<PreparedCases<'a>>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases } = &state.terminator
    else {
        return Ok(None);
    };
    if subject.access != checked_trees::CheckedStructuralAccess::Owned || !subject.path.is_empty() {
        return unsupported("Unit graph case subject lacks owned custody");
    }
    let (place, structural_type) = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter =
                parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph case parameter missing",
                    ))?;
            if parameter.access != StructuralAccess::Owned
                || parameter.multiplicity != StructuralMultiplicity::Affine
                || !parameter.qualifications.is_empty()
            {
                return unsupported("Unit graph case parameter custody drifted");
            }
            (parameter.place, parameter.structural_type)
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            let produced = result(state, binding_ordinal, operations)?;
            (produced.place, produced.structural_type)
        }
        _ => return unsupported("Unit graph case subject source unsupported"),
    };
    let declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == structural_type)
        .ok_or(LoweringError::Unsupported("Unit graph case type missing"))?;
    let StructuralTypeShape::Sum { cases: declared } = &declaration.shape else {
        return unsupported("Unit graph case subject is not a sum");
    };
    if declaration.identity != subject.type_identity || declared.len() != cases.len() {
        return unsupported("Unit graph case type or roster drifted");
    }
    let cases = declared
        .iter()
        .map(|declared| {
            let mut matching = cases
                .iter()
                .filter(|case| case.case_identity == declared.identity);
            let case = matching.next().ok_or(LoweringError::Unsupported(
                "Unit graph declared case lost its successor",
            ))?;
            if matching.next().is_some() {
                return unsupported("Unit graph declared case has duplicate successors");
            }
            let mut fields = Vec::new();
            let mut values = Vec::new();
            for payload in &case.payloads {
                let field = declared
                    .fields
                    .iter()
                    .find(|field| field.identity == payload.field_identity)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph payload field is missing",
                    ))?;
                let scalar_type = terminal_scalar_type(payload.primitive_type)?;
                if field.field_type.scalar_type() != Some(scalar_type)
                    || values
                        .iter()
                        .any(|(position, _)| *position == payload.target_scalar_parameter_index)
                {
                    return unsupported("Unit graph payload type or destination drifted");
                }
                fields.push(field.id);
                values.push((
                    payload.target_scalar_parameter_index,
                    ValueDeclaration {
                        qualifications: Default::default(),
                        id: value_id(allocate_dense(next_value)?),
                        scalar_type,
                    },
                ));
            }
            Ok(PreparedCase {
                successor: &case.successor,
                identity: declared.id,
                fields,
                values,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok(Some(PreparedCases {
        source: place,
        cases,
    }))
}

/// Rejoin a state-local result to its exact emitted source occurrence.
pub(super) fn result<'a>(
    state: &CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
    operations: &'a OperationBuffer,
) -> Result<&'a terminal_psi::StructuralOperationResult, LoweringError> {
    if let Some((_, produced)) = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == binding_ordinal)
    {
        return Ok(produced);
    }
    let mut bindings = state
        .operations
        .iter()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result,
                discard_result_on_return: false,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result,
                discard_result_on_return: false,
                ..
            } if result.binding_ordinal == binding_ordinal => Some((coordinate, result)),
            _ => None,
        });
    let (coordinate, binding) = bindings.next().ok_or(LoweringError::Unsupported(
        "Unit graph result binding missing",
    ))?;
    if bindings.next().is_some() || coordinate.statement_index != binding.statement_index {
        return unsupported("Unit graph result binding duplicated or reordered");
    }
    let mut occurrences = operations.source_calls.iter().filter(|occurrence| {
        occurrence.source_state == state.state
            && occurrence.statement_index == coordinate.statement_index as usize
            && occurrence.call_ordinal == coordinate.call_ordinal as usize
    });
    let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
        "Unit graph result source call missing",
    ))?;
    if occurrences.next().is_some() {
        return unsupported("Unit graph result source call duplicated");
    }
    let operation = operations
        .iter()
        .find(|operation| operation.id == occurrence.terminal_operation)
        .ok_or(LoweringError::Unsupported(
            "Unit graph result operation missing",
        ))?;
    let OperationResult::Structural(produced) = &operation.result else {
        return unsupported("Unit graph call lost structural result");
    };
    if produced.multiplicity != StructuralMultiplicity::Affine
        || !produced.qualifications.is_empty()
        || !produced.projected_qualifications.is_empty()
        || !produced.claims.is_empty()
    {
        return unsupported("Unit graph result ownership drifted");
    }
    Ok(produced)
}
