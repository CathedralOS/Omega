//! Authority of a completed descriptor follows its actual value producer.
//!
//! Stored descriptors retain their exact loan occurrences at the evaluated
//! cut. Normal calls continue the full declared lifetime frontier of their
//! completed actuals; selections retain every alternative. These are retained
//! access footprints, which may conservatively cover more than the pointer's
//! exact address. Missing or coarse address geometry supplies no authority and
//! cannot narrow that frontier, but does not erase the actual descriptor value.
//! Physical descriptor loads still use the typed, current ingress judgment.
//! Copied descriptors follow their earlier producer rather than rereading a
//! source slot that later operands may have replaced.

use arena::Handle;
use checked_trees::{
    CapturedPlace, CheckFacts, FlowOperandCompletion, FlowOperandFact, FlowStateFact,
};
use facts::PlaceSegment;
use typed_trees::expression::ExpressionNode;

use super::aliases::ResolvedAlias;

pub(super) struct CompletedDescriptorBackings {
    pub(super) places: Vec<ResolvedAlias>,
    /// Actual reference operands whose own referent is forwarded through the
    /// authenticated invocation/result relation. Stored child descriptors and
    /// unrelated input lifetimes are not this temporary invocation access.
    pub(super) forwarded_inputs: Vec<Handle<FlowOperandFact>>,
}

pub(super) fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: Handle<FlowOperandFact>,
) -> Result<CompletedDescriptorBackings, &'static str> {
    resolve_at_path(program, facts, state, capture, &[])
}

pub(super) fn resolve_at_path(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    capture: Handle<FlowOperandFact>,
    path: &[PlaceSegment],
) -> Result<CompletedDescriptorBackings, &'static str> {
    let descriptor = if path.is_empty() {
        descriptor_source(program, facts, capture)?
    } else if facts.flow.control.operands.is_valid(capture) {
        capture
    } else {
        return Err("the completed projection has no value occurrence");
    };
    let mut forwarded_inputs = Vec::new();
    let places = Resolver {
        program,
        facts,
        state,
    }
    .retained_descriptor(descriptor, path, &mut Vec::new(), &mut forwarded_inputs)?;
    Ok(CompletedDescriptorBackings {
        places,
        forwarded_inputs,
    })
}

/// Identify the exact descriptor of a completed value observation. Dispatch
/// and resolution use this same occurrence check; declared value shape alone
/// does not distinguish a carried reference from an ordinary storage read.
pub(super) fn descriptor_source(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    capture: Handle<FlowOperandFact>,
) -> Result<Handle<FlowOperandFact>, &'static str> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(capture) {
        return Err("the completed access has no value occurrence");
    }
    let operand = control.operands.get(capture);
    if validation::reference_shell(program, operand.type_reference).is_some() {
        Ok(capture)
    } else {
        value_observation_descriptor_source(program, facts, capture)
    }
}

/// A contextual value observation detached its referee bytes, while retaining
/// the exact descriptor that was read. A newly formed live view instead owns
/// its own geometry: its bound reference type or receiver backing cannot turn
/// it into that value-observation operation.
pub(super) fn value_observation_descriptor_source(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    capture: Handle<FlowOperandFact>,
) -> Result<Handle<FlowOperandFact>, &'static str> {
    let control = &facts.flow.control;
    if !control.operands.is_valid(capture) {
        return Err("the completed access has no value occurrence");
    }
    let operand = control.operands.get(capture);
    if crate::flow::operand_binds_live_view(program, operand.type_reference) {
        return Err("the completed access retains a live view, not a detached value observation");
    }
    if !control.operands.is_valid(operand.backing) {
        return Err("the completed observation has no retained descriptor source");
    }
    let source = control.operands.get(operand.backing);
    if source.expression != operand.expression
        || source.state_symbol != operand.state_symbol
        || source.statement_index != operand.statement_index
        || !validation::reference_shell(program, source.type_reference)
            .is_some_and(|reference| reference.access.is_readable())
    {
        return Err("the completed observation has no exact readable descriptor source");
    }
    Ok(operand.backing)
}

