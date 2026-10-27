//! Local state arrivals evaluate arguments in authored order and capture scalar
//! snapshots before committing the arrival transfer of existing reference carriers. A copied observation through that carrier's own route
//! does not create a competing successor loan. This follows state_contracts.md
//! (Bindings and arrivals) and expressions.md (Evaluation schedule), independent
//! of attached-data namespaces. The distinction is transfer versus formation:
//! explicit new borrows, retained observations, other alias routes, nested-call
//! writes and ordinary returning calls keep their normal conflict judgments.
//! Each invocation checks its live access windows against the active loans.
//! Finished nested inputs are checked at their own entry, while descendants
//! stored by that invocation remain active; no parent authority is restored.

use checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, CheckFacts, FlowStateFact,
};
use typed_trees::expression::ExpressionNode;

/// One exact invocation boundary in the shared evaluated occurrence schedule.
/// This is borrowed-check construction scratch, not a new loan or cleanup plan.
pub(in crate::checks::borrows) struct InvocationBoundary {
    pub(in crate::checks::borrows) occurrences:
        Vec<crate::checks::ownership_occurrences::StatementOccurrence>,
    pub(in crate::checks::borrows) completion: usize,
    pub(in crate::checks::borrows) ends_source_path: bool,
}

impl InvocationBoundary {
    pub(in crate::checks::borrows) fn at(
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        call: &BorrowCallFact,
    ) -> Option<Self> {
        use crate::checks::ownership_occurrences::InvocationIdentity;
        let (machine, state) =
            validation::machine_state_by_symbols(program, flow.machine_symbol, flow.state_symbol)?;
        facts
            .flow
            .control
            .statements
            .span(flow.statements)?
            .iter()
            .find(|statement| statement.statement_index == call.statement_index)?;
        let node = program
            .statement_table
            .statement_at(state, call.statement_index)?;
        let Some(site) = validation::find_call_site_in_state(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        ) else {
            return None;
        };
        let target = validation::transition_call_target(
            program,
            machine,
            state,
            call.statement_index,
            call.call_ordinal,
        );
        // An outgoing edge cuts off later lexical guards/statements only when
        // this is its actual invocation, not a nested call in an operand or guard.
        let ends_source_path = target.is_some_and(|target| {
            match (program.statement_table.transition_target(target), site) {
                (
                    typed_trees::statement::TransitionTargetNode::Value(value),
                    validation::CallSite::Expression { expression, .. },
                ) => *value == expression,
                (
                    typed_trees::statement::TransitionTargetNode::Named { .. },
                    validation::CallSite::TransitionNamed { .. },
                ) => true,
                _ => false,
            }
        });
        let identity = InvocationIdentity::from_call_site(site, target)?;
        let occurrences = crate::checks::ownership_occurrences::statement_steps(
            program,
            machine,
            state,
            call.statement_index,
            node,
            &[],
            &facts.operators,
        );
        let mut completions = occurrences
            .iter()
            .enumerate()
            .filter(|(_, occurrence)| occurrence.invocation == Some(identity));
        let Some((completion, _)) = completions.next() else {
            return None;
        };
        if completions.next().is_some() {
            return None;
        }
        Some(Self {
            occurrences,
            completion,
            ends_source_path,
        })
    }

