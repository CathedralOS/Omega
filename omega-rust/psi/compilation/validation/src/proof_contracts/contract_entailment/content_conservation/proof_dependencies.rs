//! Conditional content proofs retain their exact normalized theorem sources.
//!
//! These are transient checking results, not executable evidence or custody
//! receipts. Numeric equality has already grouped the observed endpoints into
//! current-value classes. A directed law edge remains usable only when every
//! checked theorem it cites is available. Admitted declarations have no checked
//! dependency. Reachability requires an actual law edge: class identity alone
//! cannot establish formation or an interval theorem.

use language_semantics::content::ContentConservationPlan;
use std::sync::Arc;
use typed_trees::expression::ExpressionHandle;

/// One completed-law implication between already-checked equality classes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationImplication {
    pub destination: usize,
    /// Complete normalized plans retain callable, owner, algebra, projection,
    /// place revisions and formal geometry; an owner symbol alone is insufficient.
    pub requirements: Arc<[ContentConservationPlan]>,
}

/// Alternatives within one return are graph paths, not an enumeration of
/// dependency combinations. Adjacency storage is linear in the law roster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentConservationDependencies {
    pub initial: usize,
    pub goal: usize,
    pub successors: Vec<Vec<ContentConservationImplication>>,
}

impl ContentConservationDependencies {
    /// Return the greatest source depth on one available proof path. The body
    /// publication owner adds its own derivation step after all returns check.
    /// An unavailable edge leaves other paths available; no first-candidate
    /// choice can hide an independently rooted alternative.
    pub fn derivation_depth(
        &self,
        mut source_depth: impl FnMut(&ContentConservationPlan) -> Option<u32>,
    ) -> Option<u32> {
        self.successors.get(self.initial)?;
        self.successors.get(self.goal)?;
        let mut reached = vec![false; self.successors.len()];
        let mut pending = vec![(self.initial, 0u32)];
        while let Some((current, depth)) = pending.pop() {
            for edge in self.successors.get(current)? {
                let Some(edge_depth) = edge
                    .requirements
                    .iter()
                    .try_fold(depth, |depth, requirement| {
                        source_depth(requirement).map(|next| depth.max(next))
                    })
                else {
                    continue;
                };
                let destination = reached.get_mut(edge.destination)?;
                // This check follows a law, even when both endpoints belong
                // to the same proven equality class.
                if edge.destination == self.goal {
                    return Some(edge_depth);
                }
                if !*destination {
                    *destination = true;
                    pending.push((edge.destination, edge_depth));
                }
            }
        }
        None
    }

    pub(crate) fn independently_proved(&self) -> bool {
        self.derivation_depth(|_| None).is_some()
    }
}

/// Every ordinary-return alternative must have an available proof. A normal
/// result declaration supplies no independently rooted content theorem merely
/// because its callable never completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConditionalContentGuarantee {
    pub expression: ExpressionHandle,
    pub plan: ContentConservationPlan,
    pub returns: Vec<ContentConservationDependencies>,
}