struct Resolver<'input> {
    program: &'input dyn validation::CheckingInput,
    facts: &'input CheckFacts,
    state: &'input FlowStateFact,
}

impl Resolver<'_> {
    /// Resolve the complete retained footprint through the actual value graph.
    /// Type, state, occurrence, source path and loan lineage remain mandatory;
    /// geometric precision never replaces them or invents a narrower grant.
    fn retained_descriptor(
        &self,
        capture: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<Vec<ResolvedAlias>, &'static str> {
        let control = &self.facts.flow.control;
        if !control.operands.is_valid(capture) || ancestors.contains(&capture) {
            return Err("the completed descriptor graph is missing or cyclic");
        }
        let operand = control.operands.get(capture);
        if operand.state_symbol != self.state.state_symbol {
            return Err("the completed descriptor belongs to another state");
        }
        let reference = validation::project_type_reference_from_segments(
            self.program,
            operand.type_reference,
            path,
        )
        .ok_or("the completed descriptor path has no declared type")?;
        if validation::reference_shell(self.program, reference).is_none() {
            return Err("the completed value path is not a reference descriptor");
        }
        ancestors.push(capture);
        let resolved = self.producer(operand, path, ancestors, forwarded_inputs);
        ancestors.pop();
        let resolved = resolved?;
        if resolved.is_empty() {
            return Err("the completed descriptor has no retained authority frontier");
        }
        Ok(resolved)
    }

    fn producer(
        &self,
        operand: &FlowOperandFact,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<Vec<ResolvedAlias>, &'static str> {
        let control = &self.facts.flow.control;
        // Copies and selections own completed value occurrences. Their source
        // expressions can still name storage that later operands replaced;
        // follow the retained producer graph before considering that address.
        match operand.completion {
            FlowOperandCompletion::Selected(alternatives) => {
                let alternatives = control
                    .operand_alternatives
                    .span(alternatives)
                    .ok_or("the completed selection has no alternative roster")?;
                if alternatives.is_empty() {
                    return Err("the completed selection has no incoming descriptor");
                }
                let mut resolved = Vec::new();
                for alternative in alternatives {
                    if !alternative.source_arm.is_valid() {
                        return Err("the completed selection has an unauthenticated arm");
                    }
                    append_distinct(
                        &mut resolved,
                        self.retained_descriptor(
                            alternative.value,
                            path,
                            ancestors,
                            forwarded_inputs,
                        )?,
                    );
                }
                return Ok(resolved);
            }
            FlowOperandCompletion::Copied { copy, path: prefix } => {
                if !control.copied_values.is_valid(copy) {
                    return Err("the completed copy has no source value");
                }
                let mut projection = control
                    .operand_child_paths
                    .span(prefix)
                    .ok_or("the completed copy has no source projection")?
                    .to_vec();
                projection.extend_from_slice(path);
                return self.retained_descriptor(
                    control.copied_values.get(copy).source,
                    &projection,
                    ancestors,
                    forwarded_inputs,
                );
            }
            _ => {}
        }
        let coordinate = validation::canonical_place_from_expression_in_state(
            self.program,
            self.state.state_symbol,
            operand.statement_index,
            operand.expression,
        );
        // An ordinary read of a stored descriptor retains that occurrence's
        // own loan, even when its value graph remembers an older construction.
        // Replacing this with only the construction's suppliers would lose the
        // descriptor's identity in the live-loan interference judgment.
        if !matches!(
            self.program.expression_table.expression(operand.expression),
            ExpressionNode::Borrow(_)
        ) && let Some(source) = &coordinate
            && source.root.as_symbol().is_some()
        {
            return self.stored(operand, source, path);
        }
        // A saved subject is the producer's retained value, not another read
        // of its authored initializer. Missing saved custody has no backing.
        if matches!(
            self.program.expression_table.expression(operand.expression),
            ExpressionNode::SavedSubject(_)
        ) {
            let source = control
                .operands
                .is_valid(operand.backing)
                .then(|| control.operands.get(operand.backing))
                .ok_or("the saved descriptor has no completed subject")?;
            if source.state_symbol != operand.state_symbol
                || source.statement_index != operand.statement_index
                || self.program.normalized_type_identity(source.type_reference)
                    != self
                        .program
                        .normalized_type_identity(operand.type_reference)
            {
                return Err("the saved descriptor disagrees with its completed subject");
            }
            return self.retained_descriptor(operand.backing, path, ancestors, forwarded_inputs);
        }
        // An owned completed receiver retains its fields as values. Rejoin the
        // exact evaluated backing and typed suffix; this is not a descriptor
        // load through a reference and cannot recover a named stored slot.
        if let Some(selected) = &coordinate
            && control.operands.is_valid(operand.backing)
        {
            let receiver = control.operands.get(operand.backing);
            if receiver.state_symbol != operand.state_symbol
                || receiver.statement_index != operand.statement_index
            {
                return Err("the descriptor projection has another receiver occurrence");
            }
            if !crate::flow::operand_binds_live_view(self.program, receiver.type_reference)
                && let Some(source) = validation::canonical_place_from_expression_in_state(
                    self.program,
                    self.state.state_symbol,
                    receiver.statement_index,
                    receiver.expression,
                )
                && source.root == selected.root
                && source.segments.len() < selected.segments.len()
                && same_path(
                    self.program,
                    &source.segments,
                    &selected.segments[..source.segments.len()],
                )
            {
                let mut projection = selected.segments[source.segments.len()..].to_vec();
                let reference = validation::project_type_reference_from_segments(
                    self.program,
                    receiver.type_reference,
                    &projection,
                )
                .ok_or("the descriptor projection has no typed receiver suffix")?;
                if self.program.normalized_type_identity(reference)
                    != self
                        .program
                        .normalized_type_identity(operand.type_reference)
                {
                    return Err("the descriptor projection disagrees with its completed receiver");
                }
                projection.extend_from_slice(path);
                return self.retained_descriptor(
                    operand.backing,
                    &projection,
                    ancestors,
                    forwarded_inputs,
                );
            }
        }
        if let FlowOperandCompletion::Constructed(_) = operand.completion
            && !path.is_empty()
        {
            let (child, _, consumed) = crate::flow::captured_operand_child_at_path(
                self.program,
                control,
                operand,
                operand.type_reference,
                path,
            )
            .ok_or("the constructed descriptor has no completed child")?;
            return self.retained_descriptor(child, &path[consumed..], ancestors, forwarded_inputs);
        }
        if let ExpressionNode::Call(call) =
            self.program.expression_table.expression(operand.expression)
        {
            let mut calls = control
                .calls
                .span(self.state.calls)
                .ok_or("the completed call has no invocation roster")?
                .iter()
                .filter(|row| {
                    row.statement_index == operand.statement_index
                        && row.authored_expression == operand.expression
                });
            let invocation = calls
                .next()
                .ok_or("the completed result has no actual invocation")?;
            if calls.next().is_some() || invocation.authored_target != call.target_symbol {
                return Err("the completed result has no unique checked invocation");
            }
            let sources =
                crate::borrow::call_reference_sources(self.program, self.state.state_symbol, call)
                    .ok_or("the completed call has no declared return-source relation")?;
            let mut resolved = Vec::new();
            for (source, projection) in sources {
                let output = crate::borrow::owner_place_segments(&source.owner_path);
                if !same_path(self.program, &output, path) {
                    continue;
                }
                let actuals = invocation.argument_operands();
                let index = u32::try_from(source.parameter_index)
                    .map_err(|_| "the returned descriptor's input coordinate is out of range")?;
                if index >= actuals.count() {
                    return Err("the returned descriptor has no completed input");
                }
                let input = crate::borrow::owner_place_segments(&source.source_path);
                for mut backing in self.input_descriptor(
                    actuals.handle_at(index),
                    source.source,
                    &input,
                    ancestors,
                    forwarded_inputs,
                )? {
                    backing.place.segments =
                        facts::project_captured_view_segments(&backing.place.segments, &projection)
                            .ok_or("the returned descriptor cannot project its retained source")?;
                    append_distinct(&mut resolved, [backing]);
                }
            }
            return (!resolved.is_empty())
                .then_some(resolved)
                .ok_or("the completed call does not cover the returned descriptor path");
        }
        let source =
            coordinate.ok_or("the completed descriptor has no source storage coordinate")?;
        let mut place = CapturedPlace {
            incoming_reference: Default::default(),
            root: source.root,
            segments: source.segments,
        };
        place.segments.extend_from_slice(path);
        if matches!(
            self.program.expression_table.expression(operand.expression),
            ExpressionNode::Borrow(_)
        ) {
            super::access_sources::resolve_captured_all(
                self.program,
                self.facts,
                self.state,
                operand.statement_index,
                operand.constraints,
                place,
            )
            .ok_or("the completed borrow has no retained source authority")
        } else {
            Err("the computed descriptor has no retained authority producer")
        }
    }

    fn stored(
        &self,
        operand: &FlowOperandFact,
        source: &validation::CanonicalPlace,
        path: &[PlaceSegment],
    ) -> Result<Vec<ResolvedAlias>, &'static str> {
        let mut place = CapturedPlace {
            incoming_reference: Default::default(),
            root: source.root,
            segments: source.segments.clone(),
        };
        place.segments.extend_from_slice(path);
        super::access_sources::resolve_descriptor_all(
            self.program,
            self.facts,
            self.state,
            operand.statement_index,
            operand.constraints,
            place,
        )
        .ok_or("the stored descriptor has no retained loan occurrence")
    }

    fn input_descriptor(
        &self,
        capture: Handle<FlowOperandFact>,
        source: validation::borrow_contracts::ViewReturnInputSource,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<Vec<ResolvedAlias>, &'static str> {
        let control = &self.facts.flow.control;
        if !control.operands.is_valid(capture) {
            return Err("the return-source relation has no completed actual");
        }
        let actual = control.operands.get(capture);
        // Loading a descriptor stored through an input pointer is distinct
        // from returning that pointer's own referent, including when both
        // declaration-relative paths are empty. Only the actual's captured
        // pointee inventory may supply the former; never reread its storage.
        if source == validation::borrow_contracts::ViewReturnInputSource::Stored
            && validation::reference_shell(self.program, actual.type_reference).is_some()
        {
            if !control.operand_retentions.is_valid(actual.retention) {
                return Err("the borrowed input has no captured stored-descriptor inventory");
            }
            let retained = control.operand_retentions.get(actual.retention);
            let mut projection = control
                .operand_retained_paths
                .span(retained.stored_projection)
                .ok_or("the borrowed input has no captured stored-descriptor projection")?
                .to_vec();
            projection.extend_from_slice(path);
            return self.retained_descriptor(
                retained.stored_value,
                &projection,
                ancestors,
                forwarded_inputs,
            );
        }
        let resolved = self.retained_descriptor(capture, path, ancestors, forwarded_inputs)?;
        if source == validation::borrow_contracts::ViewReturnInputSource::Referent
            && path.is_empty()
            && validation::reference_shell(self.program, actual.type_reference).is_some()
            && !forwarded_inputs.contains(&capture)
        {
            forwarded_inputs.push(capture);
        }
        Ok(resolved)
    }
}

fn same_path(
    program: &dyn validation::CheckingInput,
    left: &[PlaceSegment],
    right: &[PlaceSegment],
) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(left, right)| {
            crate::flow::canonical_place_segments_equal(program, *left, *right)
        })
}

fn append_distinct(
    target: &mut Vec<ResolvedAlias>,
    values: impl IntoIterator<Item = ResolvedAlias>,
) {
    for value in values {
        if !target
            .iter()
            .any(|existing| existing.place == value.place && existing.lineage == value.lineage)
        {
            target.push(value);
        }
    }
}
