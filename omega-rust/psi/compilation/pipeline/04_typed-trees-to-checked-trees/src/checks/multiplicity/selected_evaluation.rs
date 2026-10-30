//! Scratch custody follows the expression occurrence schedule. Each decision
//! keeps one entry and one completed sibling; reconvergence joins immediately,
//! so nested selections never enumerate combinations of execution paths.
//! Production and replay use the same routes, but replay consumes recorded
//! identities rather than minting replacement claims.

use crate::checks::ownership_occurrences::{
    EvaluationBranch, StatementStep, statement_steps_at_evaluation,
};
use checked_trees::{
    CheckedOperatorFacts, FlowCallFact, FlowPermissionEvaluation, FlowPermissionEventFact,
};
use diagnostics::Diagnostic;
use language_semantics::{Multiplicity, PermissionEventSource};
use typed_trees::expression::ExpressionHandle;

pub(super) mod place_bindings;

pub(super) struct ExpressionRoutes {
    completed: Vec<(ExpressionHandle, Vec<EvaluationBranch>)>,
}

impl ExpressionRoutes {
    pub(super) fn from_steps(
        occurrences: &[crate::checks::ownership_occurrences::StatementOccurrence],
    ) -> Self {
        let completed = occurrences
            .iter()
            .filter_map(|occurrence| match occurrence.step {
                StatementStep::Complete(expression) => Some((expression, occurrence.route.clone())),
                _ => None,
            })
            .collect();
        Self { completed }
    }

    pub(super) fn new(
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: usize,
        evaluation: FlowPermissionEvaluation,
        operators: &CheckedOperatorFacts,
    ) -> Self {
        let statement = &program.statement_table.statements(state.statement_nodes)[statement_index];
        let occurrences = statement_steps_at_evaluation(
            program,
            machine,
            state,
            statement_index,
            statement,
            &[],
            operators,
            Some(evaluation),
        );
        Self::from_steps(&occurrences)
    }

    pub(super) fn expression(&self, expression: ExpressionHandle) -> &[EvaluationBranch] {
        self.completed
            .iter()
            .find(|(candidate, _)| *candidate == expression)
            .map_or(&[], |(_, route)| route)
    }

    pub(super) fn event(
        &self,
        event: &FlowPermissionEventFact,
        calls: &[FlowCallFact],
    ) -> &[EvaluationBranch] {
        if event.expression.is_valid() {
            return self.expression(event.expression);
        }
        if let PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } = event.source
            && let Some(call) = calls.iter().find(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == target_symbol
            })
        {
            return self.expression(call.authored_expression);
        }
        &[]
    }
}

// Branch scratch belongs to the common occurrence schedule. Permission
// production and borrow replay use the same immediate reconvergence rule.
pub(super) use crate::checks::ownership_occurrences::SelectedEvaluation;

pub(super) fn join_places(
    current: &mut [super::linear_obligations::LinearPlace],
    sibling: &[super::linear_obligations::LinearPlace],
    mut bind: impl FnMut(
        &super::linear_obligations::LinearPlace,
        &super::linear_obligations::LinearPlace,
    ) -> Result<checked_trees::FlowSelectionPlaceArrival, Diagnostic>,
) -> Result<(), Diagnostic> {
    if current.len() != sibling.len() {
        return Err(Diagnostic::error(
            "expression selection has different storage rosters",
        ));
    }
    for (place, other) in current.iter_mut().zip(sibling) {
        if place.root != other.root
            || place.path != other.path
            || place.multiplicity != other.multiplicity
        {
            return Err(Diagnostic::error(
                "expression selection has incompatible typed places",
            ));
        }
        // A skipped transfer retains its exact debt in SelectedCleanup. It
        // is unavailable for ordinary use here, but must still prove an
        // owner-authorized disposition at death. A different claim or a
        // branch-born owner cannot inherit that pre-transfer custody.
        if place.multiplicity == Multiplicity::Linear
            && (place.live || other.live)
            && (place.ever_established != other.ever_established
                || place.claim_identity != other.claim_identity
                || place.provenance != other.provenance)
        {
            if !(place.live
                && other.live
                && place.ever_established
                && other.ever_established
                && !place.case_excluded
                && !other.case_excluded)
            {
                return Err(Diagnostic::error(format!(
                    "expression selection has incompatible live linear custody for `{}`",
                    place.name
                )));
            }
            // Only differing arrivals need a source-level name. Lowering
            // retains their separate namespaces at the unchanged storage
            // home; unaffected siblings keep their exact existing claims.
            let destination = bind(place, other)?;
            place.claim_identity = Some(destination.claim_identity);
            place.provenance = Some(destination.provenance);
        }
        place.live &= other.live;
        place.ever_established &= other.ever_established;
        place.case_excluded &= other.case_excluded;
    }
    Ok(())
}
