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
    operations: &OperationBuffer,
    next_value: &mut u64,
) -> Result<Option<PreparedCases<'a>>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { result, cases } = &state.terminator
    else {
        return Ok(None);
    };
    let mut occurrences = operations.source_calls.iter().filter(|occurrence| {
        occurrence.source_state == state.state
            && occurrence.statement_index == result.statement_index as usize
            && occurrence.call_ordinal == 0
    });
    let occurrence = occurrences.next().ok_or(LoweringError::Unsupported(
        "Unit graph case result has no emitted source call",
    ))?;
    if occurrences.next().is_some() {
        return unsupported("Unit graph case result has duplicate source calls");
    }
    let operation = operations
        .iter()
        .find(|operation| operation.id == occurrence.terminal_operation)
        .ok_or(LoweringError::Unsupported(
            "Unit graph case producer is missing",
        ))?;
    let OperationResult::Structural(produced) = &operation.result else {
        return unsupported("Unit graph case producer lost its structural result");
    };
    let declaration = catalogs
        .structural_types
        .iter()
        .find(|declaration| declaration.id == produced.structural_type)
        .ok_or(LoweringError::Unsupported(
            "Unit graph case type is missing",
        ))?;
    let StructuralTypeShape::Sum { cases: declared } = &declaration.shape else {
        return unsupported("Unit graph case result is not a closed sum");
    };
    if declaration.identity != result.type_identity
        || produced.multiplicity != StructuralMultiplicity::Affine
        || !produced.qualifications.is_empty()
        || !produced.projected_qualifications.is_empty()
        || !produced.claims.is_empty()
        || declared.len() != cases.len()
    {
        return unsupported("Unit graph case result or roster drifted");
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
        source: produced.place,
        cases,
    }))
}
