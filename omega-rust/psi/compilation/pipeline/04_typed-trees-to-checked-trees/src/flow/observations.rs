//! A tag observation names the receiver actually evaluated, not a later read
//! of its reference binding. The physical tag and a repeatable expression have
//! distinct lifetimes: descriptor replacement cannot change an observed tag,
//! but it can change what the expression would observe on its next evaluation.

use super::FlowBuildContext;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

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
    program: &dyn validation::CheckingInput,
    facts: CapturedSubjectFacts<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
) -> Option<validation::DispatchExtraction> {
    let state_flow = facts
        .flow
        .control
        .unique_state(machine.symbol, state.symbol)?;
    let statement = facts.flow.state_statement(state_flow, statement_index)?;
    let mut extraction =
        validation::DispatchExtraction::of_transition_arm(program, state, statement_index);
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

pub(super) struct CaseObservation {
    pub(super) subject: CanonicalPlace,
    pub(super) descriptor: arena::Handle<checked_trees::FlowValueObservationFact>,
    /// A selected descriptor namespace depends on its exact current binding
    /// and every possible pointee; it asserts no tag on those other pointees.
    pub(super) dependencies: Vec<CanonicalPlace>,
    pub(super) origin_writes: arena::HandleSpan<checked_trees::FlowOperandReferent>,
    pub(super) origin_unknown_write: bool,
}

pub(super) fn case_observation(
    program: &dyn validation::CheckingInput,
    build: &mut FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
) -> Option<CaseObservation> {
    let statements = super::reference_spans::appended_span_since(
        &build.control.statements,
        build.state_statements_start,
    );
    let mut matching = build
        .control
        .statements
        .span_or_empty(statements)
        .iter()
        .filter(|statement| statement.statement_index == statement_index)
        .flat_map(|statement| {
            build
                .control
                .value_observations
                .span_or_empty(statement.value_observations)
        })
        .filter(|observation| observation.expression == expression);
    let observation = matching.next()?;
    if matching.next().is_some() || !build.control.operands.is_valid(observation.receiver) {
        return None;
    }
    let descriptor = build.control.value_observations.handle_of(observation)?;
    let receiver = *build.control.operands.get(observation.receiver);
    if receiver.state_symbol != state
        || receiver.statement_index != statement_index
        || receiver.expression != subject
    {
        return None;
    }
    let referents = build.control.operand_referents.span(receiver.referents)?;
    let canonical = |referent: &checked_trees::FlowOperandReferent| {
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
        [referent] => (canonical(referent)?, Vec::new(), arena::Handle::invalid()),
        _ => {
            // One actual descriptor load selects one pointee. Its tag is not
            // a proposition about every possible origin. Retain the exact
            // current carrier namespace and all invalidation dependencies.
            let (selected, place) =
                descriptors::descriptor_capture(program, &build.control, &receiver)?;
            let source = build.control.case_accesses.get(selected.case_access);
            if source.evaluation != observation.evaluation
                || source.constraints != observation.constraints
            {
                return None;
            }
            let mut dependencies = vec![place.clone()];
            for referent in referents {
                let pointee = canonical(referent)?;
                crate::collections::push_unique(&mut dependencies, pointee);
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
    let observed_type =
        validation::field_domains::readable_type_reference(program, receiver.type_reference)?;
    let source_type =
        validation::canonical_place_type_reference(program, state, statement_index, &place)?;
    let source_type = validation::field_domains::readable_type_reference(program, source_type)?;
    if observed_type != source_type {
        // A readable recast may expose another nominal interpretation of the
        // same bytes. Its tag cannot become evidence for the original carrier.
        // Compare complete applications, including dependent/region operands.
        let owner = program.machine_holding_state(state)?.symbol;
        let observed = typed_trees::type_identity::applied_type_identity(
            program.typed_program(),
            observed_type,
            owner,
            &[],
        )?;
        let source = typed_trees::type_identity::applied_type_identity(
            program.typed_program(),
            source_type,
            owner,
            &[],
        )?;
        if observed != source {
            return None;
        }
    }
    Some(CaseObservation {
        subject: place,
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
    pub(super) descriptors: Vec<arena::Handle<checked_trees::FlowValueObservationFact>>,
}

pub(super) fn predicate_case_dependencies(
    program: &dyn validation::CheckingInput,
    build: &mut FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<PredicateCaseDependencies> {
    let machine = program.machine_holding_state(state)?;
    let state_definition = program.state_in_machine(machine, state)?;
    let mut observations = Vec::new();
    validation::expression_traversal::visit_pre_order(
        program,
        vec![expression],
        |expression, node| {
            if let typed_trees::expression::ExpressionNode::Binary(binary) = node
                && validation::applied_case_membership_variant(
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
        },
    );
    let mut dependencies = Vec::new();
    let mut descriptors = Vec::new();
    for (observation, subject) in observations {
        let captured =
            case_observation(program, build, state, statement_index, observation, subject)?;
        if captured.descriptor.is_valid() && !descriptors.contains(&captured.descriptor) {
            descriptors.push(captured.descriptor);
        }
        crate::collections::push_unique(&mut dependencies, captured.subject);
        if !captured.dependencies.is_empty() {
            // The actual load supplies its binding and all possible backing
            // dependencies. Re-resolving source syntax would both lose that
            // selection and require one arbitrarily chosen physical alias.
            for dependency in captured.dependencies {
                crate::collections::push_unique(&mut dependencies, dependency);
            }
            continue;
        }
        let occurrences = build.expression_occurrences_at(program, subject);
        for occurrence in occurrences.iter().copied() {
            let dependency =
                build.completed_read_place_at(program, state, statement_index, occurrence)?;
            crate::collections::push_unique(&mut dependencies, dependency);
        }
    }
    Some(PredicateCaseDependencies {
        places: dependencies,
        descriptors,
    })
}
