//! Anonymous projected owners die at their actual normal scope boundary.
//!
//! A branch-born call product cannot wait for statement-wide joined custody:
//! that meet correctly forgets its establishment on skipped paths. Nor can it
//! die at its first child move: later operands and held call arguments still
//! own their evaluation order. Close each producing scope immediately before
//! that scope's switch/join, after nested scopes have reconverged. Root-scope
//! owners keep the ordinary statement completion; call-held owners keep their
//! exact normal continuation. Cleanup still authenticates the full CURRENT
//! residual partition, not a remembered path or stale Move-time snapshot.

use crate::checks::multiplicity::cleanup_dispositions::RemainingCustody;
use crate::checks::multiplicity::linear_obligations::LinearPlace;
use crate::checks::multiplicity::selected_evaluation::place_bindings::SelectionSite;
use crate::checks::multiplicity::temporary_results::{self, TemporaryOwner};
use crate::checks::ownership_occurrences::{EvaluationBranch, StatementOccurrence, StatementStep};
use checked_trees::{ContextualCleanupFacts, FlowOwnershipFacts, FlowPermissionEventFact};
use diagnostics::Diagnostic;
use language_semantics::{PermissionEventKind, PermissionEventSource};
use typed_trees::expression::ExpressionHandle;

struct ProjectedOwner {
    expression: ExpressionHandle,
    completion: ExpressionHandle,
    path: Vec<facts::PlaceSegment>,
    scope: Option<EvaluationBranch>,
    establishment: usize,
}

#[derive(Default)]
pub(super) struct ProjectedScopes {
    owners: Vec<ProjectedOwner>,
}

impl ProjectedScopes {
    pub(super) fn observe_move(
        &mut self,
        occurrences: &[StatementOccurrence],
        held: &[TemporaryOwner],
        event: &crate::flow::DiscoveredMoveEvent,
        path: &[facts::PlaceSegment],
    ) -> Result<(), Diagnostic> {
        let Some(expression) = event.root.as_expression() else {
            return Ok(());
        };
        if path.is_empty()
            || held.iter().any(|owner| owner.expression == expression)
            || self
                .owners
                .iter()
                .any(|owner| owner.expression == expression)
        {
            return Ok(());
        }
        let mut producers = occurrences.iter().enumerate().filter(|(_, occurrence)| {
            matches!(occurrence.step, StatementStep::Complete(completed) if completed == expression)
        });
        let (Some((establishment, producer)), None) = (producers.next(), producers.next()) else {
            return Err(Diagnostic::error(
                "projected temporary has no unique evaluated producer completion",
            ));
        };
        self.owners.push(ProjectedOwner {
            expression,
            completion: event.expression,
            path: path.to_vec(),
            scope: producer.route.last().copied(),
            establishment,
        });
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn retire_scope(
        &mut self,
        program: &dyn validation::CheckingInput,
        site: SelectionSite,
        scope: Option<EvaluationBranch>,
        places: &mut [LinearPlace],
        remaining: &mut RemainingCustody,
        ownership: &mut FlowOwnershipFacts,
        permissions: &mut Vec<FlowPermissionEventFact>,
        dispositions: &mut ContextualCleanupFacts,
    ) -> Result<(), Diagnostic> {
        self.owners
            .sort_by_key(|owner| std::cmp::Reverse(owner.establishment));
        let mut position = 0;
        while position < self.owners.len() {
            if self.owners[position].scope != scope {
                position += 1;
                continue;
            }
            let owner = self.owners.remove(position);
            let root = facts::PlaceRoot::Expression(owner.expression);
            if remaining.is_present(root, &owner.path)
                || !remaining.is_present(root, &[])
                || temporary_results::projectable_owner(
                    program,
                    site.state,
                    site.statement,
                    owner.expression,
                )
                .is_none()
            {
                continue;
            }
            let producer = permissions
                .iter()
                .rev()
                .find(|event| {
                    event.kind == PermissionEventKind::Establish
                        && event.machine_symbol == site.machine
                        && event.state_symbol == site.state
                        && event.root == root
                })
                .map(|event| event.provenance)
                .ok_or_else(|| {
                    Diagnostic::error(
                        "projected temporary has no actual establishment at its normal scope exit",
                    )
                })?;
            let source = PermissionEventSource::Statement {
                statement_index: site.statement,
            };
            let first = permissions.len();
            temporary_results::retire_owner(
                program,
                ownership,
                site.machine,
                site.state,
                temporary_results::ProjectedConsumer {
                    source,
                    evaluation: site.evaluation,
                    producer,
                },
                owner.expression,
                &owner.path,
                places,
                remaining,
                permissions,
            )?;
            let disposition_start = dispositions.dispositions.len();
            remaining.observe_ordinary(
                program,
                &permissions[first..],
                &ownership.segments,
                dispositions,
                checked_trees::FlowClaimJoinExit::default(),
                &[],
                false,
            );
            remaining.retire_storage(root, site.machine, site.state, source, dispositions);
            // This source operation completed the projection. Keep that exact
            // lowering coordinate while the scope owner decides death order.
            let mut ordinal = 0;
            dispositions.dispositions.for_each_mut(|_, disposition| {
                if ordinal >= disposition_start {
                    disposition.completion = owner.completion;
                }
                ordinal += 1;
            });
        }
        Ok(())
    }
}
