//! Project an invocation's retained value tree onto the child that supplied it.
//! Destination paths identify fields independently of authored evaluation order.
//! Each selected child keeps its own completion context and type: siblings may
//! have copied the same source at different times, or may carry live loans.

use checked_trees::{FlowControlFacts, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, PlaceRoot, PlaceSegment};
use typed_trees::types::TypeReferenceHandle;
pub(crate) use validation::completed_storage::{
    captured_operand_child_at_path, captured_retention_profiles_at_path, operand_child_type,
    project_captured_view_place, reference_captures_at_path,
};

/// Contextual construction uses the bound destination declaration. A source
/// declaration is retained only when that destination cannot be projected.
/// Neither path infers a type from a sibling's expression handle.
pub(crate) struct CapturedValue<'capture> {
    /// The actual storage observation and its live qualification namespace.
    /// A reaching value origin cannot erase evidence attached to this copy.
    pub observation: &'capture FlowOperandFact,
    pub observation_type: TypeReferenceHandle,
    pub observation_projection: super::LiteralExpressionProjection,
    pub operand: &'capture FlowOperandFact,
    pub type_reference: TypeReferenceHandle,
    pub projection: super::LiteralExpressionProjection,
}

/// Project evidence onto the storage this completed value actually denotes.
/// A live view's retained referents select backing storage; detached values
/// and unresolved views keep their own observation namespace. No initializer
/// or callee body is consulted to replace a missing capture.
pub(crate) struct CapturedValuePlace {
    pub storage: validation::CanonicalPlace,
    pub projected: validation::CanonicalPlace,
}

/// Select lifetime evidence through completed copies, independently of address
/// projection. The stored-value links point to earlier captures, never to a
/// source initializer or storage reread at the consuming call.
/// A complete lifetime frontier for this value's declared reference leaves.
/// Known view-free values produce an empty set. A missing reference leaf is
/// unresolved coverage, not evidence that an earlier backing can die.
pub(crate) fn captured_lifetime_roots(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
) -> Option<Vec<PlaceRoot>> {
    let paths =
        validation::borrow_contracts::borrow_carrying_owner_paths(program, operand.type_reference)?;
    let mut roots = Vec::new();
    for path in paths {
        let path = crate::borrow::owner_place_segments(&path);
        for leaf in captured_retention_profiles_at_path(program, control, operand, &path)? {
            for root in control.retained_roots(leaf)? {
                if !roots.contains(root) {
                    roots.push(*root);
                }
            }
        }
    }
    Some(roots)
}

pub(crate) fn captured_value_places(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    value: &CapturedValue<'_>,
) -> Option<Vec<CapturedValuePlace>> {
    captured_places(
        program,
        flow,
        value.operand,
        value.type_reference,
        &value.projection,
    )
}

/// A reaching store can expose an earlier producer, but the later observation
/// still owns its storage snapshots. Both projections obey the same view law.
pub(crate) fn captured_observation_places(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    value: &CapturedValue<'_>,
) -> Option<Vec<CapturedValuePlace>> {
    captured_places(
        program,
        flow,
        value.observation,
        value.observation_type,
        &value.observation_projection,
    )
}

fn captured_places(
    program: &dyn ::validation::CheckingInput,
    flow: &FlowFacts,
    operand: &FlowOperandFact,
    type_reference: TypeReferenceHandle,
    projection: &super::LiteralExpressionProjection,
) -> Option<Vec<CapturedValuePlace>> {
    let referents = flow
        .control
        .operand_referents
        .span_or_empty(operand.referents);
    if !super::operand_contexts::operand_binds_live_view(program, type_reference)
        || referents.is_empty()
    {
        let mut place = validation::canonical_place_from_expression_in_state(
            program,
            operand.state_symbol,
            operand.statement_index,
            projection.expression,
        )?;
        let storage = place.clone();
        place.extend_segments(&projection.remaining);
        return Some(vec![CapturedValuePlace {
            storage,
            projected: place,
        }]);
    }
    referents
        .iter()
        .map(|referent| {
            let mut place = validation::CanonicalPlace {
                root: referent.root,
                segments: flow
                    .control
                    .operand_referent_segments
                    .span_or_empty(referent.segments)
                    .to_vec(),
            };
            let storage = place.clone();
            place = project_captured_view_place(place, &projection.remaining)?;
            Some(CapturedValuePlace {
                storage,
                projected: place,
            })
        })
        .collect()
}

/// Project once across the captured address/value boundary. The later path
/// dimensions are pointwise nested data, not additional address windows.
/// A dynamic selector can produce several candidates. Consumers must prove
/// each independently; combining their contexts would invent a capture epoch.
pub(crate) fn captured_operand_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: &'capture FlowFacts,
    semantic: &FactPlan,
    state: &FlowStateFact,
    operand: &'capture FlowOperandFact,
    type_reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<Vec<CapturedValue<'capture>>> {
    let control = &flow.control;
    let mut values = Vec::new();
    for projection in
        super::literal_value_projections(program, operand.expression, type_reference, path, false)?
    {
        // An omitted field belongs to its evaluated constructor's result.
        // Keep that actual completion and project its inline storage; never
        // invent an expression operand for a field the source did not write.
        let projection = match projection.source {
            super::LiteralValueSource::Expression(expression) => {
                super::LiteralExpressionProjection {
                    expression,
                    remaining: projection.remaining,
                    destination: projection.destination,
                }
            }
            super::LiteralValueSource::OmittedZero {
                constructor,
                constructor_path_length,
                ..
            } => super::LiteralExpressionProjection {
                expression: constructor,
                remaining: projection.destination[constructor_path_length..].to_vec(),
                destination: projection.destination[..constructor_path_length].to_vec(),
            },
        };
        let (captured, selected_type) = captured_operand_at_path(
            program,
            control,
            operand,
            type_reference,
            &projection.destination,
        )?;
        // The resolved constructor path and the retained evaluation must
        // name the same authored occurrence, not just the same storage.
        if captured.expression != projection.expression {
            return None;
        }
        let value = CapturedValue {
            observation: captured,
            observation_type: selected_type,
            observation_projection: projection.clone(),
            operand: captured,
            type_reference: selected_type,
            projection,
        };
        if let Some(mut stored) =
            reaching_store_values(program, flow, semantic, state, &value, None)
        {
            for origin in &mut stored {
                origin.observation = value.observation;
                origin.observation_type = value.observation_type;
                origin.observation_projection = value.observation_projection.clone();
            }
            values.extend(stored);
        } else {
            values.push(value);
        }
    }
    Some(values)
}

