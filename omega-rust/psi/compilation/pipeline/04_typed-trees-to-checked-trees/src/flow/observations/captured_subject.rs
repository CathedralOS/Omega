//! Rejoin a completed case read to its completed address formation. A resident
//! reference is resolved by the reaching-store producer and its write history,
//! never by reading a local declaration's initializer. The retained backing
//! capture supplies the address's real evaluated graph; this query supplies
//! source correspondence, not referent contents or loan authority.

use arena::Handle;
use checked_trees::{
    BorrowFacts, CheckFacts, FlowFacts, FlowOperandFact, FlowPermissionEvaluation,
};
use typed_trees::expression::ExpressionHandle;
use validation::CanonicalPlace;

/// Immutable facts needed to authenticate one completed subject correspondence.
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

/// The diagnostic owner needs the exact correspondence refusal, including
/// reaching-store, formation-loan and source-binding gates. The Option query
/// above and its in-state caller retain their original acceptance behavior.
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

/// Authenticate the same completed address/loan relation before the current
/// state's immutable row is published. All evidence comes from its actual
/// statement prefix and current constraint inventory.
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
        && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some()
    {
        trace_captured_subject_refusal(
            program,
            semantic,
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
    let binary = program
        .expression_table
        .binary_node(expression)
        .ok_or(CapturedCaseSubjectFailure::MissingOperation)?;
    validation::applied_case_membership_variant(program, machine, Some(state), expression, binary)
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

/// Keep every correspondence gate in its original evaluation order. A refusal
/// names its missing evidence for the existing opt-in diagnostic; it never
/// substitutes a declaration, a newer stored value, or weaker loan evidence.
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
    let descriptor = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        receiver.expression,
    )
    .ok_or("receiver_descriptor_has_no_coordinate")?;
    let super::super::stored_values::StoredOperand::Value {
        capture,
        reference,
        projection,
    } = super::super::stored_values::at_snapshot(
        program,
        control,
        receiver.storage_snapshot,
        &descriptor,
        super::super::stored_values::StoredObservation::Binding,
    )
    .ok_or("no_reaching_descriptor_store")?
    else {
        return Err("reaching_descriptor_store_unavailable");
    };
    let capture = super::super::captured_operand_handle_at_path(
        program,
        control,
        capture,
        reference,
        &projection,
    )
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
    // The current read must retain the very leaf formed by this completed
    // store. An equal address or an earlier declaration does not identify a
    // completed descriptor occurrence.
    let owner = descriptor
        .root
        .as_symbol()
        .ok_or("descriptor_root_not_symbol")?;
    if !descriptor.segments.is_empty() {
        return Err("descriptor_is_not_whole_binding");
    }
    // A formed borrow retains its separate source operand. Forwarding an
    // existing reference instead retains the descriptor itself: no second
    // backing operand is manufactured for an entry reference or stored view.
    let forwarded = !control.operands.is_valid(formed.backing);
    let (source_handle, formation_access, forwarded_places) = if forwarded {
        let shell = validation::reference_shell(program, formed.type_reference)
            .ok_or("forwarded_descriptor_has_no_reference_carrier")?;
        if !shell.access.is_readable() {
            return Err("forwarded_descriptor_is_not_readable");
        }
        let places = super::super::operand_values::captured_reference_places_with_formation(
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
    if !super::super::canonical_places_agree(program, &observed, &source_place) {
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
    let coordinate = validation::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        source.statement_index,
        source.expression,
    )
    .ok_or("completed_backing_has_no_source_coordinate")?;
    // A stored pointer can keep its old address after an intermediate slot or
    // selector changes. Source spelling is reusable only when those binding
    // coordinates still denote the captured address at this actual read.
    if !super::super::operand_contexts::operand_bindings_agree_in_state(
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
            let selector = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                source.statement_index,
                *expression,
            )
            .ok_or("source_selector_has_no_coordinate")?;
            if !super::super::operand_contexts::operand_bindings_agree_in_state(
                program, control, statements, source, receiver, &selector,
            ) {
                return Err("completed_backing_selector_binding_changed");
            }
        }
    }
    Ok(CapturedCaseSubject {
        observed: receiver.expression,
        // All formed->guard version checks above still consume the actual
        // earlier descriptor. Its current guard read owns the payload's saved
        // namespace only after that correspondence has been established.
        source: if forwarded {
            observation.receiver
        } else {
            source_handle
        },
        receiver: observation.receiver,
        loans,
    })
}

/// Private diagnostic inventory over immutable captures. Handle validity and
/// span bounds remain checked even when the refusal describes malformed input.
#[allow(clippy::too_many_arguments)]
fn trace_captured_subject_refusal(
    program: &dyn validation::CheckingInput,
    semantic: &facts::FactPlan,
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
            if let Some(descriptor) = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                receiver.expression,
            ) {
                if let Some(super::super::stored_values::StoredOperand::Value {
                    capture,
                    reference,
                    projection,
                }) = super::super::stored_values::at_snapshot(
                    program,
                    control,
                    receiver.storage_snapshot,
                    &descriptor,
                    super::super::stored_values::StoredObservation::Binding,
                ) {
                    eprintln!(
                        "CHECKSITE captured-case-stored capture={:?} type={:?} projection={:?}",
                        capture, reference, projection
                    );
                    if let Some(formed) = super::super::captured_operand_handle_at_path(
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
    super::super::narrow_pinned_index_segments(
        program,
        semantic,
        operand.state_symbol,
        operand.statement_index,
        &contexts,
        &mut place.segments,
    );
    Some(place)
}
