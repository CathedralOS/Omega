//! An observation names the value actually read, not a later read from the
//! same reference binding. Its completed source and content-write cut remain
//! distinct from what a repeated source expression would observe now.

use super::FlowBuildContext;
use arena::Handle;
use arena::HandleSpan;
use checked_trees::FlowOperandReferent;
use checked_trees::FlowValueObservationFact;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::type_identity::applied_type_identity;
use validation::expression_traversal::visit_pre_order;
use validation::field_domains::readable_type_reference;
use validation::{
    CanonicalPlace, CheckingInput, DispatchExtraction, applied_case_membership_variant,
    canonical_place_type_reference,
};

mod captured_subject;
mod current_values;
mod descriptors;
pub(super) use current_values::{current_scalar_observation, scalar_observation_at};
pub(crate) use descriptors::{descriptor_context_agrees, descriptor_observation_namespace};
mod dispatch_copies;
pub(crate) use captured_subject::{
    CapturedSubjectFacts, captured_case_subject, captured_case_subject_in_state,
    captured_case_subject_result,
};
pub(super) use dispatch_copies::completed_copy_read_place;
pub(crate) use dispatch_copies::copied_payload_at;

pub(crate) fn completed_dispatch_extraction(
    program: &dyn CheckingInput,
    facts: CapturedSubjectFacts<'_>,
    machine: &Machine,
    state: &State,
    statement_index: usize,
) -> Option<DispatchExtraction> {
    let state_flow = facts
        .flow
        .control
        .unique_state(machine.symbol, state.symbol)?;
    let statement = facts.flow.state_statement(state_flow, statement_index)?;
    let mut extraction = DispatchExtraction::of_transition_arm(program, state, statement_index);
    let mut completed = false;
    for observation in facts
        .flow
        .control
        .value_observations
        .span_or_empty(statement.value_observations)
    {
        if let Some(subject) = captured_case_subject(
            program,
            facts,
            machine,
            state,
            statement_index,
            observation.expression,
        ) {
            extraction.rejoin_completed_subject(
                subject.observed,
                facts.flow.control.operands.get(subject.source).expression,
            );
            completed = true;
        }
    }
    completed.then_some(extraction)
}

pub(super) struct ContentsObservation {
    pub(super) subject: CanonicalPlace,
    pub(super) descriptor: Handle<FlowValueObservationFact>,
    /// A selected descriptor namespace depends on its exact current binding
    /// and every possible pointee; it asserts no fact for each pointee separately.
    pub(super) dependencies: Vec<CanonicalPlace>,
    pub(super) origin_writes: HandleSpan<FlowOperandReferent>,
    pub(super) origin_unknown_write: bool,
}

fn completed_observation_receiver<'flow>(
    build: &'flow FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
) -> Result<
    (
        Handle<FlowValueObservationFact>,
        &'flow FlowValueObservationFact,
        &'flow checked_trees::FlowOperandFact,
    ),
    &'static str,
> {
    let statement = build
        .statement_fact_at(state, statement_index)
        .ok_or("the observation has no uniquely retained source statement")?;
    let mut matching = build
        .control
        .value_observations
        .span(statement.value_observations)
        .ok_or("the statement has no complete observation roster")?
        .iter()
        .filter(|observation| observation.expression == expression);
    let observation = matching
        .next()
        .ok_or("the expression has no completed value observation")?;
    if matching.next().is_some() {
        return Err("the expression has multiple completed observations at this source occurrence");
    }
    if !build.control.operands.is_valid(observation.receiver) {
        return Err("the observation has no retained operand");
    }
    let receiver = build.control.operands.get(observation.receiver);
    if receiver.state_symbol != state
        || receiver.statement_index != statement_index
        || receiver.expression != subject
    {
        return Err("the retained operand belongs to a different source occurrence");
    }
    let handle = build
        .control
        .value_observations
        .handle_of(observation)
        .ok_or("the completed observation has no retained identity")?;
    Ok((handle, observation, receiver))
}

pub(super) fn case_observation(
    program: &dyn CheckingInput,
    build: &mut FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
) -> Option<ContentsObservation> {
    let (descriptor, observation, receiver) =
        completed_observation_receiver(build, state, statement_index, expression, subject).ok()?;
    let referents = build.control.operand_referents.span(receiver.referents)?;
    let canonical = |referent: &FlowOperandReferent| {
        Some(CanonicalPlace {
            root: referent.root,
            segments: build
                .control
                .operand_referent_segments
                .span(referent.segments)?
                .to_vec(),
        })
    };
    let (place, dependencies, descriptor) = match referents {
        [] => return None,
        [referent] => (canonical(referent)?, Vec::new(), Handle::invalid()),
        _ => {
            // One actual descriptor load selects one pointee. Its tag is not
            // a proposition about every possible origin. Retain the exact
            // current carrier namespace and all invalidation dependencies.
            let (selected, place) =
                descriptors::descriptor_capture(program, &build.control, receiver)?;
            let source = build.control.case_accesses.get(selected.case_access);
            if source.evaluation != observation.evaluation
                || source.constraints != observation.constraints
            {
                return None;
            }
            let mut dependencies = vec![place.clone()];
            for referent in referents {
                let pointee = canonical(referent)?;
                crate::shared::collections::push_unique(&mut dependencies, pointee);
            }
            (place, dependencies, descriptor)
        }
    };
    let machine = program.machine_holding_state(state)?;
    let state_definition = program.state_in_machine(machine, state)?;
    if std::iter::once(&place).chain(&dependencies).any(|subject| {
        !super::stable_case_subject(program, machine, state_definition, statement_index, subject)
    }) {
        return None;
    }
    let observed_type = readable_type_reference(program, receiver.type_reference)?;
    let source_type = canonical_place_type_reference(program, state, statement_index, &place)?;
    let source_type = readable_type_reference(program, source_type)?;
    if observed_type != source_type {
        // A readable recast may expose another nominal interpretation of the
        // same bytes. Its tag cannot become evidence for the original carrier.
        // Compare complete applications, including dependent/region operands.
        let owner = program.machine_holding_state(state)?.symbol;
        let observed = applied_type_identity(program.typed_program(), observed_type, owner, &[])?;
        let source = applied_type_identity(program.typed_program(), source_type, owner, &[])?;
        if observed != source {
            return None;
        }
    }
    Some(ContentsObservation {
        subject: place,
        descriptor,
        dependencies,
        origin_writes: observation.origin_writes,
        origin_unknown_write: observation.origin_unknown_write,
    })
}

