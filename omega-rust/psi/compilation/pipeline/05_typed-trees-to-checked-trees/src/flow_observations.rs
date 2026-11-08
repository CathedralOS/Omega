/*
    A tag observation names the receiver actually evaluated, not a later read
    of its reference binding. The physical tag and a repeatable expression have
    distinct lifetimes: descriptor replacement cannot change an observed tag,
    but it can change what the expression would observe on its next evaluation.
*/

use super::FlowBuildContext;
use super::reference_spans::appended_span_since;
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

mod captured_subject {
    /*
        Rejoin a completed case read to its completed address formation. A resident
        reference is resolved by the reaching-store producer and its write history,
        never by reading a local declaration's initializer. The retained backing
        capture supplies the address's real evaluated graph; this query supplies
        source correspondence, not referent contents or loan authority.
    */
    use super::super::canonical_places_agree;
    use super::super::captured_operand_handle_at_path;
    use super::super::narrow_pinned_index_segments;
    use super::super::operand_contexts::operand_bindings_agree_in_state;
    use super::super::operand_values::captured_reference_places_with_formation;
    use super::super::stored_values::StoredObservation;
    use super::super::stored_values::StoredOperand;
    use super::super::stored_values::at_snapshot;
    use crate::flow::expressions;
    use crate::published::places::place_at;
    use arena::Handle;
    use checked_trees::{
        BorrowFacts, CheckFacts, FlowFacts, FlowOperandFact, FlowPermissionEvaluation,
    };
    use typed_trees::expression::ExpressionHandle;
    use validation::CanonicalPlace;