/// An invocation observes a borrowed scalar after every operand completes.
/// Resolve its reaching value at that epoch, not when its address was captured.
/// Snapshot operands already have their own completed origin and stay unchanged.
pub(crate) fn observe_captured_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: &'capture FlowFacts,
    semantic: &FactPlan,
    state: &FlowStateFact,
    values: Vec<CapturedValue<'capture>>,
    epoch: &'capture FlowOperandFact,
) -> Vec<CapturedValue<'capture>> {
    let mut observed = Vec::new();
    for value in values {
        if super::operand_contexts::operand_binds_live_view(program, value.type_reference)
            && let Some(mut stored) =
                reaching_store_values(program, flow, semantic, state, &value, Some(epoch))
        {
            for origin in &mut stored {
                origin.observation = value.observation;
                origin.observation_type = value.observation_type;
                origin.observation_projection = value.observation_projection.clone();
            }
            observed.extend(stored);
        } else {
            observed.push(value);
        }
    }
    observed
}

/// A stored copy retains the completed RHS, not a fresh interpretation of its
/// initializer. Walk produced stores and writes backwards; each successful hop
/// selects a capture preceding its store on the immutable storage timeline. Unknown writes stop provenance,
/// leaving the current storage observation and its own contexts intact.
fn reaching_store_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: &'capture FlowFacts,
    semantic: &FactPlan,
    state: &FlowStateFact,
    value: &CapturedValue<'capture>,
    observation: Option<&FlowOperandFact>,
) -> Option<Vec<CapturedValue<'capture>>> {
    let operand = observation.unwrap_or(value.operand);
    let observed_type = validation::project_type_reference_from_segments(
        program,
        value.type_reference,
        &value.projection.remaining,
    )?;
    // An owned linear record need not be copyable for a stable scalar field
    // read to retain its reaching store. Judge the observed projection, not
    // the containing owner's unrelated custody or mutable fields. A live view
    // still observes invocation storage; tracing its old store would freeze
    // referent predicates across later operand writes.
    if (observation.is_none()
        && super::operand_contexts::operand_binds_live_view(program, value.type_reference))
        || operand.state_symbol != state.state_symbol
        || !super::operand_contexts::operand_binds_stable_copy(program, observed_type)
    {
        return None;
    }
    let sources = captured_value_places(program, flow, value)?;
    let [source] = sources.as_slice() else {
        return None;
    };
    let source = &source.projected;
    if !matches!(source.root, PlaceRoot::Symbol(_)) {
        return None;
    }
    program.state_by_symbol(state.state_symbol)?;
    let captures = super::stored_values::at_snapshot(
        program,
        &flow.control,
        operand.storage_snapshot,
        source,
        super::stored_values::StoredObservation::Contents,
    )?
    .captures()?;
    let mut values = Vec::new();
    for selected in captures {
        let stored = flow.control.operands.get(selected.capture);
        // Zero formation has no evaluated RHS. Its establishment belongs to
        // the observed storage, not an invalid completed producer. Every
        // predecessor must support the requested detached copy.
        if !program
            .expression_table
            .expression_is_valid(stored.expression)
            || !selected.reference.is_valid()
            || !validation::project_type_reference_from_segments(
                program,
                selected.reference,
                &selected.projection,
            )
            .is_some_and(|projected| {
                super::operand_contexts::operand_binds_stable_copy(program, projected)
            })
        {
            return None;
        }
        values.extend(captured_operand_values(
            program,
            flow,
            semantic,
            state,
            stored,
            selected.reference,
            &selected.projection,
        )?);
    }
    Some(values)
}

pub(crate) fn captured_operand_at_path<'capture>(
    program: &dyn ::validation::CheckingInput,
    control: &'capture FlowControlFacts,
    operand: &'capture FlowOperandFact,
    type_reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<(&'capture FlowOperandFact, TypeReferenceHandle)> {
    let (operand, reference, _) =
        projected_capture(program, control, operand, type_reference, path)?;
    Some((operand, reference))
}

fn projected_capture<'capture>(
    program: &dyn ::validation::CheckingInput,
    control: &'capture FlowControlFacts,
    mut operand: &'capture FlowOperandFact,
    mut type_reference: TypeReferenceHandle,
    mut path: &[PlaceSegment],
) -> Option<(
    &'capture FlowOperandFact,
    TypeReferenceHandle,
    arena::Handle<FlowOperandFact>,
)> {
    let mut selected_handle = arena::Handle::invalid();
    while !path.is_empty() {
        let (child, reference, length) =
            captured_operand_child_at_path(program, control, operand, type_reference, path)?;
        selected_handle = child;
        operand = control.operands.get(child);
        type_reference = reference;
        path = &path[length..];
    }
    type_reference
        .is_valid()
        .then_some((operand, type_reference, selected_handle))
}
