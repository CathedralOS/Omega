//! Project an invocation's retained value tree onto the child that supplied it.
//! Destination paths identify fields independently of authored evaluation order.
//! Each selected child keeps its own completion context and type: siblings may
//! have copied the same source at different times, or may carry live loans.

use checked_trees::{FlowControlFacts, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, PlaceRoot, PlaceSegment};
use typed_trees::types::TypeReferenceHandle;

/// Contextual construction uses the bound destination declaration. A source
/// declaration is retained only when that destination cannot be projected.
/// Neither path infers a type from a sibling's expression handle.
pub(super) fn operand_child_type(
    program: &dyn ::validation::CheckingInput,
    parent: TypeReferenceHandle,
    destination: &[PlaceSegment],
    declared: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    parent
        .is_valid()
        .then(|| validation::project_type_reference_from_segments(program, parent, destination))
        .flatten()
        .or_else(|| declared.is_valid().then_some(declared))
}

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
pub(crate) fn project_captured_view_place(
    mut place: validation::CanonicalPlace,
    path: &[PlaceSegment],
) -> Option<validation::CanonicalPlace> {
    place.segments = facts::project_captured_view_segments(&place.segments, path)?;
    Some(place)
}

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
        if let Some(mut stored) = reaching_store_values(program, flow, semantic, state, &value) {
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

/// A stored copy retains the completed RHS, not a fresh interpretation of its
/// initializer. Walk produced stores and writes backwards; each successful hop
/// strictly decreases the statement frontier. Unknown writes stop provenance,
/// leaving the current storage observation and its own contexts intact.
fn reaching_store_values<'capture>(
    program: &dyn ::validation::CheckingInput,
    flow: &'capture FlowFacts,
    semantic: &FactPlan,
    state: &FlowStateFact,
    value: &CapturedValue<'capture>,
) -> Option<Vec<CapturedValue<'capture>>> {
    let operand = value.operand;
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
    if super::operand_contexts::operand_binds_live_view(program, value.type_reference)
        || operand.state_symbol != state.state_symbol
        || !super::operand_contexts::operand_binds_stable_copy(program, observed_type)
    {
        return None;
    }
    let mut source = validation::canonical_place_from_expression_in_state(
        program,
        operand.state_symbol,
        operand.statement_index,
        value.projection.expression,
    )?;
    source.extend_segments(&value.projection.remaining);
    if !matches!(source.root, PlaceRoot::Symbol(_))
        || !super::operand_entry_origin_is_stable(program, flow, operand, &source)
    {
        return None;
    }
    // Storage order and execution order can differ across admitted assembly
    // jumps. A missing executed row cannot assert that a value was preserved.
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    let order = program
        .statement_table
        .admitted_execution_order(typed_state.statement_nodes);
    let end = order.position(operand.statement_index).or_else(|| {
        (operand.statement_index
            == program
                .statement_table
                .statements(typed_state.statement_nodes)
                .len())
        .then_some(order.indices().len())
    })?;
    for &index in order.indices()[..end].iter().rev() {
        let statement = flow.state_statement(state, index)?;
        if semantic.places.is_valid(statement.stored_destination)
            && flow.control.operands.is_valid(statement.stored_value)
        {
            let destination = super::canonical_place_from_semantic_place(
                program,
                semantic,
                semantic.places.get(statement.stored_destination),
            )?;
            if destination.root == source.root
                && destination.segments.len() <= source.segments.len()
                && destination
                    .segments
                    .iter()
                    .zip(&source.segments)
                    .all(|(left, right)| {
                        super::canonical_place_segments_equal(program, *left, *right)
                    })
            {
                let stored = flow.control.operands.get(statement.stored_value);
                let stored_type = stored.type_reference;
                // Zero formation has no evaluated RHS. Its establishment
                // facts belong to the observed storage, not an invalid source
                // expression pretending to be a completed producer.
                if !program
                    .expression_table
                    .expression_is_valid(stored.expression)
                    || !stored_type.is_valid()
                    || !validation::project_type_reference_from_segments(
                        program,
                        stored_type,
                        &source.segments[destination.segments.len()..],
                    )
                    .is_some_and(|projected| {
                        super::operand_contexts::operand_binds_stable_copy(program, projected)
                    })
                {
                    return None;
                }
                return captured_operand_values(
                    program,
                    flow,
                    semantic,
                    state,
                    stored,
                    stored_type,
                    &source.segments[destination.segments.len()..],
                );
            }
        }
        if !super::entry_origin_is_stable(
            program,
            &flow.control,
            statement.completed_content_writes,
            statement.completed_content_unknown_write,
            &source,
        ) {
            return None;
        }
    }
    None
}

pub(crate) fn captured_operand_at_path<'capture>(
    program: &dyn ::validation::CheckingInput,
    control: &'capture FlowControlFacts,
    mut operand: &'capture FlowOperandFact,
    mut type_reference: TypeReferenceHandle,
    mut path: &[PlaceSegment],
) -> Option<(&'capture FlowOperandFact, TypeReferenceHandle)> {
    while !path.is_empty() {
        let mut selected = None;
        for child in control.operand_children.span(operand.children)? {
            let destination = control.operand_child_paths.span(child.path)?;
            if destination.is_empty() {
                return None;
            }
            if destination.len() <= path.len()
                && destination.iter().zip(path).all(|(left, right)| {
                    super::canonical_place_segments_equal(program, *left, *right)
                })
            {
                if selected.is_some() || !control.operands.is_valid(child.value) {
                    return None;
                }
                selected = Some((child, destination.len()));
            }
        }
        let (child, length) = selected?;
        operand = control.operands.get(child.value);
        type_reference = operand_child_type(
            program,
            type_reference,
            control.operand_child_paths.span(child.path)?,
            child.type_reference,
        )?;
        path = &path[length..];
    }
    type_reference
        .is_valid()
        .then_some((operand, type_reference))
}
