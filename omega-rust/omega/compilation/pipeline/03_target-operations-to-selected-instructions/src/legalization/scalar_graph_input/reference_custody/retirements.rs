//! Descriptor disposal ends residency, not a descendant's backing obligation.
//! Keep executed disposal and ancestry until the last descendant ends. Retired
//! occurrences never participate in readable storage lookup or grant authority;
//! they connect the surviving cohort to the exact original parent occurrence.

use super::{Custody, Leaf, LegalizationError, OccurrenceIdentity, OperationId, invalid};
use semantic_vocabulary::{EdgeId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralPathSegment;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum DisposalEvent {
    Operation(OperationId),
    Edge(EdgeId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RetiredOccurrence {
    pub(super) leaf: Leaf,
    pub(super) disposals: Vec<DisposalEvent>,
}

impl Custody {
    pub(super) fn retire(
        &mut self,
        leaf: Leaf,
        event: DisposalEvent,
    ) -> Result<(), LegalizationError> {
        if self.retired.contains_key(&leaf.identity) {
            return Err(invalid());
        }
        self.retired.insert(
            leaf.identity.clone(),
            RetiredOccurrence {
                leaf,
                disposals: vec![event],
            },
        );
        Ok(())
    }

    /// Follow original immediate links across disposed carriers. An active
    /// intermediate remains an ordinary live suspension in its own right.
    pub(super) fn descends_from(&self, child: &Leaf, ancestor: &OccurrenceIdentity) -> bool {
        let mut pending = child.parents.iter().collect::<Vec<_>>();
        let mut visited = BTreeSet::new();
        while let Some(parent) = pending.pop() {
            if parent == ancestor {
                return true;
            }
            if visited.insert(parent)
                && let Some(retired) = self.retired.get(parent)
            {
                pending.extend(&retired.leaf.parents);
            }
        }
        false
    }

    /// A disposed ancestor matters only while reachable from a resident leaf.
    /// Do not preserve arbitrary history through loops or manufacture a parent
    /// when a source graph omitted it; completion still diagnoses missing links.
    pub(super) fn prune_retired(&mut self) {
        let mut pending = self
            .leaves
            .values()
            .flat_map(|leaf| leaf.obligations())
            .collect::<Vec<_>>();
        let mut retained = BTreeSet::new();
        while let Some(identity) = pending.pop() {
            if retained.insert(identity.clone())
                && let Some(retired) = self.retired.get(identity)
            {
                pending.extend(retired.leaf.obligations());
            }
        }
        self.retired
            .retain(|identity, _| retained.contains(identity));
    }

    pub(super) fn validate_occurrences(&self) -> Result<(), LegalizationError> {
        let mut identities = BTreeMap::<_, (&(PlaceId, Vec<StructuralPathSegment>), &Leaf)>::new();
        for (location, leaf) in &self.leaves {
            let duplicate = identities
                .insert(&leaf.identity, (location, leaf))
                .is_some_and(|(previous, previous_leaf)| {
                    previous_leaf != leaf || !alternative_residency(previous, location)
                });
            if duplicate || self.retired.contains_key(&leaf.identity) {
                return Err(LegalizationError::ReferenceCompletion {
                    reason: terminal_semantics::ReferenceCompletionError::DuplicateOccurrence,
                });
            }
        }
        for leaf in self
            .leaves
            .values()
            .chain(self.retired.values().map(|retired| &retired.leaf))
        {
            if leaf.obligations().any(|parent| {
                !identities.contains_key(parent) && !self.retired.contains_key(parent)
            }) {
                return Err(LegalizationError::ReferenceCompletion {
                    reason: terminal_semantics::ReferenceCompletionError::MissingParent,
                });
            }
        }
        Ok(())
    }
}

/// A sum carrier holds one active case, so the same occurrence resident below
/// two different cases of one carrier is a single residency with alternative
/// layouts, not two live loans. The leaves must agree on their whole contract.
pub(super) fn alternative_residency(
    left: &(PlaceId, Vec<StructuralPathSegment>),
    right: &(PlaceId, Vec<StructuralPathSegment>),
) -> bool {
    left.0 == right.0
        && left
            .1
            .iter()
            .zip(&right.1)
            .find(|(left, right)| left != right)
            .is_some_and(|pair| {
                matches!(
                    pair,
                    (
                        StructuralPathSegment::Case(_),
                        StructuralPathSegment::Case(_)
                    )
                )
            })
}

/// Retained parent occurrences are history, not selected live values. Their
/// complete leaf contract must agree; only actual alternative disposal sites
/// may union. A mismatched provisional arrival must converge before replay.
pub(super) fn merge(
    left: &mut BTreeMap<OccurrenceIdentity, RetiredOccurrence>,
    right: &BTreeMap<OccurrenceIdentity, RetiredOccurrence>,
) -> Option<()> {
    for (identity, incoming) in right {
        let Some(existing) = left.get_mut(identity) else {
            left.insert(identity.clone(), incoming.clone());
            continue;
        };
        if existing.leaf != incoming.leaf {
            return None;
        }
        existing.disposals.extend_from_slice(&incoming.disposals);
        existing.disposals.sort();
        existing.disposals.dedup();
    }
    Some(())
}