/// A probe describes its completed operand, not the address of a variable
/// containing a view. A selected descriptor keeps one proof namespace and
/// depends on every possible backing; it establishes no predicate for all of
/// those backings independently. Owned operands use the common captured-value
/// projection, including detached copies and computed values.
pub(super) fn byte_contents_observation(
    program: &dyn CheckingInput,
    build: &FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
) -> Result<ContentsObservation, &'static str> {
    let (descriptor, observation, receiver) =
        completed_observation_receiver(build, state, statement_index, expression, subject)?;
    let (subject, dependencies, descriptor) =
        if super::operand_binds_live_view(program, receiver.type_reference) {
            let (_, namespace) = descriptors::descriptor_capture(program, &build.control, receiver)
                .ok_or("the probe operand has no complete readable descriptor capture")?;
            let mut dependencies = Vec::new();
            for referent in build
                .control
                .operand_referents
                .span(receiver.referents)
                .ok_or("the probe operand has no complete backing roster")?
            {
                if !referent.root.is_valid() {
                    return Err("the probe operand has an unknown backing");
                }
                crate::shared::collections::push_unique(
                    &mut dependencies,
                    CanonicalPlace {
                        root: referent.root,
                        segments: build
                            .control
                            .operand_referent_segments
                            .span(referent.segments)
                            .ok_or("a probe backing has an incomplete path")?
                            .to_vec(),
                    },
                );
            }
            if dependencies.is_empty() {
                return Err("the probe operand has no backing");
            }
            (namespace, dependencies, descriptor)
        } else {
            let places = super::operand_values::captured_places(
                program,
                &build.control,
                receiver,
                receiver.type_reference,
                &super::LiteralExpressionProjection {
                    expression: receiver.expression,
                    remaining: Vec::new(),
                    destination: Vec::new(),
                },
            )
            .ok_or("the probe operand has no complete captured value projection")?;
            let [place] = places.as_slice() else {
                return Err("the probe operand has no single completed value place");
            };
            (
                place.projected.clone(),
                vec![place.storage.clone()],
                Handle::invalid(),
            )
        };
    Ok(ContentsObservation {
        subject,
        descriptor,
        dependencies,
        origin_writes: observation.origin_writes,
        origin_unknown_write: observation.origin_unknown_write,
    })
}

/// A repeated predicate reads both its reference descriptors and the exact
/// referents captured by its case tests. Unknown or ambiguous captures cannot
/// authorize a new assertion about evaluating that predicate again.
pub(super) struct PredicateCaseDependencies {
    pub(super) places: Vec<CanonicalPlace>,
    pub(super) descriptors: Vec<Handle<FlowValueObservationFact>>,
}

pub(super) fn predicate_case_dependencies(
    program: &dyn CheckingInput,
    build: &mut FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PredicateCaseDependencies> {
    let machine = program.machine_holding_state(state)?;
    let state_definition = program.state_in_machine(machine, state)?;
    let mut observations = Vec::new();
    visit_pre_order(program, vec![expression], |expression, node| {
        if let ExpressionNode::Binary(binary) = node
            && applied_case_membership_variant(
                program,
                machine,
                Some(state_definition),
                expression,
                binary,
            )
            .is_some()
        {
            observations.push((expression, binary.left));
        }
        false
    });
    let mut dependencies = Vec::new();
    let mut descriptors = Vec::new();
    for (observation, subject) in observations {
        let captured =
            case_observation(program, build, state, statement_index, observation, subject)?;
        if captured.descriptor.is_valid() && !descriptors.contains(&captured.descriptor) {
            descriptors.push(captured.descriptor);
        }
        crate::shared::collections::push_unique(&mut dependencies, captured.subject);
        if !captured.dependencies.is_empty() {
            // The actual load supplies its binding and all possible backing
            // dependencies. Re-resolving source syntax would both lose that
            // selection and require one arbitrarily chosen physical alias.
            for dependency in captured.dependencies {
                crate::shared::collections::push_unique(&mut dependencies, dependency);
            }
            continue;
        }
        let occurrences = build.expression_occurrences_at(program, subject);
        for occurrence in occurrences.iter().copied() {
            let dependency =
                build.completed_read_place_at(program, state, statement_index, occurrence)?;
            crate::shared::collections::push_unique(&mut dependencies, dependency);
        }
    }
    Some(PredicateCaseDependencies {
        places: dependencies,
        descriptors,
    })
}
