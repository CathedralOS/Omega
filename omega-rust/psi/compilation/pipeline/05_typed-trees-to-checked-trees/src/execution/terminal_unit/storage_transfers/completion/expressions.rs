//! Normal alternatives retain their own invocation and the predicates that
//! precede them. An aborted alternative cannot justify normal cleanup.

use super::{Query, Transfers};
use checked_trees::{
    CheckedUnitCompletionExpression, CheckedUnitCompletionKind, CheckedUnitCompletionPlan,
};

impl Query<'_> {
    pub(super) fn completion_expression(
        &self,
        plan: &CheckedUnitCompletionPlan,
    ) -> Result<Transfers, &'static str> {
        self.completion_node(&plan.expression)?
            .ok_or("completion cleanup: expression has no normal transfer path")
    }

    fn completion_node(
        &self,
        node: &CheckedUnitCompletionExpression,
    ) -> Result<Option<Transfers>, &'static str> {
        if !node.normal_continuation {
            return Ok(None);
        }
        if let CheckedUnitCompletionKind::Invocation(operations) = &node.kind {
            return self.operation_sequence(operations.iter()).map(Some);
        }
        let subject = node
            .subject()
            .ok_or("completion cleanup: selected subject is missing")?;
        let mut scope = self.clone();
        let mut subject_transfers = scope
            .dispatch_subject(&subject)
            .ok_or("completion cleanup: selected subject has incomplete transfers")?;
        match subject {
            checked_trees::CheckedDispatchSubject::Scalar(value) => {
                scope.completed_computations.push(value)
            }
            checked_trees::CheckedDispatchSubject::Structural { value, .. } if value.is_valid() => {
                scope.established_values.push(value)
            }
            checked_trees::CheckedDispatchSubject::Structural { .. } => {}
        }
        let mut prefixes = Vec::new();
        for (pattern, _) in node.arms() {
            let mut transfers = Transfers::default();
            if let Some(computation) = pattern {
                transfers = scope
                    .computation(computation)
                    .ok_or("completion cleanup: selected pattern has incomplete transfers")?;
                scope.completed_computations.push(computation);
            }
            prefixes.push((transfers, scope.completed_computations.len()));
        }
        let mut remaining = None;
        let mut possible = Vec::new();
        for ((_, child), (mut prefix, completed)) in
            node.arms().rev().zip(prefixes.into_iter().rev())
        {
            scope.completed_computations.truncate(completed);
            let mut child = scope.completion_node(child)?;
            // These are named possible destinations, not execution events.
            // Do not copy them repeatedly while joining definite paths.
            if let Some(child) = &mut child {
                possible.append(&mut child.possible);
            }
            let joined = match (child, remaining) {
                (Some(child), Some(fallback)) => {
                    Some(Transfers::alternatives(vec![child, fallback]))
                }
                (child, fallback) => child.or(fallback),
            };
            remaining = if let Some(joined) = joined {
                possible.append(&mut prefix.possible);
                prefix.append(joined);
                Some(prefix)
            } else {
                None
            };
        }
        let remaining =
            remaining.ok_or("completion cleanup: normal dispatch has no normal alternative")?;
        possible.append(&mut subject_transfers.possible);
        subject_transfers.append(remaining);
        subject_transfers.possible = possible;
        Ok(Some(subject_transfers))
    }
}