    /// A finished nested invocation releases its direct input window. Its
    /// stored descendants remain live and independently constrain later uses.
    /// A copied descriptor read is not such an invocation completion.
    pub(in crate::checks::borrows) fn access_finished_before_call(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        flow: &FlowStateFact,
        call: &BorrowCallFact,
        access_index: usize,
        access_count: usize,
        access: &BorrowArgumentAccessFact,
    ) -> bool {
        use crate::checks::ownership_occurrences::{InvocationIdentity, routes_compatible};
        if !access
            .end_before_access
            .is_some_and(|end| end > access_index && end <= access_count)
            || !access.released_at.is_valid()
            || access.released_at == access.expression
        {
            return false;
        }
        let prefix = &self.occurrences[..self.completion];
        let route = &self.occurrences[self.completion].route;
        if !access_released_in_prefix(access, prefix, route) {
            return false;
        }
        let mut completions = prefix.iter().filter(|occurrence| {
            occurrence.invocation == Some(InvocationIdentity::Expression(access.released_at))
                && routes_compatible(&occurrence.route, route)
        });
        if completions.next().is_none() || completions.next().is_some() {
            return false;
        }
        // Every real nested invocation receives its own ordinary borrow
        // judgment at that entry cut. Missing or ambiguous rosters cannot
        // justify omitting a later invocation's check.
        let Some(borrow_state) = super::super::matching_borrow_state(facts, flow) else {
            return false;
        };
        let Some(calls) = facts.borrow.calls.span(borrow_state.calls) else {
            return false;
        };
        let mut suppliers = calls.iter().filter(|candidate| {
            candidate.statement_index == call.statement_index
                && candidate.call_ordinal != call.call_ordinal
                && candidate.authored_expression == access.released_at
        });
        if suppliers.next().is_none() || suppliers.next().is_some() {
            return false;
        }
        validation::expression_type_reference_in_state(
            program,
            flow.state_symbol,
            call.statement_index,
            access.released_at,
        )
        .is_some_and(|reference| {
            validation::has_contents(
                program,
                reference,
                &[],
                validation::ContentsRequirement::BorrowFree,
            )
        })
    }
}

/// A staged access ends at its producer-owned completion, not when the next
/// accessed place happens to be observed. Explicit moves supply the prefix
/// before their occurrence; an implicit completion supplies the inclusive
/// prefix after completion. Both keep the same branch selection.
pub(in crate::checks::borrows) fn access_released_in_prefix(
    access: &BorrowArgumentAccessFact,
    prefix: &[crate::checks::ownership_occurrences::StatementOccurrence],
    route: &[crate::checks::ownership_occurrences::EvaluationBranch],
) -> bool {
    use crate::checks::ownership_occurrences::{StatementStep, routes_compatible};
    access.end_before_access == Some(0)
        || (access.released_at.is_valid()
            && prefix.iter().any(|occurrence| {
                matches!(occurrence.step, StatementStep::Complete(expression)
                    if expression == access.released_at)
                    && routes_compatible(&occurrence.route, route)
            }))
}

pub(super) fn copied_read_before_reference_transfer(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call: &BorrowCallFact,
    transferred: &BorrowArgumentAccessFact,
    copied: &BorrowArgumentAccessFact,
) -> bool {
    if super::super::statements::local_jump(program, state_flow, call).is_none()
        || transferred.kind != BorrowAccessKind::Mutable
        || transferred.end_before_access.is_some()
        || copied.kind != BorrowAccessKind::Read
        || copied.end_before_access.is_none()
        || transferred.root != copied.root
        || !facts
            .borrow
            .access_segments(copied)
            .starts_with(facts.borrow.access_segments(transferred))
    {
        return false;
    }
    // Reference values forward their established loan. A newly authored
    // borrow starts a child lifetime during argument evaluation instead.
    let mut expression = transferred.expression;
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Borrow(_) | ExpressionNode::Call(_) => return false,
            // Reference recasts form a new loan over backing storage. Other
            // casts preserve their operand's formation provenance; they must
            // not hide a borrow beneath a qualified result type.
            ExpressionNode::Cast(cast) => {
                if cast.form.is_recast() {
                    return false;
                }
                expression = cast.value;
            }
            _ => break,
        }
    }
    validation::expression_type_reference_in_state(
        program,
        state_flow.state_symbol,
        call.statement_index,
        expression,
    )
    .and_then(|reference| validation::reference_shell(program, reference))
    .is_some_and(|reference| reference.access == language_semantics::ReferenceAccess::Mutable)
}
