//! Project an invocation's retained value tree onto the child that supplied it.
//! Destination paths identify fields independently of authored evaluation order.
//! Each selected child keeps its own completion context and type: siblings may
//! have copied the same source at different times, or may carry live loans.

mod captured_operand;
mod reference_places;
pub(crate) use captured_operand::CapturedOperand;
pub(crate) use reference_places::{
    captured_operand_reference_places_with_formation, captured_reference_places,
    captured_reference_places_with_formation,
};

use checked_trees::{FlowControlFacts, FlowFacts, FlowOperandFact, FlowStateFact};
use facts::{FactPlan, PlaceRoot, PlaceSegment};
use typed_trees::types::TypeReferenceHandle;
pub(crate) use validation::completed_storage::{
    captured_operand_child_at_path, captured_retention_profiles_at_path, finite_projection_paths,
    operand_child_type, project_captured_view_place, reference_captures_at_path,
};

/// Contextual construction uses the bound destination declaration. A source
/// declaration is retained only when that destination cannot be projected.
/// Neither path infers a type from a sibling's expression handle.
pub(crate) struct CapturedValue<'capture> {
    /// The actual storage observation and its live qualification namespace.
    /// A reaching value origin cannot erase evidence attached to this copy.
    pub observation: CapturedOperand<'capture>,
    pub observation_type: TypeReferenceHandle,
    pub observation_projection: super::LiteralExpressionProjection,
    pub operand: CapturedOperand<'capture>,
    pub type_reference: TypeReferenceHandle,
    pub projection: super::LiteralExpressionProjection,
    /// The exact read that detached contents from a live producer view.
    /// Later copies retain this epoch, independently of their own observation.
    pub producer_epoch: Option<CapturedOperand<'capture>>,
    /// Correlated selections traversed to obtain this observation. Every
    /// candidate remains an obligation; these coordinates never license a
    /// consumer to merge facts from mutually exclusive arms.
    pub choices: Vec<CapturedValueChoice>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CapturedValueChoice {
    /// The producer allocates a fresh alternative span per evaluation. Copies
    /// of that completed selection retain the same span, unlike syntax keys.
    pub selection: arena::HandleSpan<checked_trees::FlowOperandAlternative>,
    pub alternative: arena::Handle<checked_trees::FlowOperandAlternative>,
}

impl CapturedValue<'_> {
    /// Reaching view evidence belongs to the read that detached its contents.
    /// Observation evidence still uses the consumer's own captured namespace.
    pub(crate) fn producer_invocation(
        &self,
        invocation: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    ) -> arena::HandleSpan<checked_trees::FlowConstraintRef> {
        self.producer_epoch
            .map_or(invocation, |epoch| epoch.constraints)
    }

    pub(crate) fn producer_frontier(
        &self,
        state: symbols::SymbolHandle,
        statement: usize,
    ) -> (symbols::SymbolHandle, usize) {
        self.producer_epoch.map_or((state, statement), |epoch| {
            (epoch.state_symbol, epoch.statement_index)
        })
    }
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

