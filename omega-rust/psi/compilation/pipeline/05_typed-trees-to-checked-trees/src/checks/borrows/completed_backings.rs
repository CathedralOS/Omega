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

use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use crate::published::places::place_at;
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

/// A selected incoming alternative keeps its original entry descriptor even
/// when a conditional store retires that identity from the current roster.
/// Callers have selected this exact Incoming member on the immutable storage
/// timeline. Resolve its entry namespace without reactivating current access;
/// the reference used to reach the slot cannot supply that permission.
pub(in crate::checks::borrows) fn incoming_sources(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    statement_index: usize,
    constraints: arena::HandleSpan<checked_trees::FlowConstraintRef>,
    source: validation::CanonicalPlace,
) -> Result<Vec<ResolvedAlias>, &'static str> {
    if let Some(places) = super::aliases::unchanged_incoming_leaf(
        program,
        facts,
        state,
        state.entry_constraints,
        &source,
    )? {
        return Ok(places);
    }
    super::access_sources::resolve_descriptor_all(
        program,
        facts,
        loan_occurrences,
        state,
        statement_index,
        constraints,
        CapturedPlace {
            root: source.root,
            segments: source.segments,
            ..Default::default()
        },
    )
    .ok_or("the incoming descriptor has no retained loan occurrence")
}

pub(super) fn resolve(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
    state: &FlowStateFact,
    capture: Handle<FlowOperandFact>,
) -> Result<CompletedDescriptorBackings, &'static str> {
    resolve_at_path(program, facts, loan_occurrences, state, capture, &[])
}