    /* Immutable facts needed to authenticate one completed subject correspondence. */
    #[derive(Clone, Copy)]
    pub(crate) struct CapturedSubjectFacts<'facts> {
        pub semantic: &'facts facts::FactPlan,
        pub flow: &'facts FlowFacts,
        pub borrow: &'facts BorrowFacts,
    }

    impl<'facts> From<&'facts CheckFacts> for CapturedSubjectFacts<'facts> {
        fn from(facts: &'facts CheckFacts) -> Self {
            Self {
                semantic: &facts.semantic,
                flow: &facts.flow,
                borrow: &facts.borrow,
            }
        }
    }

    pub(crate) struct CapturedCaseSubject {
        pub observed: ExpressionHandle,
        pub source: Handle<FlowOperandFact>,
        pub receiver: Handle<FlowOperandFact>,
        pub loans: Vec<Handle<checked_trees::BorrowLoanFact>>,
    }

    pub(crate) fn captured_case_subject(
        program: &dyn validation::CheckingInput,
        facts: CapturedSubjectFacts<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CapturedCaseSubject> {
        let state_flow = facts
            .flow
            .control
            .unique_state(machine.symbol, state.symbol)?;
        captured_case_subject_in_state(
            program,
            facts.semantic,
            &facts.flow.control,
            &facts.flow.contexts,
            facts.borrow,
            machine,
            state,
            state_flow.statements,
            statement_index,
            expression,
        )
    }

    /*
        The diagnostic owner needs the exact correspondence refusal, including
        reaching-store, formation-loan and source-binding gates. The Option query
        above and its in-state caller retain their original acceptance behavior.
    */
    pub(crate) enum CapturedCaseSubjectFailure {
        MissingStateFlow,
        MissingOperation,
        MembershipUnavailable,
        Correspondence(&'static str),
    }

    impl CapturedCaseSubjectFailure {
        pub(crate) fn reason(&self) -> &'static str {
            match self {
                Self::MissingStateFlow => "case observation has no unique owning state flow",
                Self::MissingOperation => "case observation has no binary membership operation",
                Self::MembershipUnavailable => {
                    "case observation membership authentication is unavailable"
                }
                Self::Correspondence(reason) => reason,
            }
        }
    }

    pub(crate) fn captured_case_subject_result(
        program: &dyn validation::CheckingInput,
        facts: CapturedSubjectFacts<'_>,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Result<CapturedCaseSubject, CapturedCaseSubjectFailure> {
        let state_flow = facts
            .flow
            .control
            .unique_state(machine.symbol, state.symbol)
            .ok_or(CapturedCaseSubjectFailure::MissingStateFlow)?;
        captured_case_subject_in_state_result(
            program,
            facts.semantic,
            &facts.flow.control,
            &facts.flow.contexts,
            facts.borrow,
            machine,
            state,
            state_flow.statements,
            statement_index,
            expression,
        )
    }

    /*
        Authenticate the same completed address/loan relation before the current
        state's immutable row is published. All evidence comes from its actual
        statement prefix and current constraint inventory.
    */
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn captured_case_subject_in_state(
        program: &dyn validation::CheckingInput,
        semantic: &facts::FactPlan,
        control: &checked_trees::FlowControlFacts,
        contexts: &checked_trees::FlowContextFacts,
        borrow: &BorrowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: arena::HandleSpan<checked_trees::FlowStatementFact>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Option<CapturedCaseSubject> {
        let subject = captured_case_subject_in_state_result(
            program,
            semantic,
            control,
            contexts,
            borrow,
            machine,
            state,
            statements,
            statement_index,
            expression,
        );
        if let Err(CapturedCaseSubjectFailure::Correspondence(reason)) = subject.as_ref()
            && ::validation::trace_flags::check_sites()
        {
            trace_captured_subject_refusal(
                program,
                control,
                contexts,
                borrow,
                state,
                statements,
                statement_index,
                expression,
                reason,
            );
        }
        subject.ok()
    }

    #[allow(clippy::too_many_arguments)]
    fn captured_case_subject_in_state_result(
        program: &dyn validation::CheckingInput,
        semantic: &facts::FactPlan,
        control: &checked_trees::FlowControlFacts,
        contexts: &checked_trees::FlowContextFacts,
        borrow: &BorrowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: arena::HandleSpan<checked_trees::FlowStatementFact>,
        statement_index: usize,
        expression: ExpressionHandle,
    ) -> Result<CapturedCaseSubject, CapturedCaseSubjectFailure> {
        let binary = expressions(program)
            .binary_node(expression)
            .ok_or(CapturedCaseSubjectFailure::MissingOperation)?;
        validation::applied_case_membership_variant(
            program,
            machine,
            Some(state),
            expression,
            binary,
        )
        .ok_or(CapturedCaseSubjectFailure::MembershipUnavailable)?;
        completed_case_subject(
            program,
            semantic,
            control,
            contexts,
            borrow,
            machine,
            state,
            statements,
            statement_index,
            expression,
            binary.left,
        )
        .map_err(CapturedCaseSubjectFailure::Correspondence)
    }

    /*
        Keep every correspondence gate in its original evaluation order. A refusal
        names its missing evidence for the existing opt-in diagnostic; it never
        substitutes a declaration, a newer stored value, or weaker loan evidence.
    */
    #[allow(clippy::too_many_arguments)]
    fn completed_case_subject(
        program: &dyn validation::CheckingInput,
        semantic: &facts::FactPlan,
        control: &checked_trees::FlowControlFacts,
        contexts: &checked_trees::FlowContextFacts,
        borrow: &BorrowFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statements: arena::HandleSpan<checked_trees::FlowStatementFact>,
        statement_index: usize,
        expression: ExpressionHandle,
        receiver_expression: ExpressionHandle,
    ) -> Result<CapturedCaseSubject, &'static str> {
        let statement = control
            .statements
            .span_or_empty(statements)
            .iter()
            .find(|row| row.statement_index == statement_index)
            .ok_or("missing_current_statement")?;
        let mut observations = control
            .value_observations
            .span(statement.value_observations)
            .ok_or("invalid_value_observation_span")?
            .iter()
            .filter(|observation| {
                observation.expression == expression
                    && observation.evaluation == FlowPermissionEvaluation::TransitionGuard
            });
        let observation = observations
            .next()
            .ok_or("missing_exact_guard_observation")?;
        if observations.next().is_some() {
            return Err("multiple_exact_guard_observations");
        }
        if !control.operands.is_valid(observation.receiver) {
            return Err("invalid_observation_receiver");
        }
        let receiver = control.operands.get(observation.receiver);
        if receiver.expression != receiver_expression
            || receiver.state_symbol != state.symbol
            || receiver.statement_index != statement_index
        {
            return Err("receiver_occurrence_mismatch");
        }
        let observed = referent_place(program, semantic, control, contexts, receiver)
            .ok_or("receiver_referent_not_unique")?;
        let descriptor = place_at(program, state.symbol, statement_index, receiver.expression)
            .ok_or("receiver_descriptor_has_no_coordinate")?;
        let StoredOperand::Value {
            capture,
            reference,
            projection,
            ..
        } = at_snapshot(
            program,
            control,
            receiver.storage_snapshot,
            &descriptor,
            StoredObservation::Binding,
        )
        .ok_or("no_reaching_descriptor_store")?
        else {
            return Err("reaching_descriptor_store_unavailable");
        };
        let capture =
            captured_operand_handle_at_path(program, control, capture, reference, &projection)
                .ok_or("stored_projection_has_no_unique_completed_operand")?;
        let formed = control.operands.get(capture);
        let order = program
            .statement_table
            .admitted_execution_order(state.statement_nodes);
        if formed.state_symbol != state.symbol {
            return Err("formed_descriptor_state_mismatch");
        }
        if order
            .position(formed.statement_index)
            .ok_or("formed_statement_not_scheduled")?
            >= order
                .position(statement_index)
                .ok_or("observation_statement_not_scheduled")?
        {
            return Err("formed_descriptor_not_before_observation");
        }
        let borrow_state = borrow
            .with_state_borrows(machine.symbol, state.symbol, |rows| {
                let [handle] = rows else {
                    return None;
                };
                Some(*handle)
            })
            .ok_or("borrow_state_not_unique")?;
        let borrow_state = borrow.states.get(borrow_state);
        let owner = descriptor
            .root
            .as_symbol()
            .ok_or("descriptor_root_not_symbol")?;
        if !descriptor.segments.is_empty() {
            return Err("descriptor_is_not_whole_binding");
        }
        let forwarded = !control.operands.is_valid(formed.backing);
        let (source_handle, formation_access, forwarded_places) = if forwarded {
            let shell = validation::reference_shell(program, formed.type_reference)
                .ok_or("forwarded_descriptor_has_no_reference_carrier")?;
            if !shell.access.is_readable() {
                return Err("forwarded_descriptor_is_not_readable");
            }
            let places = captured_reference_places_with_formation(
                program,
                control,
                capture,
                formed.type_reference,
                &[],
            )
            .ok_or("forwarded_descriptor_has_no_complete_referents")?;
            if places.len() != 1 {
                return Err("forwarded_descriptor_referent_not_unique");
            }
            (
                capture,
                checked_trees::BorrowAccessKind::from(shell.access),
                places,
            )
        } else {
            (
                formed.backing,
                checked_trees::BorrowAccessKind::Read,
                Vec::new(),
            )
        };
        let loans = contexts
            .constraint_refs
            .span_or_empty(observation.constraints)
            .iter()
            .filter_map(|reference| match reference.kind {
                checked_trees::FlowConstraintKind::BorrowLoan { loan } => Some(loan),
                _ => None,
            })
            .filter(|handle| {
                borrow.state_owns_loan(borrow_state, *handle) && {
                    let loan = borrow.loans.get(*handle);
                    loan.owner_symbol == owner
                        && loan.storage_owner == descriptor.root
                        && loan.owner_path.is_empty()
                        && loan.kind == formation_access
                        && loan.lineage != checked_trees::BorrowLoanLineage::LifetimeDependency
                        && loan.statement_index == formed.statement_index
                        && loan.formation_expression == formed.expression
                        && (!forwarded
                            || forwarded_places.iter().any(|place| {
                                loan.root == place.place.root
                                    && borrow.access_segments.span(loan.segments)
                                        == Some(place.place.segments.as_slice())
                            }))
                }
            })
            .collect::<Vec<_>>();
        if loans.is_empty() {
            return Err("no_exact_live_formation_loan");
        }
        let source = control.operands.get(source_handle);
        if source.state_symbol != state.symbol || source.statement_index != formed.statement_index {
            return Err("completed_backing_occurrence_mismatch");
        }
        let source_place = referent_place(program, semantic, control, contexts, source)
            .ok_or("completed_backing_referent_not_unique")?;
        if !canonical_places_agree(program, &observed, &source_place) {
            return Err("observed_and_completed_backing_addresses_disagree");
        }
        let observed_type =
            validation::field_domains::readable_type_reference(program, receiver.type_reference)
                .ok_or("observed_type_not_readable")?;
        let source_type =
            validation::field_domains::readable_type_reference(program, source.type_reference)
                .ok_or("completed_backing_type_not_readable")?;
        if observed_type != source_type
            && typed_trees::type_identity::applied_type_identity(
                program.typed_program(),
                observed_type,
                machine.symbol,
                &[],
            )
            .ok_or("observed_type_identity_unresolved")?
                != typed_trees::type_identity::applied_type_identity(
                    program.typed_program(),
                    source_type,
                    machine.symbol,
                    &[],
                )
                .ok_or("completed_backing_type_identity_unresolved")?
        {
            return Err("observed_and_completed_backing_types_disagree");
        }
        let coordinate = place_at(
            program,
            state.symbol,
            source.statement_index,
            source.expression,
        )
        .ok_or("completed_backing_has_no_source_coordinate")?;
        if !operand_bindings_agree_in_state(
            program,
            control,
            statements,
            source,
            receiver,
            &coordinate,
        ) {
            return Err("completed_backing_source_binding_changed");
        }
        for segment in &coordinate.segments {
            if let facts::PlaceSegment::Index { expression } = segment {
                let selector = place_at(program, state.symbol, source.statement_index, *expression)
                    .ok_or("source_selector_has_no_coordinate")?;
                if !operand_bindings_agree_in_state(
                    program, control, statements, source, receiver, &selector,
                ) {
                    return Err("completed_backing_selector_binding_changed");
                }
            }
        }
        Ok(CapturedCaseSubject {
            observed: receiver.expression,
            source: if forwarded {
                observation.receiver
            } else {
                source_handle
            },
            receiver: observation.receiver,
            loans,
        })
    }

    /*
        Private diagnostic inventory over immutable captures. Handle validity and
        span bounds remain checked even when the refusal describes malformed input.
    */
    #[allow(clippy::too_many_arguments)]
    fn trace_captured_subject_refusal(
        program: &dyn validation::CheckingInput,
        control: &checked_trees::FlowControlFacts,
        contexts: &checked_trees::FlowContextFacts,
        borrow: &BorrowFacts,
        state: &typed_trees::state::State,
        statements: arena::HandleSpan<checked_trees::FlowStatementFact>,
        statement_index: usize,
        expression: ExpressionHandle,
        reason: &str,
    ) {
        eprintln!(
            "CHECKSITE captured-case-subject state={} statement={} expression={:?} source={} refusal={}",
            state.name,
            statement_index,
            expression,
            program.expression_table.display_name(expression),
            reason
        );
        for statement in control.statements.span_or_empty(statements) {
            eprintln!(
                "CHECKSITE captured-case-statement index={} destination={:?} stored={:?} binding_writes={:?} binding_unknown={} content_writes={:?} content_unknown={}",
                statement.statement_index,
                statement.stored_destination,
                statement.stored_value,
                statement.completed_writes,
                statement.completed_unknown_write,
                statement.completed_content_writes,
                statement.completed_content_unknown_write
            );
            if statement.statement_index != statement_index {
                continue;
            }
            for observation in control
                .value_observations
                .span_or_empty(statement.value_observations)
            {
                if observation.expression != expression
                    || observation.evaluation != FlowPermissionEvaluation::TransitionGuard
                {
                    continue;
                }
                eprintln!(
                    "CHECKSITE captured-case-observation receiver={:?} constraints={:?}",
                    observation.receiver, observation.constraints
                );
                if !control.operands.is_valid(observation.receiver) {
                    continue;
                }
                let receiver = control.operands.get(observation.receiver);
                eprintln!(
                    "CHECKSITE captured-case-receiver {:?} referents={:?} bindings={:?}",
                    receiver,
                    control.operand_referents.span_or_empty(receiver.referents),
                    control
                        .operand_origin_writes
                        .span_or_empty(receiver.entry_binding_writes)
                );
                for reference in contexts
                    .constraint_refs
                    .span_or_empty(observation.constraints)
                {
                    if let checked_trees::FlowConstraintKind::BorrowLoan { loan } = reference.kind {
                        if borrow.loans.is_valid(loan) {
                            eprintln!(
                                "CHECKSITE captured-case-live-loan {:?} {:?}",
                                loan,
                                borrow.loans.get(loan)
                            );
                        } else {
                            eprintln!("CHECKSITE captured-case-live-loan {:?} unavailable", loan);
                        }
                    }
                }
                if let Some(descriptor) =
                    place_at(program, state.symbol, statement_index, receiver.expression)
                    && let Some(StoredOperand::Value {
                        capture,
                        reference,
                        projection,
                        ..
                    }) = at_snapshot(
                        program,
                        control,
                        receiver.storage_snapshot,
                        &descriptor,
                        StoredObservation::Binding,
                    )
                {
                    eprintln!(
                        "CHECKSITE captured-case-stored capture={:?} type={:?} projection={:?}",
                        capture, reference, projection
                    );
                    if let Some(formed) = captured_operand_handle_at_path(
                        program,
                        control,
                        capture,
                        reference,
                        &projection,
                    ) && control.operands.is_valid(formed)
                    {
                        let formed = control.operands.get(formed);
                        eprintln!("CHECKSITE captured-case-formed {:?}", formed);
                        if control.operands.is_valid(formed.backing) {
                            let source = control.operands.get(formed.backing);
                            eprintln!(
                                "CHECKSITE captured-case-source {:?} referents={:?} bindings={:?}",
                                source,
                                control.operand_referents.span_or_empty(source.referents),
                                control
                                    .operand_origin_writes
                                    .span_or_empty(source.entry_binding_writes)
                            );
                        }
                    }
                }
            }
        }
    }

    fn referent_place(
        program: &dyn validation::CheckingInput,
        semantic: &facts::FactPlan,
        control: &checked_trees::FlowControlFacts,
        contexts: &checked_trees::FlowContextFacts,
        operand: &FlowOperandFact,
    ) -> Option<CanonicalPlace> {
        let [referent] = control.operand_referents.span(operand.referents)? else {
            return None;
        };
        let mut place = CanonicalPlace {
            root: referent.root,
            segments: control
                .operand_referent_segments
                .span(referent.segments)?
                .to_vec(),
        };
        let contexts = contexts
            .constraint_refs
            .span_or_empty(operand.constraints)
            .iter()
            .filter_map(|reference| match reference.kind {
                checked_trees::FlowConstraintKind::SemanticContext { context } => Some(context),
                _ => None,
            })
            .collect::<Vec<_>>();
        narrow_pinned_index_segments(
            program,
            semantic,
            operand.state_symbol,
            operand.statement_index,
            &contexts,
            &mut place.segments,
        );
        Some(place)
    }
}
mod current_values {
    /*
        Current relational formulas bind the storage actually observed by Flow.
        Cached scalar algebra carries source occurrences only. A completed structural
        read supplies its address and content epoch; neither a later selector value
        nor an equal-looking source path can replace that observation.
    */
    use super::super::FlowBuildContext;
    use super::super::operand_contexts::observation_prefixes_agree;
    use super::super::reference_spans::appended_span_since;
    use super::super::stable_case_subject;
    use arena::HandleSpan;
    use checked_trees::{CheckedScalarExpression, FlowOperandReferent, FlowValueObservationFact};
    use facts::BooleanPredicateOperand;
    use symbols::SymbolHandle;
    use typed_trees::types::TypeReferenceHandle;
    use validation::field_domains::readable_type_reference;
    use validation::{CanonicalPlace, CheckingInput, expression_type_reference_in_state};