/// A receiver's read needs its captured storage as well as the backing of any
/// reference leaves it carries. Projecting a reference-free child does not
/// detach that storage before the read. Completed-result lifetime queries stay
/// separate so later scalar copies do not prolong their old backing.
pub(crate) fn captured_observation_roots(
    program: &dyn validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
) -> Option<Vec<PlaceRoot>> {
    let mut roots = captured_lifetime_roots(program, control, operand)?;
    for referent in control.operand_referents.span(operand.referents)? {
        if !referent.root.is_valid() {
            return None;
        }
        if !roots.contains(&referent.root) {
            roots.push(referent.root);
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
        &flow.control,
        value.operand.row(),
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
        &flow.control,
        value.observation.row(),
        value.observation_type,
        &value.observation_projection,
    )
}

/// The completed-value projection is also available while the current state's
/// Flow row is being built. It consumes the same captured graph, not source
/// storage looked up again after later operands have run.
pub(super) fn captured_places(
    program: &dyn ::validation::CheckingInput,
    control: &FlowControlFacts,
    operand: &FlowOperandFact,
    type_reference: TypeReferenceHandle,
    projection: &super::LiteralExpressionProjection,
) -> Option<Vec<CapturedValuePlace>> {
    if let checked_trees::FlowOperandCompletion::Copied { copy, path } = operand.completion {
        let copied = control.copied_values.get(copy);
        let source = control.operands.get(copied.source);
        let mut selected = control.operand_child_paths.span_or_empty(path).to_vec();
        selected.extend_from_slice(&projection.remaining);
        if let Some(leaf) = control
            .captured_reference_leaves
            .span_or_empty(copied.references)
            .iter()
            .find(|leaf| selected.starts_with(control.operand_child_paths.span_or_empty(leaf.path)))
        {
            let referents = control.operand_referents.span_or_empty(leaf.referents);
            if referents.is_empty() {
                return None;
            }
            let prefix_length = leaf.path.count() as usize;
            return referents
                .iter()
                .map(|referent| {
                    let storage = validation::CanonicalPlace {
                        root: referent.root,
                        segments: control
                            .operand_referent_segments
                            .span_or_empty(referent.segments)
                            .to_vec(),
                    };
                    let place =
                        project_captured_view_place(storage.clone(), &selected[prefix_length..])?;
                    Some(CapturedValuePlace {
                        storage,
                        projected: place,
                    })
                })
                .collect();
        }
        if super::operand_contexts::operand_binds_live_view(program, type_reference) {
            return None;
        }
        let storage = validation::CanonicalPlace {
            root: PlaceRoot::Expression(source.expression),
            segments: control.operand_child_paths.span_or_empty(path).to_vec(),
        };
        let place = validation::CanonicalPlace {
            root: storage.root,
            segments: selected,
        };
        return Some(vec![CapturedValuePlace {
            storage,
            projected: place,
        }]);
    }
    let referents = control.operand_referents.span_or_empty(operand.referents);
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
                segments: control
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
    operand: CapturedOperand<'capture>,
    type_reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<Vec<CapturedValue<'capture>>> {
    let control = &flow.control;
    if !operand.belongs_to(control) {
        return None;
    }
    if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } = operand.completion
    {
        let copy = control.copied_values.get(copy);
        let mut selected = control.operand_child_paths.span_or_empty(prefix).to_vec();
        selected.extend_from_slice(path);
        if control
            .captured_reference_leaves
            .span_or_empty(copy.references)
            .iter()
            .any(|leaf| selected.starts_with(control.operand_child_paths.span_or_empty(leaf.path)))
        {
            return Some(vec![CapturedValue {
                observation: operand,
                observation_type: type_reference,
                observation_projection: super::LiteralExpressionProjection {
                    expression: operand.expression,
                    remaining: path.to_vec(),
                    destination: Vec::new(),
                },
                operand,
                type_reference,
                projection: super::LiteralExpressionProjection {
                    expression: operand.expression,
                    remaining: path.to_vec(),
                    destination: Vec::new(),
                },
                choices: Vec::new(),
                producer_epoch: None,
            }]);
        }
        let source = CapturedOperand::recorded(control, copy.source)?;
        let mut values = captured_operand_values(
            program,
            flow,
            semantic,
            state,
            source,
            source.type_reference,
            &selected,
        )?;
        for value in &mut values {
            value.observation = operand;
            value.observation_type = type_reference;
            value.observation_projection = super::LiteralExpressionProjection {
                expression: operand.expression,
                remaining: path.to_vec(),
                destination: Vec::new(),
            };
        }
        return Some(values);
    }
    if let checked_trees::FlowOperandCompletion::Selected(alternatives) = operand.completion {
        let alternatives_rows = control.operand_alternatives.span(alternatives)?;
        if alternatives_rows.is_empty() {
            return None;
        }
        let mut values = Vec::new();
        for (index, alternative) in alternatives_rows.iter().enumerate() {
            if !alternative.source_arm.is_valid() || !control.operands.is_valid(alternative.value) {
                return None;
            }
            let completed = CapturedOperand::recorded(control, alternative.value)?;
            let mut selected = captured_operand_values(
                program,
                flow,
                semantic,
                state,
                completed,
                type_reference,
                path,
            )?;
            if selected.is_empty() {
                return None;
            }
            let choice = CapturedValueChoice {
                selection: alternatives,
                alternative: alternatives.handle_at(u32::try_from(index).ok()?),
            };
            for value in &mut selected {
                value.choices.push(choice);
            }
            values.extend(selected);
        }
        return Some(values);
    }
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
        let (captured, selected_type, _) = projected_capture(
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
        if matches!(
            captured.completion,
            checked_trees::FlowOperandCompletion::Selected(_)
        ) {
            // Constructor projection stops at a selected child. Continue on
            // its retained evaluations, not a fresh walk over arm syntax.
            let mut selected = captured_operand_values(
                program,
                flow,
                semantic,
                state,
                captured,
                selected_type,
                &projection.remaining,
            )?;
            for value in &mut selected {
                let mut destination = projection.destination.clone();
                destination.append(&mut value.projection.destination);
                value.projection.destination = destination;
            }
            values.extend(selected);
            continue;
        }
        let value = CapturedValue {
            observation: captured,
            observation_type: selected_type,
            observation_projection: projection.clone(),
            operand: captured,
            type_reference: selected_type,
            projection,
            choices: Vec::new(),
            producer_epoch: None,
        };
        // A primitive receiving read detached contents from its actual
        // descriptor. Keep that complete source inventory, not only literal
        // snapshots: call guarantees and qualifications belong to this read's
        // post-receiver epoch even after later writes change the referent.
        if let Some(source) = primitive_read_source(program, control, captured, selected_type) {
            let mut origins = captured_operand_values(
                program,
                flow,
                semantic,
                state,
                source,
                source.type_reference,
                &[],
            )?;
            for origin in &mut origins {
                origin.producer_epoch = Some(captured);
                origin.observation = value.observation;
                origin.observation_type = value.observation_type;
                origin.observation_projection = value.observation_projection.clone();
            }
            values.extend(origins);
            continue;
        }
        if let Some(mut stored) =
            reaching_store_values(program, flow, semantic, state, &value, None)
        {
            let observed_type = validation::project_type_reference_from_segments(
                program,
                value.type_reference,
                &value.projection.remaining,
            )?;
            let detached =
                !super::operand_contexts::operand_binds_live_view(program, observed_type);
            for origin in &mut stored {
                // Crossing from a live descriptor into a detached value freezes
                // the referent read here. Outer copies must not move that read
                // to a later consumer's invocation.
                if detached
                    && origin.producer_epoch.is_none()
                    && super::operand_contexts::operand_binds_live_view(
                        program,
                        origin.type_reference,
                    )
                {
                    origin.producer_epoch = Some(value.observation);
                }
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

/// Only the typed primitive read crosses this boundary. A field's container
/// backing is not its value, and a representation view needs its own scalar
/// interpretation rather than equal-width pointer geometry.
fn primitive_read_source<'capture>(
    program: &dyn validation::CheckingInput,
    control: &'capture FlowControlFacts,
    captured: CapturedOperand<'capture>,
    selected_type: TypeReferenceHandle,
) -> Option<CapturedOperand<'capture>> {
    let primitive = program.primitive_type_reference(selected_type)?;
    if program.primitive_type_reference(captured.type_reference) != Some(primitive) {
        return None;
    }
    let source = CapturedOperand::recorded(control, captured.backing)?;
    let shell = validation::reference_shell(program, source.type_reference)?;
    (shell.access.is_readable()
        && program.primitive_type_reference(shell.referee) == Some(primitive))
    .then_some(source)
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
        if value.producer_epoch.is_none()
            && super::operand_contexts::operand_binds_live_view(program, value.type_reference)
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
    let operand = observation.unwrap_or(&value.operand);
    let observed_type = validation::project_type_reference_from_segments(
        program,
        value.type_reference,
        &value.projection.remaining,
    )?;
    // An owned value transports its descriptor words, not the pointee's
    // contents. Selecting a reference-valued field therefore follows Binding
    // at this capture's cut, even though that reference is not stable contents.
    // Keep the selected value typed as a live view: subsequent reads through
    // it still observe invocation storage and cannot freeze pointee predicates.
    // Stable scalar fields likewise follow their exact reaching contents;
    // the containing owner's unrelated custody does not decide their identity.
    let descriptor = super::operand_contexts::operand_binds_live_view(program, observed_type);
    if (observation.is_none()
        && super::operand_contexts::operand_binds_live_view(program, value.type_reference))
        || operand.state_symbol != state.state_symbol
        || (!descriptor
            && !super::operand_contexts::operand_binds_stable_copy(program, observed_type))
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
        if descriptor {
            super::stored_values::StoredObservation::Binding
        } else {
            super::stored_values::StoredObservation::Contents
        },
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
                let selected_descriptor =
                    super::operand_contexts::operand_binds_live_view(program, projected);
                if descriptor {
                    selected_descriptor
                } else {
                    super::operand_contexts::operand_binds_stable_copy(program, projected)
                        // An admitted value read can dereference a resident view.
                        // Retain its actual producer and freeze the referent at
                        // this read; provenance supplies no permission to copy.
                        || (selected_descriptor
                            && program.normalized_type_identity(
                                program.type_reference_table.peeled_type_reference(projected),
                            ) == program.normalized_type_identity(
                                program.type_reference_table.peeled_type_reference(observed_type),
                            ))
                }
            })
        {
            return None;
        }
        values.extend(captured_operand_values(
            program,
            flow,
            semantic,
            state,
            CapturedOperand::recorded(&flow.control, selected.capture)?,
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
    let (operand, reference, _) = projected_capture(
        program,
        control,
        CapturedOperand::unrecorded(operand),
        type_reference,
        path,
    )?;
    Some((operand.row(), reference))
}

pub(crate) fn captured_operand_handle_at_path(
    program: &dyn ::validation::CheckingInput,
    control: &FlowControlFacts,
    capture: arena::Handle<FlowOperandFact>,
    reference: TypeReferenceHandle,
    path: &[PlaceSegment],
) -> Option<arena::Handle<FlowOperandFact>> {
    let operand = CapturedOperand::recorded(control, capture)?;
    let (_, _, selected) = projected_capture(program, control, operand, reference, path)?;
    Some(if path.is_empty() { capture } else { selected })
}

fn projected_capture<'capture>(
    program: &dyn ::validation::CheckingInput,
    control: &'capture FlowControlFacts,
    mut operand: CapturedOperand<'capture>,
    mut type_reference: TypeReferenceHandle,
    mut path: &[PlaceSegment],
) -> Option<(
    CapturedOperand<'capture>,
    TypeReferenceHandle,
    arena::Handle<FlowOperandFact>,
)> {
    if !operand.belongs_to(control) {
        return None;
    }
    if let checked_trees::FlowOperandCompletion::Copied { copy, path: prefix } = operand.completion
    {
        let copy = control.copied_values.get(copy);
        let mut projection = control.operand_child_paths.span_or_empty(prefix).to_vec();
        projection.extend_from_slice(path);
        let source = CapturedOperand::recorded(control, copy.source)?;
        return projected_capture(program, control, source, source.type_reference, &projection);
    }
    let mut selected_handle = arena::Handle::invalid();
    while !path.is_empty() {
        let (child, reference, length) =
            captured_operand_child_at_path(program, control, &operand, type_reference, path)?;
        selected_handle = child;
        operand = CapturedOperand::recorded(control, child)?;
        type_reference = reference;
        path = &path[length..];
    }
    type_reference
        .is_valid()
        .then_some((operand, type_reference, selected_handle))
}
