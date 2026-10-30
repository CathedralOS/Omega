//! Predicate subjects bind completed values without performing their reads.
//! Owned paths preserve conditional payload coordinates; actual reference
//! crossings keep the ordinary captured-read and live-memory obligations.
use super::Actual;
use checked_trees::{CheckFacts, FlowCallFact, FlowStateFact};

/// A formula's owned payload projections are conditional coordinates, not
/// eagerly performed reads. Retain the already-completed carrier and its typed
/// path even when another case is active. Reference crossings still use the
/// ordinary observed-read route and cannot freeze pointee contents here.
pub(super) fn values<'facts>(
    program: &dyn validation::CheckingInput,
    facts: &'facts CheckFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    actual: &Actual<'_>,
) -> Option<Vec<crate::flow::CapturedValue<'facts>>> {
    if !actual.path.is_empty()
        && actual
            .path
            .iter()
            .all(crate::flow::place_segment_has_static_identity)
        && validation::reference_shell(program, actual.parameter.type_reference).is_none()
        && validation::owned_projection_type_reference(
            program,
            actual.parameter.type_reference,
            &actual.path,
        )
        .is_some_and(|selected| validation::reference_shell(program, selected).is_none())
    {
        let root = Actual {
            parameter: actual.parameter,
            argument: actual.argument,
            path: Vec::new(),
            operand_position: actual.operand_position,
        };
        let mut values = super::captured_values(program, facts, state, call, &root)?;
        for value in &mut values {
            value
                .observation_projection
                .remaining
                .extend_from_slice(&actual.path);
            value.projection.remaining.extend_from_slice(&actual.path);
        }
        Some(values)
    } else {
        super::captured_values(program, facts, state, call, actual)
    }
}

/// Bind a structural tag observation to the completed actual's existing
/// namespace. This publishes no selected case or payload read permission.
pub(super) fn current_case(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    captured: &crate::flow::CapturedValue<'_>,
    bindings: &mut Vec<crate::facts::boolean_predicates::BoundObservation>,
) -> Option<facts::BooleanPredicateOperand> {
    let places = crate::flow::captured_observation_places(program, &facts.flow, captured)?;
    let [observed] = places.as_slice() else {
        return None;
    };
    let reference = validation::project_type_reference_from_segments(
        program,
        captured.observation_type,
        &captured.observation_projection.remaining,
    )
    .and_then(|reference| validation::field_domains::readable_type_reference(program, reference))?;
    let data = program.closed_data_definition(reference)?;
    if !program
        .data_members(data)
        .iter()
        .any(|member| matches!(member, typed_trees::data::DataMember::Variant(_)))
    {
        return None;
    }
    let operand =
        facts::BooleanPredicateOperand::CaseSubject(checked_trees::CheckedStructuralCaseSubject {
            root: checked_trees::CheckedStructuralCaseRoot::Parameter {
                position: u32::try_from(bindings.len()).ok()?,
            },
            path: Vec::new(),
        });
    bindings.push(crate::facts::boolean_predicates::BoundObservation {
        operand: operand.clone(),
        place: observed.projected.clone(),
        type_reference: reference,
    });
    Some(operand)
}