    pub(in crate::flow) struct ScalarObservation<'flow> {
        pub place: CanonicalPlace,
        pub reference: TypeReferenceHandle,
        pub read: Option<&'flow FlowValueObservationFact>,
    }

    /*
        Resolve the producer's actual selected geometry once. A zero observation
        belongs to declared/formal algebra and never enters a saved-copy namespace.
        A nonzero but unavailable occurrence is unknown, not a declaration fallback.
    */
    pub(in crate::flow) fn scalar_observation_at<'flow>(
        program: &dyn CheckingInput,
        build: &'flow FlowBuildContext<'_>,
        state: SymbolHandle,
        statement_index: usize,
        operand: &CheckedScalarExpression,
    ) -> Option<ScalarObservation<'flow>> {
        let (place, reference, read) = match operand {
            CheckedScalarExpression::StructuralParameterIndexedRead { observation, .. }
            | CheckedScalarExpression::StructuralParameterField { observation, .. }
                if observation.is_valid() =>
            {
                if !program.expression_table.expression_is_valid(*observation) {
                    return None;
                }
                let statements =
                    appended_span_since(&build.control.statements, build.state_statements_start);
                let mut matching = build
                    .control
                    .statements
                    .span(statements)?
                    .iter()
                    .filter(|statement| statement.statement_index == statement_index)
                    .flat_map(|statement| {
                        build
                            .control
                            .value_observations
                            .span_or_empty(statement.value_observations)
                    })
                    .filter(|read| read.expression == *observation);
                let read = matching.next()?;
                if matching.next().is_some() || !build.control.operands.is_valid(read.receiver) {
                    return None;
                }
                let receiver = build.control.operands.get(read.receiver);
                if receiver.state_symbol != state || receiver.statement_index != statement_index {
                    return None;
                }
                let [subject] = build.control.operand_referents.span(read.places)? else {
                    return None;
                };
                let source = CanonicalPlace {
                    root: subject.root,
                    segments: build
                        .control
                        .operand_referent_segments
                        .span(subject.segments)?
                        .to_vec(),
                };
                let reference = expression_type_reference_in_state(
                    program,
                    state,
                    statement_index,
                    *observation,
                )?;
                let reference = readable_type_reference(program, reference)?;
                (source, reference, Some(read))
            }
            _ => {
                let (place, reference) =
                    crate::published::facts::boolean_predicates::declared_observation(
                        program,
                        state,
                        statement_index,
                        &BooleanPredicateOperand::Scalar(operand.clone()),
                    )?;
                (place, reference, None)
            }
        };
        Some(ScalarObservation {
            place,
            reference,
            read,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(in crate::flow) fn current_scalar_observation(
        program: &dyn CheckingInput,
        build: &FlowBuildContext<'_>,
        state: SymbolHandle,
        statement_index: usize,
        writes: HandleSpan<FlowOperandReferent>,
        unknown_write: bool,
        operand: &CheckedScalarExpression,
    ) -> Option<(CanonicalPlace, TypeReferenceHandle)> {
        let ScalarObservation {
            place,
            reference,
            read,
        } = scalar_observation_at(program, build, state, statement_index, operand)?;
        if let Some(read) = read {
            let machine = program.machine_holding_state(state)?;
            let state_definition = program.state_in_machine(machine, state)?;
            if !stable_case_subject(program, machine, state_definition, statement_index, &place)
                || !observation_prefixes_agree(
                    program,
                    &build.control,
                    read.origin_writes,
                    read.origin_unknown_write,
                    writes,
                    unknown_write,
                    &place,
                )
            {
                return None;
            }
        }
        Some((place, reference))
    }
}
mod descriptors {
    /*
        A selected descriptor has one value observation and several possible
        backing places. A tag on that value is not a tag on every backing. Rejoin
        the actual carrier capture, its binding version and all live contents
        before lending that observation namespace to another completed read.
    */
    use super::super::operand_contexts::operand_bindings_agree_in_state;
    use super::super::operand_contexts::operand_sources_agree_in_state;
    use super::super::operand_values::captured_retention_profiles_at_path;
    use super::super::value_origins::ValueObservation;
    use arena::HandleSpan;
    use checked_trees::{FlowControlFacts, FlowOperandFact};
    use facts::FactContextHandle;
    use validation::CanonicalPlace;

    /*
        The producer's exact descriptor-load namespace, independently of its
        pointee addresses. A type, source expression or address equality alone is
        insufficient: the actual source access and complete retention must exist.
    */
    pub(super) fn descriptor_namespace(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
    ) -> Option<CanonicalPlace> {
        let shell = validation::reference_shell(program, operand.type_reference)?;
        if !shell.access.is_readable()
            || !control.case_accesses.is_valid(operand.case_access)
            || control
                .operand_referents
                .span(operand.referents)?
                .is_empty()
            || !same_retention(program, control, operand, operand)
        {
            return None;
        }
        let access = control.case_accesses.get(operand.case_access);
        if access.expression != operand.expression
            || access.state_symbol != operand.state_symbol
            || access.statement_index != operand.statement_index
            || access.constraints != operand.constraints
        {
            return None;
        }
        let [source] = control.operand_referents.span(access.places)? else {
            return None;
        };
        if source.formation.is_valid() {
            return None;
        }
        Some(CanonicalPlace {
            root: source.root,
            segments: control
                .operand_referent_segments
                .span(source.segments)?
                .to_vec(),
        })
    }

    /*
        Each actual descriptor load adds a formation endpoint for its own source
        access. Compare the retained descriptor beneath only those authenticated
        current-read wrappers; saved-owner and earlier formation identities remain.
    */
    fn descriptor_source<'facts>(
        control: &'facts FlowControlFacts,
        operand: &FlowOperandFact,
        mut referent: &'facts checked_trees::FlowOperandReferent,
    ) -> Option<&'facts checked_trees::FlowOperandReferent> {
        let mut visited = Vec::new();
        while referent.formation.is_valid() {
            if !control.referent_formations.is_valid(referent.formation)
                || visited.contains(&referent.formation)
            {
                return None;
            }
            visited.push(referent.formation);
            let formation = control.referent_formations.get(referent.formation);
            if formation.owner.is_valid()
                || !formation.path.is_empty()
                || !control.case_accesses.is_valid(formation.source_access())
            {
                break;
            }
            let access = control.case_accesses.get(formation.source_access());
            if access.expression != operand.expression
                || access.state_symbol != operand.state_symbol
                || access.statement_index != operand.statement_index
                || access.constraints != operand.constraints
            {
                break;
            }
            let (Some(actual_path), Some(formed_path)) = (
                control.operand_referent_segments.span(referent.segments),
                control.operand_referent_segments.span(formation.segments),
            ) else {
                return None;
            };
            if formation.root != referent.root
                || formed_path != actual_path
                || !control.operand_referents.is_valid(formation.source)
                || !access
                    .places
                    .handles()
                    .any(|source| source == formation.source)
            {
                return None;
            }
            let source = control.operand_referents.get(formation.source);
            if source.root != referent.root
                || control.operand_referent_segments.span(source.segments)? != actual_path
            {
                return None;
            }
            referent = source;
        }
        Some(referent)
    }

    fn same_referents(
        control: &FlowControlFacts,
        first: &FlowOperandFact,
        second: &FlowOperandFact,
    ) -> bool {
        let (Some(first_referents), Some(second_referents)) = (
            control.operand_referents.span(first.referents),
            control.operand_referents.span(second.referents),
        ) else {
            return false;
        };
        let same = |first_source: &checked_trees::FlowOperandReferent,
                    second_source: &checked_trees::FlowOperandReferent| {
            let (Some(first), Some(second)) = (
                descriptor_source(control, first, first_source),
                descriptor_source(control, second, second_source),
            ) else {
                return false;
            };
            let (Some(first_path), Some(second_path)) = (
                control.operand_referent_segments.span(first.segments),
                control.operand_referent_segments.span(second.segments),
            ) else {
                return false;
            };
            first.formation == second.formation
                && first.root == second.root
                && first_path == second_path
        };
        !first_referents.is_empty()
            && first_referents
                .iter()
                .all(|first| second_referents.iter().any(|second| same(first, second)))
            && second_referents
                .iter()
                .all(|second| first_referents.iter().any(|first| same(first, second)))
    }

    fn same_retention(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        first: &FlowOperandFact,
        second: &FlowOperandFact,
    ) -> bool {
        type Retention = arena::Handle<checked_trees::FlowOperandRetention>;

        fn selected_profiles(
            program: &dyn validation::CheckingInput,
            control: &FlowControlFacts,
            retention: Retention,
        ) -> Option<Vec<Retention>> {
            if !control.operand_retentions.is_valid(retention) {
                return None;
            }
            let profile = control.operand_retentions.get(retention);
            let path = control
                .operand_retained_paths
                .span(profile.stored_projection)?;
            if path.is_empty() {
                return Some(vec![retention]);
            }
            if !control.operands.is_valid(profile.stored_value) {
                return None;
            }
            captured_retention_profiles_at_path(
                program,
                control,
                control.operands.get(profile.stored_value),
                path,
            )
        }

        fn alternatives_agree(
            program: &dyn validation::CheckingInput,
            control: &FlowControlFacts,
            first: &[Retention],
            second: &[Retention],
            visited: &mut Vec<(Retention, Retention)>,
        ) -> bool {
            !first.is_empty()
                && !second.is_empty()
                && first.iter().all(|first| {
                    second
                        .iter()
                        .any(|second| profiles_agree(program, control, *first, *second, visited))
                })
                && second.iter().all(|second| {
                    first
                        .iter()
                        .any(|first| profiles_agree(program, control, *first, *second, visited))
                })
        }

        fn profiles_agree(
            program: &dyn validation::CheckingInput,
            control: &FlowControlFacts,
            first: Retention,
            second: Retention,
            visited: &mut Vec<(Retention, Retention)>,
        ) -> bool {
            if !control.operand_retentions.is_valid(first)
                || !control.operand_retentions.is_valid(second)
                || visited.contains(&(first, second))
            {
                return false;
            }
            visited.push((first, second));
            let agreed = (|| {
                let first_profile = control.operand_retentions.get(first);
                let second_profile = control.operand_retentions.get(second);
                if !first_profile.stored_projection.is_empty()
                    || !second_profile.stored_projection.is_empty()
                {
                    let (Some(first), Some(second)) = (
                        selected_profiles(program, control, first),
                        selected_profiles(program, control, second),
                    ) else {
                        return false;
                    };
                    return alternatives_agree(program, control, &first, &second, visited);
                }
                let (Some(first_roots), Some(second_roots)) = (
                    control.retained_roots(first),
                    control.retained_roots(second),
                ) else {
                    return false;
                };
                if (first_profile.stored_value.is_valid()
                    && !control.operands.is_valid(first_profile.stored_value))
                    || (second_profile.stored_value.is_valid()
                        && !control.operands.is_valid(second_profile.stored_value))
                    || first_profile.stored_value != second_profile.stored_value
                    || !first_roots.iter().all(|root| second_roots.contains(root))
                    || !second_roots.iter().all(|root| first_roots.contains(root))
                {
                    return false;
                }
                let (Some(first_leaves), Some(second_leaves)) = (
                    control.operand_retained_leaves.span(first_profile.leaves),
                    control.operand_retained_leaves.span(second_profile.leaves),
                ) else {
                    return false;
                };
                if first_leaves.len() != second_leaves.len()
                    || !first_leaves
                        .iter()
                        .zip(second_leaves)
                        .all(|(first, second)| {
                            let (Some(first_path), Some(second_path)) = (
                                control.operand_retained_paths.span(first.path),
                                control.operand_retained_paths.span(second.path),
                            ) else {
                                return false;
                            };
                            first_path == second_path
                                && profiles_agree(
                                    program,
                                    control,
                                    first.retention,
                                    second.retention,
                                    visited,
                                )
                        })
                {
                    return false;
                }
                let first_alternatives: Vec<_> = first_profile.alternatives.handles().collect();
                let second_alternatives: Vec<_> = second_profile.alternatives.handles().collect();
                if first_alternatives.is_empty() && second_alternatives.is_empty() {
                    true
                } else {
                    alternatives_agree(
                        program,
                        control,
                        &first_alternatives,
                        &second_alternatives,
                        visited,
                    )
                }
            })();
            visited.pop();
            agreed
        }
        let (Some(first), Some(second)) = (
            captured_retention_profiles_at_path(program, control, first, &[]),
            captured_retention_profiles_at_path(program, control, second, &[]),
        ) else {
            return false;
        };
        alternatives_agree(program, control, &first, &second, &mut Vec::new())
    }

    /*
        Contextual value observations preserve a same-expression descriptor through
        their actual backing edge. A different expression or detached value cannot
        be replaced by whichever reference happens to occupy the source slot now.
    */
    pub(super) fn descriptor_capture<'facts>(
        program: &dyn validation::CheckingInput,
        control: &'facts FlowControlFacts,
        mut operand: &'facts FlowOperandFact,
    ) -> Option<(&'facts FlowOperandFact, CanonicalPlace)> {
        let mut visited = Vec::new();
        loop {
            if let Some(namespace) = descriptor_namespace(program, control, operand) {
                return Some((operand, namespace));
            }
            let backing = operand.backing;
            if !control.operands.is_valid(backing) || visited.contains(&backing) {
                return None;
            }
            visited.push(backing);
            let source = control.operands.get(backing);
            if source.expression != operand.expression
                || source.state_symbol != operand.state_symbol
                || source.statement_index != operand.statement_index
            {
                return None;
            }
            operand = source;
        }
    }

    #[derive(Debug)]
    enum DescriptorObservationRefusal {
        MissingObservation,
        MissingReceiver,
        UnavailableObservedDescriptor,
        UnavailableConsumerDescriptor,
        DifferentNamespace,
        DifferentDescriptorSources,
        DifferentRetention,
        UnavailableType,
        DifferentType,
        UnavailableStatementCut,
        ChangedBinding,
        UnavailableDescriptorSources,
        ChangedContents,
    }

    fn observation_agrees(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        observation: arena::Handle<checked_trees::FlowValueObservationFact>,
        operand: &FlowOperandFact,
    ) -> Result<CanonicalPlace, DescriptorObservationRefusal> {
        if !control.value_observations.is_valid(observation) {
            return Err(DescriptorObservationRefusal::MissingObservation);
        }
        let observation = control.value_observations.get(observation);
        if !control.operands.is_valid(observation.receiver) {
            return Err(DescriptorObservationRefusal::MissingReceiver);
        }
        let (observed, namespace) =
            descriptor_capture(program, control, control.operands.get(observation.receiver))
                .ok_or(DescriptorObservationRefusal::UnavailableObservedDescriptor)?;
        let (consumer, actual_namespace) = descriptor_capture(program, control, operand)
            .ok_or(DescriptorObservationRefusal::UnavailableConsumerDescriptor)?;
        if namespace != actual_namespace {
            return Err(DescriptorObservationRefusal::DifferentNamespace);
        }
        if !same_referents(control, observed, consumer) {
            return Err(DescriptorObservationRefusal::DifferentDescriptorSources);
        }
        if !same_retention(program, control, observed, consumer) {
            return Err(DescriptorObservationRefusal::DifferentRetention);
        }
        let machine = program
            .machine_holding_state(observed.state_symbol)
            .ok_or(DescriptorObservationRefusal::UnavailableType)?
            .symbol;
        if typed_trees::type_identity::applied_type_identity(
            program.typed_program(),
            observed.type_reference,
            machine,
            &[],
        )
        .ok_or(DescriptorObservationRefusal::UnavailableType)?
            != typed_trees::type_identity::applied_type_identity(
                program.typed_program(),
                consumer.type_reference,
                machine,
                &[],
            )
            .ok_or(DescriptorObservationRefusal::UnavailableType)?
        {
            return Err(DescriptorObservationRefusal::DifferentType);
        }
        let statements = if observed.statement_index == consumer.statement_index {
            HandleSpan::empty()
        } else {
            control
                .state_by_symbol(observed.state_symbol)
                .ok_or(DescriptorObservationRefusal::UnavailableStatementCut)?
                .statements
        };
        if !operand_bindings_agree_in_state(
            program, control, statements, observed, consumer, &namespace,
        ) {
            return Err(DescriptorObservationRefusal::ChangedBinding);
        }
        let mut completed_read = *observed;
        completed_read.entry_origin_writes = observation.origin_writes;
        completed_read.entry_origin_unknown_write = observation.origin_unknown_write;
        let mut contents = vec![namespace.clone()];
        for referent in control
            .operand_referents
            .span(observed.referents)
            .ok_or(DescriptorObservationRefusal::UnavailableDescriptorSources)?
        {
            contents.push(CanonicalPlace {
                root: referent.root,
                segments: control
                    .operand_referent_segments
                    .span(referent.segments)
                    .ok_or(DescriptorObservationRefusal::UnavailableDescriptorSources)?
                    .to_vec(),
            });
        }
        for place in contents {
            if !operand_sources_agree_in_state(
                program,
                control,
                statements,
                &completed_read,
                consumer,
                &place,
                ValueObservation::Contents,
            ) {
                return Err(DescriptorObservationRefusal::ChangedContents);
            }
        }
        Ok(namespace)
    }

    fn rejoin_observation(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        context: FactContextHandle,
        observation: arena::Handle<checked_trees::FlowValueObservationFact>,
        operand: &FlowOperandFact,
    ) -> Result<CanonicalPlace, DescriptorObservationRefusal> {
        let result = observation_agrees(program, control, observation, operand);
        if let Err(reason) = &result
            && ::validation::trace_flags::check_sites()
        {
            eprintln!(
                "[checked descriptor observation] context={context:?} observation={observation:?} consumer={:?} state={:?} statement={} refusal={reason:?}",
                operand.expression, operand.state_symbol, operand.statement_index
            );
        }
        result
    }

    pub(crate) fn descriptor_context_agrees(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        context: FactContextHandle,
    ) -> bool {
        control
            .descriptor_context_observations
            .for_context(context)
            .all(|row| {
                rejoin_observation(program, control, context, row.observation, operand).is_ok()
            })
    }

    /*
        A proof namespace for this actual selected value. This grants no address,
        extent, loan or lifetime authority. Physical view consumers still use the
        captured referent roster; predicates and payload-case checks share this one
        rejoin instead of separately guessing aliases from source syntax.
    */
    pub(crate) fn descriptor_observation_namespace(
        program: &dyn validation::CheckingInput,
        control: &FlowControlFacts,
        operand: &FlowOperandFact,
        contexts: &[FactContextHandle],
    ) -> Option<CanonicalPlace> {
        let mut selected = None;
        for row in contexts.iter().flat_map(|context| {
            control
                .descriptor_context_observations
                .for_context(*context)
        }) {
            let Ok(namespace) =
                rejoin_observation(program, control, row.context, row.observation, operand)
            else {
                continue;
            };
            if selected
                .as_ref()
                .is_some_and(|previous| previous != &namespace)
            {
                return None;
            }
            selected = Some(namespace);
        }
        selected
    }
}
pub(super) use current_values::{current_scalar_observation, scalar_observation_at};
pub(crate) use descriptors::{descriptor_context_agrees, descriptor_observation_namespace};
mod dispatch_copies {
    /*
        Selected-arm payload copies are completed values before target operands.
        The saved guard receiver supplies the exact address/loan correspondence;
        each copy retains the selected entry facts and its own whole carrier.
        Later storage mutation cannot replace that value. Reference descendants
        still have ordinary live referent epochs, not frozen referee contents.
    */
    use super::super::canonical_place_segments_equal;
    use super::super::operand_binds_live_view;
    use super::super::project_captured_view_place;
    use crate::published::places::place_at;
    use arena::{Handle, HandleSpan};
    use checked_trees::{
        FlowControlFacts, FlowFacts, FlowOperandCompletion, FlowOperandFact,
        FlowPermissionEvaluation,
    };
    use facts::PlaceRoot;
    use symbols::SymbolHandle;
    use typed_trees::expression::ExpressionHandle;
    use validation::CanonicalPlace;
    use validation::CheckingInput;
    use validation::DispatchExtraction;
    use validation::owned_projection_type_reference;
    use validation::project_type_reference_from_segments;

