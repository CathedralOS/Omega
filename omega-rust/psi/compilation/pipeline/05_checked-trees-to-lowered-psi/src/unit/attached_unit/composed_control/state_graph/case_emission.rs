//! Selected payload bindings feed ordinary successor arguments and completions.
use super::super::super::super::{
    CheckedComposedUnitControlTerminatorPlan, StructuralCaseId, StructuralFieldId,
    StructuralParameterDeclaration,
};
use super::super::super::{
    PlaceId, StructuralTypeShape, ValueDeclaration, terminal_scalar_type, unsupported,
};
use super::super::{LoweringError, catalogs};
use super::CheckedComposedUnitControlStatePlan;
use crate::emission::operation_emission::buffer::{OperationBuffer, StructuralBinding};

pub(super) struct PreparedCase<'a> {
    pub outcome: &'a checked_trees::CheckedClosedSumCaseOutcomePlan,
    pub identity: StructuralCaseId,
    pub fields: Vec<StructuralFieldId>,
    pub values: Vec<(u32, ValueDeclaration)>,
}

pub(super) struct PreparedCases<'a> {
    pub source: PlaceId,
    pub path: Vec<terminal_psi::StructuralPathSegment>,
    pub cases: Vec<PreparedCase<'a>>,
}

pub(super) fn prepare<'a>(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    state: &'a CheckedComposedUnitControlStatePlan,
    catalogs: &catalogs::ComposedCatalogs,
    parameters: &[StructuralParameterDeclaration],
    operations: &OperationBuffer,
    next_value: &mut u64,
) -> Result<Option<PreparedCases<'a>>, LoweringError> {
    let CheckedComposedUnitControlTerminatorPlan::ClosedSum { subject, cases, .. } =
        &state.terminator
    else {
        return Ok(None);
    };
    // Stage 04's closed-sum planner admitted the original carrier. Its path
    // selects the inspected sum without transferring that carrier's
    // ownership or loan.
    let (place, structural_type) = match subject.source {
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index } => {
            let parameter =
                parameters
                    .get(parameter_index as usize)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph case parameter missing",
                    ))?;
            (parameter.place, parameter.structural_type)
        }
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal,
        } => {
            let produced = result(state, binding_ordinal, operations)?;
            (produced.place(), produced.structural_type())
        }
        _ => return unsupported("Unit graph case subject source unsupported"),
    };
    let prepared_path =
        crate::expression_preparation::bindings::structural_paths::prepare_argument_path(
            identities,
            &subject.path,
            structural_type,
            &catalogs.structural_types,
        )?;
    let path = prepared_path
        .into_iter()
        .map(|segment| match segment {
            crate::expression_preparation::bindings::structural_paths::PreparedStructuralPathSegment::Static(segment) => Ok(segment),
            _ => unsupported("case dispatch runtime projection requires a selector evidence owner"),
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    // A captured reference starts a new storage region, not a canonical owned
    // subtree. Keep its explicit crossing as the selected payload's identity.
    let declaration = terminal_semantics::runtime_structural_path_tip(
        catalogs.structural_types.iter(),
        structural_type,
        &path,
    )
    .ok_or(LoweringError::Unsupported(
        "Unit graph case projection has no exact declared carrier",
    ))?;
    let StructuralTypeShape::Sum { cases: declared } = &declaration.shape else {
        return unsupported("Unit graph case subject is not a sum");
    };
    let cases = declared
        .iter()
        .map(|declared| {
            let mut matching = cases.iter().filter(|case| {
                case.case_identity.spelling()
                    == identities.text(declared.identity).unwrap_or_default()
            });
            let case = matching.next().ok_or(LoweringError::Unsupported(
                "Unit graph declared case lost its successor",
            ))?;
            if matching.next().is_some() {
                return unsupported("Unit graph declared case has duplicate successors");
            }
            let mut fields = Vec::new();
            let mut values = Vec::new();
            for payload in &case.payloads {
                let field = terminal_psi::field_by_identity(
                    &declared.fields,
                    identities.intern(&payload.field_identity.spelling()),
                )
                .ok_or(LoweringError::Unsupported(
                    "Unit graph payload field is missing",
                ))?;
                let scalar_type = terminal_scalar_type(payload.primitive_type)?;
                fields.push(field.id);
                values.push((
                    payload.target_scalar_parameter_index,
                    crate::terminal_identities::fresh_scalar_declaration(next_value, scalar_type)?,
                ));
            }
            // A wildcard returning arm shared by several cases cannot name
            // a case-specific payload. Its common completion block therefore
            // receives no payload formals. All other arms use one snapshot
            // namespace, independent of pure expressions versus computations.
            let shared_return = matches!(
                case.outcome,
                checked_trees::CheckedClosedSumCaseOutcomePlan::Return(_)
            ) && fields.is_empty()
                && cases
                    .iter()
                    .filter(|other| other.outcome == case.outcome)
                    .count()
                    > 1;
            for field in declared.fields.iter().filter(|_| !shared_return) {
                if field.relevance.is_erased() || fields.contains(&field.id) {
                    continue;
                }
                // Atomic representation is scalar, but observing it is an
                // authored access event, never an implicit payload snapshot.
                if matches!(
                    field.field_type,
                    terminal_psi::StructuralFieldType::Atomic(_)
                ) {
                    continue;
                }
                let Some(scalar_type) = field.field_type.scalar_type() else {
                    continue;
                };
                fields.push(field.id);
                values.push((
                    u32::MAX,
                    crate::terminal_identities::fresh_scalar_declaration(next_value, scalar_type)?,
                ));
            }
            Ok(PreparedCase {
                outcome: &case.outcome,
                identity: declared.id,
                fields,
                values,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    Ok(Some(PreparedCases {
        source: place,
        path,
        cases,
    }))
}

/// Rejoin a state-local result to its exact emitted source occurrence.
pub(in crate::unit::attached_unit) fn result<'a>(
    state: &CheckedComposedUnitControlStatePlan,
    binding_ordinal: u32,
    operations: &'a OperationBuffer,
) -> Result<std::borrow::Cow<'a, StructuralBinding>, LoweringError> {
    if let Some((_, produced)) = operations
        .structural_values
        .iter()
        .find(|(ordinal, _)| *ordinal == binding_ordinal)
    {
        return Ok(std::borrow::Cow::Borrowed(produced));
    }
    let mut bindings = state.operations.iter().filter_map(|operation| {
        // A kept structural result of a call, ordinary or boundary.
        let call = operation.call()?;
        let result = call.structural_result()?;
        (call.discard_result_on_return() == Some(false)
            && result.binding_ordinal == binding_ordinal)
            .then_some((&call.coordinate, result))
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
        .find(|operation| occurrence.terminal_site.operation() == Some(operation.id))
        .ok_or(LoweringError::Unsupported(
            "Unit graph result operation missing",
        ))?;
    let Some(produced) = operation.result.as_structural() else {
        return unsupported("Unit graph call lost structural result");
    };
    Ok(std::borrow::Cow::Owned(produced.clone().into()))
}