pub(super) fn resolve_at_path(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    loan_occurrences: &LoanOccurrences<'_>,
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
    let resolved = Resolver {
        program,
        facts,
        loan_occurrences,
        state,
    }
    .retained_descriptor(descriptor, path, &mut Vec::new(), &mut forwarded_inputs)?;
    Ok(CompletedDescriptorBackings {
        places: resolved.places,
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

/// Complete authority alternatives. Image storage has intrinsic shared
/// formation, with no fabricated caller address or borrowed-loan lineage.
#[derive(Default)]
struct BackingFrontier {
    places: Vec<ResolvedAlias>,
    immutable_image: bool,
}

impl BackingFrontier {
    fn borrowed(places: Vec<ResolvedAlias>) -> Self {
        Self {
            places,
            immutable_image: false,
        }
    }

    fn append(&mut self, other: Self) {
        append_distinct(&mut self.places, other.places);
        self.immutable_image |= other.immutable_image;
    }
}

struct Resolver<'input> {
    program: &'input dyn validation::CheckingInput,
    facts: &'input CheckFacts,
    loan_occurrences: &'input LoanOccurrences<'input>,
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
    ) -> Result<BackingFrontier, &'static str> {
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
        let resolved = self.producer(capture, path, ancestors, forwarded_inputs);
        ancestors.pop();
        let resolved = resolved?;
        if resolved.places.is_empty() && !resolved.immutable_image {
            return Err("the completed descriptor has no retained authority frontier");
        }
        if resolved.immutable_image
            && validation::reference_shell(self.program, reference).is_none_or(|reference| {
                reference.access != language_semantics::ReferenceAccess::Shared
            })
        {
            return Err("immutable image storage cannot grant writable access");
        }
        Ok(resolved)
    }

    fn producer(
        &self,
        capture: Handle<FlowOperandFact>,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<BackingFrontier, &'static str> {
        let control = &self.facts.flow.control;
        let operand = control.operands.get(capture);
        // Copies and selections own completed value occurrences. Their source
        // expressions can still name storage that later operands replaced;
        // follow the retained producer graph before considering that address.
        match operand.completion {
            FlowOperandCompletion::IncomingContents { .. } => {
                let frontier = validation::completed_storage::reference_sources_at_path(
                    self.program,
                    control,
                    capture,
                    path,
                    operand.storage_snapshot,
                );
                return self.stored_frontier(frontier, operand, ancestors, forwarded_inputs);
            }
            FlowOperandCompletion::Selected(alternatives) => {
                let alternatives = control
                    .operand_alternatives
                    .span(alternatives)
                    .ok_or("the completed selection has no alternative roster")?;
                if alternatives.is_empty() {
                    return Err("the completed selection has no incoming descriptor");
                }
                let mut resolved = BackingFrontier::default();
                for alternative in alternatives {
                    if !alternative.source_arm.is_valid() {
                        return Err("the completed selection has an unauthenticated arm");
                    }
                    resolved.append(self.retained_descriptor(
                        alternative.value,
                        path,
                        ancestors,
                        forwarded_inputs,
                    )?);
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
        if path.is_empty()
            && matches!(
                operand.descriptor_permission,
                checked_trees::FlowDescriptorPermission::SuppliedOutput { .. }
            )
        {
            let handle = crate::borrow::supplied_outputs::captured_permission(
                self.program,
                &self.facts.borrow,
                &self.facts.flow,
                operand,
            )?;
            let loan = self.facts.borrow.loans.get(handle);
            return Ok(BackingFrontier::borrowed(vec![ResolvedAlias {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    incoming_owner_path: self
                        .facts
                        .borrow
                        .access_segments
                        .span(loan.incoming_owner_path)
                        .ok_or("the produced output lost its incoming path")?
                        .to_vec(),
                    root: loan.root,
                    segments: self.facts.borrow.loan_segments(loan).to_vec(),
                },
                lineage: vec![handle],
            }]));
        }
        if path.is_empty() && operand.descriptor_permission.loaded_loan().is_valid() {
            let handle = operand.descriptor_permission.loaded_loan();
            let (_, owner) = self
                .facts
                .borrow
                .first_state_borrow(self.state.machine_symbol, self.state.state_symbol)
                .ok_or("the loaded descriptor has no borrow owner")?;
            if !self.facts.borrow.state_owns_loan(owner, handle) {
                return Err("the loaded descriptor carries a foreign permission");
            }
            let loan = self.facts.borrow.loans.get(handle);
            if !self
                .facts
                .borrow
                .incoming_references
                .is_valid(loan.incoming_reference)
            {
                return Err("the loaded descriptor has no supplied pointer identity");
            }
            let incoming = self
                .facts
                .borrow
                .incoming_references
                .get(loan.incoming_reference);
            if !matches!(incoming.origin,
                checked_trees::IncomingReferenceOrigin::DescriptorLoad { expression }
                    if expression == operand.expression)
                || incoming.state_symbol != operand.state_symbol
                || loan.statement_index != operand.statement_index
                || !self
                    .facts
                    .flow
                    .borrow_loan_constraints(operand.constraints)
                    .any(|active| active == handle)
                || !control.case_accesses.is_valid(operand.case_access)
            {
                return Err("the loaded descriptor has no exact completed permission");
            }
            let access = control.case_accesses.get(operand.case_access);
            if access.expression != operand.expression
                || access.state_symbol != operand.state_symbol
                || access.statement_index != operand.statement_index
                || self
                    .facts
                    .flow
                    .borrow_loan_constraints(access.constraints)
                    .any(|active| active == handle)
            {
                return Err("the loaded descriptor cannot authorize its own formation");
            }
            let mut activations = self
                .facts
                .flow
                .borrow_lifetimes
                .activations
                .span_or_empty(self.state.borrow_activations)
                .iter()
                .filter(|activation| activation.loan == handle);
            let activation = activations
                .next()
                .ok_or("the loaded descriptor has no completed activation")?;
            if activations.next().is_some()
                || !control.operands.is_valid(activation.formation_value)
                || !control
                    .value_observations
                    .is_valid(operand.descriptor_permission.loaded_read())
            {
                return Err("the loaded descriptor has an ambiguous activation or source read");
            }
            let formed = control.operands.get(activation.formation_value);
            let read = control
                .value_observations
                .get(operand.descriptor_permission.loaded_read());
            if formed.descriptor_permission.loaded_loan() != handle
                || formed.descriptor_permission.loaded_read()
                    != operand.descriptor_permission.loaded_read()
                || formed.expression != operand.expression
                || formed.state_symbol != operand.state_symbol
                || formed.statement_index != operand.statement_index
                || formed.backing != operand.backing
                || formed.case_access != operand.case_access
                || formed.retention != operand.retention
                || formed.referents != operand.referents
                || read.expression != operand.expression
                || read.evaluation != access.evaluation
                || read.receiver != operand.backing
                || read.constraints != access.constraints
                || read.places.is_empty()
                || self
                    .facts
                    .flow
                    .borrow_loan_constraints(read.constraints)
                    .any(|active| active == handle)
                || !validation::declared_carriers_match(
                    self.program,
                    incoming.carrier_type,
                    formed.type_reference,
                )
                || crate::borrow::reference_borrow_access_kind(self.program, formed.type_reference)
                    .as_ref()
                    != Some(&loan.kind)
                || control.retained_roots(formed.retention).is_none()
            {
                return Err(
                    "the loaded descriptor disagrees with its completed read, carrier or lifetime",
                );
            }
            return Ok(BackingFrontier::borrowed(vec![ResolvedAlias {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    root: loan.root,
                    segments: self.facts.borrow.loan_segments(loan).to_vec(),
                    ..Default::default()
                },
                lineage: vec![handle],
            }]));
        }
        let coordinate = place_at(
            self.program,
            self.state.state_symbol,
            operand.statement_index,
            operand.expression,
        );
        // Literal formation authenticates image scope in the completed value
        // inventory. Consume that positive evidence for any producer instead
        // of inferring image backing from missing suppliers or source syntax.
        // A stored image descriptor has no activation-relative loan to find.
        if let Some(profiles) = validation::completed_storage::captured_retention_profiles_at_path(
            self.program,
            control,
            operand,
            path,
        ) && !profiles.is_empty()
            && profiles
                .iter()
                .all(|profile| self.immutable_image_profile(*profile, &mut Vec::new()))
        {
            return Ok(BackingFrontier {
                places: Vec::new(),
                immutable_image: true,
            });
        }
        // Other stored descriptors retain that occurrence's own loan, even
        // when their value graph remembers an older construction. Replacing
        // this with only the construction's suppliers would lose descriptor
        // identity in the live-loan interference judgment.
        if !matches!(
            self.program.expression_table.expression(operand.expression),
            ExpressionNode::Borrow(_)
        ) && let Some(source) = &coordinate
            && source.root.as_symbol().is_some()
        {
            return self.stored(capture, operand, source, path, ancestors, forwarded_inputs);
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
                && let Some(source) = place_at(
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
            let children = validation::completed_storage::captured_operand_children_at_path(
                self.program,
                control,
                capture,
                path,
            )
            .ok_or("the constructed descriptor has no complete evaluated child frontier")?;
            let mut resolved = BackingFrontier::default();
            for child in children {
                resolved.append(self.retained_descriptor(
                    child.capture,
                    &child.projection,
                    ancestors,
                    forwarded_inputs,
                )?);
            }
            return Ok(resolved);
        }
        if let FlowOperandCompletion::InvocationResult {
            invocation,
            result_type,
            path: output,
        } = operand.completion
        {
            if !path.is_empty() {
                return Err("a returned descriptor observation does not capture pointee contents");
            }
            let output = control
                .operand_retained_paths
                .span(output)
                .ok_or("the returned descriptor has no result coordinate")?;
            let selected =
                validation::project_type_reference_from_segments(self.program, result_type, output)
                    .ok_or("the returned descriptor coordinate has no declared type")?;
            if self.program.normalized_type_identity(selected)
                != self
                    .program
                    .normalized_type_identity(operand.type_reference)
            {
                return Err("the returned descriptor disagrees with its completed result leaf");
            }
            return self.invocation_result(
                operand,
                invocation,
                result_type,
                output,
                ancestors,
                forwarded_inputs,
            );
        }
        if matches!(
            self.program.expression_table.expression(operand.expression),
            ExpressionNode::Call(_)
        ) {
            let mut calls = self.state.calls.handles().filter(|handle| {
                let row = control.calls.get(*handle);
                row.statement_index == operand.statement_index
                    && row.authored_expression == operand.expression
            });
            let invocation = calls
                .next()
                .ok_or("the completed result has no actual invocation")?;
            if calls.next().is_some() {
                return Err("the completed result has no unique checked invocation");
            }
            return self.invocation_result(
                operand,
                invocation,
                operand.type_reference,
                path,
                ancestors,
                forwarded_inputs,
            );
        }
        if matches!(
            self.program.expression_table.expression(operand.expression),
            ExpressionNode::Borrow(_)
        ) {
            let formation =
                super::formation_captures::reference_formation(self.program, self.facts, capture)?
                    .ok_or("the completed borrow has no exact formation receiver")?;
            if formation.storage && path.is_empty() {
                return super::access_sources::stored_address(
                    self.program,
                    self.facts,
                    self.loan_occurrences,
                    self.state,
                    formation.target,
                    true,
                )
                .map(BackingFrontier::borrowed);
            }
            let source =
                coordinate.ok_or("the completed descriptor has no source storage coordinate")?;
            let mut place = CapturedPlace {
                root: source.root,
                segments: source.segments,
                ..Default::default()
            };
            place.segments.extend_from_slice(path);
            super::access_sources::resolve_captured_all(
                self.program,
                self.facts.view(),
                self.loan_occurrences,
                self.state,
                operand.statement_index,
                operand.constraints,
                place,
            )
            .map(BackingFrontier::borrowed)
            .ok_or("the completed borrow has no retained source authority")
        } else {
            Err("the computed descriptor has no retained authority producer")
        }
    }

    /// Both an evaluated call and its retained result-leaf observations name
    /// the same performed invocation. The normal return contract selects the
    /// actual captured inputs; lifetime debt and exclusion regions alone never
    /// become a descriptor supplier.
    fn invocation_result(
        &self,
        operand: &FlowOperandFact,
        invocation: Handle<checked_trees::FlowCallFact>,
        result_type: typed_trees::types::TypeReferenceHandle,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<BackingFrontier, &'static str> {
        let control = &self.facts.flow.control;
        if !control.calls.is_valid(invocation)
            || !self
                .state
                .calls
                .handles()
                .any(|candidate| candidate == invocation)
        {
            return Err("the completed result invocation belongs to another state");
        }
        let invocation = control.calls.get(invocation);
        let ExpressionNode::Call(call) = self
            .program
            .expression_table
            .expression(invocation.authored_expression)
        else {
            return Err("the completed result has no authored call occurrence");
        };
        if invocation.statement_index != operand.statement_index
            || invocation.authored_target != call.target_symbol
        {
            return Err("the completed result disagrees with its checked invocation");
        }
        if let Some(permission) = super::resources::invocation_permissions::resolve(
            self.program,
            self.facts,
            self.state,
            invocation,
            operand,
            result_type,
            path,
        ) {
            let handle = permission?;
            let loan = self.facts.borrow.loans.get(handle);
            return Ok(BackingFrontier::borrowed(vec![ResolvedAlias {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    incoming_owner_path: self
                        .facts
                        .borrow
                        .access_segments
                        .span_or_empty(loan.incoming_owner_path)
                        .to_vec(),
                    root: loan.root,
                    segments: self.facts.borrow.loan_segments(loan).to_vec(),
                },
                lineage: vec![handle],
            }]));
        }
        let sources = crate::borrow::completed_call_reference_sources(
            self.program,
            self.state.state_symbol,
            call,
        )
        .ok_or("the completed call has no declared return-source relation")?;
        let mut resolved = BackingFrontier::default();
        for source in sources {
            let (source, projection) = match source {
                crate::borrow::CompletedReturnSource::ImmutableImage { owner_path, kind } => {
                    if kind != checked_trees::BorrowAccessKind::Read {
                        return Err("immutable image storage cannot grant writable access");
                    }
                    let structural =
                        crate::borrow::structural_owner_path(self.program, &owner_path)
                            .ok_or("the image result has no declared output path")?;
                    let output = validation::reference_result_custody::loan_place_segments(
                        self.program,
                        result_type,
                        &structural,
                    )
                    .ok_or("the image result path disagrees with its completed carrier")?;
                    if same_path(self.program, &output, path) {
                        resolved.immutable_image = true;
                    }
                    continue;
                }
                crate::borrow::CompletedReturnSource::DescriptorCandidate { field, projection } => {
                    (field, projection)
                }
                crate::borrow::CompletedReturnSource::LifetimeDependency { .. } => continue,
                crate::borrow::CompletedReturnSource::UnresolvedDescriptor { .. } => {
                    return Err("the completed call has an unresolved descriptor alternative");
                }
            };
            let structural = crate::borrow::structural_owner_path(self.program, &source.owner_path)
                .ok_or("the returned descriptor has no declared output path")?;
            let output = validation::reference_result_custody::loan_place_segments(
                self.program,
                result_type,
                &structural,
            )
            .ok_or("the returned descriptor path disagrees with its completed carrier")?;
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
            let mut backings = self.input_descriptor(
                actuals.handle_at(index),
                source.source,
                &input,
                ancestors,
                forwarded_inputs,
            )?;
            for backing in &mut backings.places {
                backing.place.segments =
                    facts::project_captured_view_segments(&backing.place.segments, &projection)
                        .ok_or("the returned descriptor cannot project its retained source")?;
            }
            resolved.append(backings);
        }
        (!resolved.places.is_empty() || resolved.immutable_image)
            .then_some(resolved)
            .ok_or("the completed call does not cover the returned descriptor path")
    }

    fn immutable_image_profile(
        &self,
        profile: Handle<checked_trees::FlowOperandRetention>,
        ancestors: &mut Vec<Handle<checked_trees::FlowOperandRetention>>,
    ) -> bool {
        let control = &self.facts.flow.control;
        if !control.operand_retentions.is_valid(profile) || ancestors.contains(&profile) {
            return false;
        }
        let retained = control.operand_retentions.get(profile);
        ancestors.push(profile);
        // Call-result lifetime profiles keep the actual descriptor separately
        // from their declared input dependencies. Follow that positive body
        // correspondence; neither empty roots nor missing geometry imply image
        // storage, and every alternative must establish the same property.
        let image = if !retained.alternatives.is_empty() {
            control
                .operand_retentions
                .span(retained.alternatives)
                .is_some_and(|alternatives| {
                    !alternatives.is_empty()
                        && retained
                            .alternatives
                            .handles()
                            .all(|alternative| self.immutable_image_profile(alternative, ancestors))
                })
        } else if control.operands.is_valid(retained.descriptor) {
            self.immutable_image_profile(
                control.operands.get(retained.descriptor).retention,
                ancestors,
            )
        } else {
            retained.immutable_image
        };
        ancestors.pop();
        image
    }

    fn stored(
        &self,
        capture: Handle<FlowOperandFact>,
        operand: &FlowOperandFact,
        source: &validation::CanonicalPlace,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<BackingFrontier, &'static str> {
        let mut place = CapturedPlace {
            root: source.root,
            segments: source.segments.clone(),
            ..Default::default()
        };
        place.segments.extend_from_slice(path);
        // Current checked descriptor occurrences own custody even when a weak
        // call store cannot reconstruct one value producer. Authenticate that
        // complete roster first. Abstract incoming families still cannot pass
        // this raw lookup and must join the completed storage cut below.
        if let Some(places) = super::access_sources::resolve_descriptor_all(
            self.program,
            self.facts,
            self.loan_occurrences,
            self.state,
            operand.statement_index,
            operand.constraints,
            place,
        ) {
            return Ok(BackingFrontier::borrowed(places));
        }
        let control = &self.facts.flow.control;
        // An indexed incoming family describes permissions for unchanged
        // entry leaves. The completed receiver owns which leaf value survives
        // at this cut: a replacement must follow its produced value instead
        // of resurrecting the entry family from its declared shape.
        if control.operands.is_valid(operand.backing) && operand.backing != capture {
            let receiver = control.operands.get(operand.backing);
            if receiver.state_symbol == operand.state_symbol
                && let Some(receiver_place) = place_at(
                    self.program,
                    self.state.state_symbol,
                    receiver.statement_index,
                    receiver.expression,
                )
                && receiver_place.root == source.root
                && receiver_place.segments.len() < source.segments.len()
                && same_path(
                    self.program,
                    &receiver_place.segments,
                    &source.segments[..receiver_place.segments.len()],
                )
            {
                let mut projection = source.segments[receiver_place.segments.len()..].to_vec();
                projection.extend_from_slice(path);
                let frontier = validation::completed_storage::reference_sources_at_path(
                    self.program,
                    control,
                    operand.backing,
                    &projection,
                    operand.storage_snapshot,
                );
                return self.stored_frontier(frontier, operand, ancestors, forwarded_inputs);
            }
        }
        Err("the stored descriptor has no retained loan occurrence")
    }

    fn stored_frontier(
        &self,
        source: validation::completed_storage::StoredOperand,
        operand: &FlowOperandFact,
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<BackingFrontier, &'static str> {
        use validation::completed_storage::StoredOperand;
        match source {
            StoredOperand::Unavailable => Err("the stored descriptor has no current value source"),
            StoredOperand::Incoming(source) => incoming_sources(
                self.program,
                self.facts,
                self.loan_occurrences,
                self.state,
                operand.statement_index,
                operand.constraints,
                source,
            )
            .map(BackingFrontier::borrowed),
            StoredOperand::Value {
                capture,
                projection,
                slot,
                ..
            } => {
                if let Some(formed) =
                    self.current_formation(capture, &projection, slot.as_ref(), operand)?
                {
                    Ok(formed)
                } else {
                    self.retained_descriptor(capture, &projection, ancestors, forwarded_inputs)
                }
            }
            StoredOperand::Alternatives(alternatives) => {
                if alternatives.is_empty() {
                    return Err("the stored descriptor has no incoming alternatives");
                }
                let mut result = BackingFrontier::default();
                for alternative in alternatives {
                    result.append(self.stored_frontier(
                        alternative,
                        operand,
                        ancestors,
                        forwarded_inputs,
                    )?);
                }
                Ok(result)
            }
        }
    }

    /// The storage owner selected this exact produced descriptor. Rejoin its
    /// active installed occurrence before following older construction inputs:
    /// those inputs retain backing, but cannot exempt a current descriptor's
    /// own loan from interference. Every matching formation alternative stays.
    fn current_formation(
        &self,
        capture: Handle<FlowOperandFact>,
        projection: &[PlaceSegment],
        slot: Option<&validation::CanonicalPlace>,
        observation: &FlowOperandFact,
    ) -> Result<Option<BackingFrontier>, &'static str> {
        let Some(slot) = slot else {
            return Ok(None);
        };
        let control = &self.facts.flow.control;
        if !control.operands.is_valid(capture) {
            return Err("the installed descriptor has no completed formation");
        }
        let source = control.operands.get(capture);
        if source.state_symbol != self.state.state_symbol {
            return Err("the installed descriptor formation belongs to another state");
        }
        let (_, owner) = self
            .facts
            .borrow
            .first_state_borrow(self.state.machine_symbol, self.state.state_symbol)
            .ok_or("the installed descriptor has no owning borrow state")?;
        let sources = validation::completed_storage::descriptor_formation_sources(
            self.program,
            control,
            capture,
            projection,
            self.state.statements,
        )
        .ok_or("the installed descriptor has no complete formation graph")?;
        let typed_state = self
            .program
            .state_by_symbol(self.state.state_symbol)
            .ok_or("the installed descriptor has no state declaration")?;
        let order = self
            .program
            .statement_table
            .admitted_execution_order(typed_state.statement_nodes);
        let homes = validation::completed_storage::InstalledLoanHomes {
            control,
            semantic: &self.facts.semantic,
            borrow: &self.facts.borrow,
            statements: self.state.statements,
            order: &order,
            state: self.state.state_symbol,
        };
        let mut candidates = Vec::new();
        for handle in self
            .facts
            .flow
            .borrow_loan_constraints(observation.constraints)
        {
            if !self.facts.borrow.state_owns_loan(owner, handle) {
                return Err("the installed descriptor roster contains a foreign loan");
            }
            let loan = self.facts.borrow.loans.get(handle);
            if loan.lineage == checked_trees::BorrowLoanLineage::LifetimeDependency
                || loan.storage_owner != slot.root
            {
                continue;
            }
            let Some(formed) = loan.formation_owner_path else {
                continue;
            };
            let path = self
                .facts
                .borrow
                .owner_segments
                .span(formed)
                .ok_or("the installed descriptor formation has an incomplete leaf path")?;
            if path.contains(&checked_trees::BorrowLoanOwnerSegment::DynamicIndex)
                || !sources.iter().any(|formed| {
                    let value = control.operands.get(formed.capture);
                    value.expression == loan.formation_expression
                        && value.statement_index == loan.statement_index
                        && crate::borrow::owner_path_selects_leaf(
                            self.program,
                            path,
                            &formed.projection,
                        )
                })
                || crate::checks::storage_path_compatibility(
                    self.program,
                    crate::checks::StorageSelectorFacts {
                        flow: self.facts.flow.view(),
                        semantic: &self.facts.semantic,
                    },
                    &crate::borrow::owner_place_segments(&homes.owner_path(self.program, loan)),
                    &slot.segments,
                )
                .1 != checked_trees::CapturedPlaceContainment::Same
            {
                continue;
            }
            candidates.push(super::aliases::ResolvedPlace {
                place: CapturedPlace {
                    incoming_reference: loan.incoming_reference,
                    incoming_owner_path: self
                        .facts
                        .borrow
                        .access_segments
                        .span_or_empty(loan.incoming_owner_path)
                        .to_vec(),
                    root: loan.root,
                    segments: self.facts.borrow.loan_segments(loan).to_vec(),
                },
                local_loan: handle,
                consumed_segments: slot.segments.len(),
            });
        }
        if candidates.is_empty() {
            return Ok(None);
        }
        let coordinate = slot.clone();
        // The completed storage query selected one owned descriptor from one
        // installed value. Matching formation rows are its alternative input
        // debts, even when its receiving owner uses a dynamic index. This does
        // not group raw wildcard storage paths or unrelated descriptors.
        let alternatives = candidates
            .iter()
            .map(|candidate| candidate.local_loan)
            .collect::<Vec<_>>();
        let mut places = super::access_sources::retain_captured_lineage(
            self.program,
            self.facts.view(),
            self.loan_occurrences,
            self.state,
            observation.statement_index,
            CapturedPlace {
                root: coordinate.root,
                segments: coordinate.segments,
                ..Default::default()
            },
            candidates,
        )
        .ok_or("the installed descriptor has incomplete formation lineage")?;
        for place in &mut places {
            for alternative in &alternatives {
                crate::shared::collections::push_unique(&mut place.lineage, *alternative);
            }
        }
        Ok(Some(BackingFrontier::borrowed(places)))
    }

    fn input_descriptor(
        &self,
        capture: Handle<FlowOperandFact>,
        source: validation::borrow_contracts::ViewReturnInputSource,
        path: &[PlaceSegment],
        ancestors: &mut Vec<Handle<FlowOperandFact>>,
        forwarded_inputs: &mut Vec<Handle<FlowOperandFact>>,
    ) -> Result<BackingFrontier, &'static str> {
        let control = &self.facts.flow.control;
        if !control.operands.is_valid(capture) {
            return Err("the return-source relation has no completed actual");
        }
        let actual = control.operands.get(capture);
        // Loading a descriptor stored through an input pointer is distinct
        // from returning that pointer's own referent, including when both
        // declaration-relative paths are empty. Ask the completed-value owner
        // at this actual's captured cut. Its lossless frontier distinguishes
        // an unchanged incoming slot from a performed value. A roots-only
        // lifetime profile is not an evaluated descriptor producer.
        if source == validation::borrow_contracts::ViewReturnInputSource::Stored
            && validation::reference_shell(self.program, actual.type_reference).is_some()
        {
            if !control.operand_retentions.is_valid(actual.retention) {
                return Err("the borrowed input has no captured stored-descriptor inventory");
            }
            let retained = control.operand_retentions.get(actual.retention);
            if !control.operands.is_valid(retained.stored_value) {
                return Err("the borrowed input has no captured contents source");
            }
            let mut projection = control
                .operand_retained_paths
                .span(retained.stored_projection)
                .ok_or("the borrowed input has no captured stored-descriptor projection")?
                .to_vec();
            projection.extend_from_slice(path);
            let frontier = validation::completed_storage::reference_sources_at_path(
                self.program,
                control,
                retained.stored_value,
                &projection,
                actual.storage_snapshot,
            );
            return self.stored_frontier(frontier, actual, ancestors, forwarded_inputs);
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