    /*
        The selected target's completed copy actually consumed by this occurrence.
        Readable reference crossings are geometry here, not snapshots of referees.
        The flow operand/observation graph independently authenticates the use;
        source spelling alone never redirects a guard or an address-forming loan.
    */
    pub(crate) fn copied_payload_at<'facts>(
        program: &dyn CheckingInput,
        flow: &'facts FlowFacts,
        machine: SymbolHandle,
        state: SymbolHandle,
        statement: usize,
        evaluation: FlowPermissionEvaluation,
        expression: ExpressionHandle,
    ) -> Option<(Handle<FlowOperandFact>, &'facts FlowOperandFact)> {
        let FlowPermissionEvaluation::TransitionTarget(target) = evaluation else {
            return None;
        };
        let place = place_at(program, state, statement, expression)?;
        let state_flow = flow.control.unique_state(machine, state)?;
        let mut matching = flow
            .control
            .outgoing
            .span_or_empty(state_flow.outgoing)
            .iter()
            .filter(|outgoing| {
                outgoing.statement_index == statement && outgoing.transition_target == target
            })
            .flat_map(|outgoing| outgoing.copied_payloads.handles())
            .filter_map(|handle| {
                let copy = flow.control.operands.get(handle);
                if !DispatchExtraction::reads_payload_binding(program, expression, copy.expression)
                {
                    return None;
                }
                let prefix = place_at(program, state, statement, copy.expression)?;
                if prefix.root != place.root
                    || prefix.segments.len() > place.segments.len()
                    || !prefix
                        .segments
                        .iter()
                        .zip(&place.segments)
                        .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
                {
                    return None;
                }
                project_type_reference_from_segments(
                    program,
                    copy.type_reference,
                    &place.segments[prefix.segments.len()..],
                )?;
                let FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion else {
                    return None;
                };
                if !flow
                    .control
                    .copied_value_uses
                    .span_or_empty(flow.control.copied_values.get(identity).uses)
                    .iter()
                    .any(|usage| usage.expression == expression && usage.evaluation == evaluation)
                {
                    return None;
                }
                Some((handle, copy))
            });
        let found = matching.next()?;
        matching.next().is_none().then_some(found)
    }

    /*
        Resolve a value read within an active selected-copy scope. An unmatched
        place keeps its ordinary identity; an unresolved copied reference never
        falls back to a newer descriptor at the old source coordinate.
    */
    pub(in crate::flow) fn completed_copy_read_place(
        program: &dyn CheckingInput,
        control: &FlowControlFacts,
        copies: HandleSpan<FlowOperandFact>,
        state: SymbolHandle,
        statement: usize,
        expression: ExpressionHandle,
        place: &CanonicalPlace,
    ) -> Option<CanonicalPlace> {
        let mut matching = Vec::new();
        for copy in control.operands.span_or_empty(copies) {
            if !DispatchExtraction::reads_payload_binding(program, expression, copy.expression) {
                continue;
            }
            let prefix = place_at(program, state, statement, copy.expression)?;
            if prefix.root != place.root
                || prefix.segments.len() > place.segments.len()
                || !prefix
                    .segments
                    .iter()
                    .zip(&place.segments)
                    .all(|(left, right)| canonical_place_segments_equal(program, *left, *right))
            {
                continue;
            }
            let path = &place.segments[prefix.segments.len()..];
            let FlowOperandCompletion::Copied { copy: identity, .. } = copy.completion else {
                return None;
            };
            let record = control.copied_values.get(identity);
            let mut leaves = control
                .captured_reference_leaves
                .span_or_empty(record.references)
                .iter()
                .filter(|leaf| {
                    path.starts_with(control.operand_child_paths.span_or_empty(leaf.path))
                });
            let projected = if let Some(leaf) = leaves.next() {
                if leaves.next().is_some() {
                    return None;
                }
                let [referent] = control.operand_referents.span_or_empty(leaf.referents) else {
                    return None;
                };
                let place = CanonicalPlace {
                    root: referent.root,
                    segments: control
                        .operand_referent_segments
                        .span_or_empty(referent.segments)
                        .to_vec(),
                };
                project_captured_view_place(place, &path[leaf.path.count() as usize..])?
            } else {
                let projected =
                    owned_projection_type_reference(program, copy.type_reference, path)?;
                if operand_binds_live_view(program, projected) {
                    return None;
                }
                CanonicalPlace {
                    root: PlaceRoot::Expression(copy.expression),
                    segments: path.to_vec(),
                }
            };
            matching.push(projected);
        }
        match matching.as_slice() {
            [] => Some(place.clone()),
            [place] => Some(place.clone()),
            _ => None,
        }
    }
}
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
    let mut extraction =
        DispatchExtraction::of_transition_arm(program, machine, state, statement_index);
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
    pub(super) descriptor: Handle<FlowValueObservationFact>,
    /*
        A selected descriptor namespace depends on its exact current binding
        and every possible pointee; it asserts no tag on those other pointees.
    */
    pub(super) dependencies: Vec<CanonicalPlace>,
    pub(super) origin_writes: HandleSpan<FlowOperandReferent>,
    pub(super) origin_unknown_write: bool,
}

pub(super) fn case_observation(
    program: &dyn CheckingInput,
    build: &mut FlowBuildContext<'_>,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    subject: ExpressionHandle,
) -> Option<CaseObservation> {
    let statements = appended_span_since(&build.control.statements, build.state_statements_start);
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
        let owner = program.machine_holding_state(state)?.symbol;
        let observed = applied_type_identity(program.typed_program(), observed_type, owner, &[])?;
        let source = applied_type_identity(program.typed_program(), source_type, owner, &[])?;
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

/*
    A repeated predicate reads both its reference descriptors and the exact
    referents captured by its case tests. Unknown or ambiguous captures cannot
    authorize a new assertion about evaluating that predicate again.
*/
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
