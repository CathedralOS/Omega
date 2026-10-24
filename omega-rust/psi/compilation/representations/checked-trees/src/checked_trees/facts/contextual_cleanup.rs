//! Selected owned death edges, including the symbolic edges of generic bodies.
//!
//! The ownership producer records remaining custody once. Cleanup effects and
//! contextual obligations consume this same selection; an untouched generic
//! template retains its own rows beside each closed specialization. These rows
//! select disposition, not entry assumptions or invented authored requirements.
use arena::{Arena, Handle, HandleSpan, OwnedIndex};
use facts::{Place, PlaceSegment};

use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

#[derive(Clone, Default)]
pub struct ContextualCleanupStateIndex {
    by_state: symbols::SymbolKeyMap<
        (SymbolHandle, SymbolHandle),
        Vec<Handle<ContextualCleanupDisposition>>,
    >,
    selected: Vec<Handle<ContextualCleanupDisposition>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupFacts {
    pub dispositions: Arena<ContextualCleanupDisposition>,
    /// Receiving extents authenticated with the old occurrence's disposition.
    /// Claim events may name individual leaves of this containing write.
    pub storage_establishments: Arena<ContextualStorageEstablishment>,
    /// Exact candidate deaths for owners transferred on selected evaluation
    /// paths. These share ordinary hook admission, but are not unconditional
    /// permission events. Lowering must retain the selected live frontier.
    pub selected_dispositions: Arena<Handle<ContextualCleanupDisposition>>,
    /// Source-to-instance correspondence produced by machine specialization.
    /// These join selected occurrences; they do not grant hook prerequisites.
    pub applications: Arena<ContextualCleanupApplication>,
    pub exclusions: Arena<Place>,
    pub segments: Arena<PlaceSegment>,
    /// Open source windows at exact evaluated invocations. Their storage is
    /// selected with deaths; invocation effects are checked only after the
    /// selected cleanup dependencies complete those effects.
    pub window_invocations: Arena<StorageWindowInvocation>,
    /// Bound generic obligations are entry inputs to source flow. Their
    /// storage survives refreshing the selected death occurrences.
    pub entry_requirements: ContextualCleanupEntryRequirements,
    /// `(machine, state)` and selected-disposition lookup owned by this ledger.
    pub state_index: OwnedIndex<ContextualCleanupStateIndex, (u64, u64)>,
}

impl ContextualCleanupFacts {
    fn with_state_index<R>(&self, run: impl FnOnce(&ContextualCleanupStateIndex) -> R) -> R {
        let revision = (
            self.dispositions.revision(),
            self.selected_dispositions.revision(),
        );
        self.state_index.with(
            revision,
            || {
                let mut by_state = symbols::SymbolKeyMap::<
                    (SymbolHandle, SymbolHandle),
                    Vec<Handle<ContextualCleanupDisposition>>,
                >::default();
                for (handle, disposition) in self.dispositions.iter() {
                    by_state
                        .entry((disposition.machine, disposition.state))
                        .or_default()
                        .push(handle);
                }
                // Selection names exact occurrences, including their generations.
                // Active row count is neither a slot bound nor a handle identity.
                let mut selected: Vec<_> = self
                    .selected_dispositions
                    .iter()
                    .map(|(_, candidate)| *candidate)
                    .collect();
                selected.sort_unstable_by_key(|handle| (handle.arena_index(), handle.generation()));
                selected.dedup();
                ContextualCleanupStateIndex { by_state, selected }
            },
            run,
        )
    }

    pub fn is_selected_disposition(&self, handle: Handle<ContextualCleanupDisposition>) -> bool {
        self.with_state_index(|index| {
            index
                .selected
                .binary_search_by_key(&(handle.arena_index(), handle.generation()), |candidate| {
                    (candidate.arena_index(), candidate.generation())
                })
                .is_ok()
        })
    }

    pub fn for_each_state_disposition(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        mut visit: impl FnMut(Handle<ContextualCleanupDisposition>, &ContextualCleanupDisposition),
    ) {
        self.with_state_index(|index| {
            if let Some(handles) = index.by_state.get(&(machine, state)) {
                for &handle in handles {
                    visit(handle, self.dispositions.get(handle));
                }
            }
        });
    }

    pub fn any_state_disposition(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
        mut predicate: impl FnMut(
            Handle<ContextualCleanupDisposition>,
            &ContextualCleanupDisposition,
        ) -> bool,
    ) -> bool {
        self.with_state_index(|index| {
            index
                .by_state
                .get(&(machine, state))
                .is_some_and(|handles| {
                    handles
                        .iter()
                        .any(|&handle| predicate(handle, self.dispositions.get(handle)))
                })
        })
    }

    pub fn state_disposition_handles(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Vec<Handle<ContextualCleanupDisposition>> {
        self.with_state_index(|index| {
            index
                .by_state
                .get(&(machine, state))
                .cloned()
                .unwrap_or_default()
        })
    }
}

/// One successful receiving write, distinct from the claims it establishes.
/// Its destination path belongs to `ContextualCleanupFacts::segments`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualStorageEstablishment {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub source: PermissionEventSource,
    pub destination: Place,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StorageWindowInvocation {
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub statement: usize,
    pub expression: typed_trees::expression::ExpressionHandle,
    /// Canonical absent storage; segments belong to this family's arena.
    pub absent: Place,
    /// Match-arm windows have no statement-level extraction occurrence.
    pub opened_statement: Option<usize>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupDisposition {
    /// Conditional complement of this exact owned value join. The source
    /// occurrence proves its ordinary cleanup; receiving storage is selected
    /// by the retained join, never by a still-available lexical alias.
    pub selection: Handle<crate::FlowOwnedSelectionReceipt>,
    /// Stable source occurrence selected by the ownership producer. This is
    /// not a second permission ledger: finalized claims and provenance remain
    /// in flow ownership, including any conditional claim joins.
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub source: PermissionEventSource,
    /// Exact selected normal-exit occurrence. An invalid state means this row
    /// is not an exit, or the custody producer cannot distinguish its exits;
    /// it never authorizes borrowing facts from every exit of the state.
    pub exit: crate::FlowClaimJoinExit,
    /// The value completion owning this death, when it occurs inside an
    /// evaluation. Zero retains the statement, call or exit boundary. This
    /// identity keeps branch-local cleanup with its actual produced value.
    pub completion: typed_trees::expression::ExpressionHandle,
    pub root: facts::PlaceRoot,
    /// A displaced borrowed referent owns the stored value's type, not its
    /// access route's reference carrier. Zero retains the ordinary owned-place
    /// type query. The source window producer supplies this only at replacement.
    pub disposed_type: typed_trees::types::TypeReferenceHandle,
    /// The selected path uses the flow ownership segment arena. Excluded
    /// subtrees below use this family's segment arena instead.
    pub path: HandleSpan<PlaceSegment>,
    pub exclusions: HandleSpan<Place>,
    /// Alternatives proved inactive at this occurrence. Unlike transferred
    /// subtrees, these do not make an intact nominal owner partially moved.
    /// Paths use the same place/segment arenas as exclusions.
    pub inactive_cases: HandleSpan<Place>,
    /// The death belongs to a root still under field-wise construction at
    /// this edge, so the ownership producer already ordered this root's
    /// deaths in reverse establishment order. Consumers keep that order; a
    /// completed aggregate or an ordinary root dies in declared cleanup
    /// order instead (ownership.md, construction and disposal order).
    pub establishment_ordered: bool,
    /// The dying storage is a linear local's zero image: zero-filled bits
    /// that established no value and owe no obligation (ownership.md,
    /// "Establishment and erasure"). Its retirement is the whole structural
    /// discard; no hook is selected and a hook-less linear type is not an
    /// undisposable value here.
    pub zero_image: bool,
}

/// The same lexical death edge in a retained template and one exact instance.
/// Both handles belong to `ContextualCleanupFacts::dispositions`; refresh them
/// with that arena rather than carrying scratch handles across flow sweeps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupApplication {
    pub template: Handle<ContextualCleanupDisposition>,
    pub instance: Handle<ContextualCleanupDisposition>,
}

/// Instantiated cleanup prerequisites, distinct from authored `requires`.
/// The shared checked predicate vocabulary names every observation in its
/// exact invocation namespace during source flow and Terminal publication.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupEntryRequirements {
    pub rows: Arena<ContextualCleanupEntryRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextualCleanupEntryRequirement {
    pub machine: SymbolHandle,
    /// Complete callable clause in the invocation's checked scalar namespace.
    /// Every observation is rebased together; retaining one selected field
    /// would lose conjunctions, comparisons and their relational meaning.
    pub consequence: crate::CheckedBooleanExpression,
    /// Invocation-entry condition selecting this cleanup obligation.
    pub antecedent: crate::CheckedBooleanExpression,
    /// Authored hook clause supplying this proposition and diagnostic origin.
    pub hook_machine: SymbolHandle,
    pub hook_clause: typed_trees::expression::ExpressionHandle,
    /// Generic source body whose symbolic death first introduced the row.
    pub template_machine: SymbolHandle,
}

impl Default for ContextualCleanupEntryRequirement {
    fn default() -> Self {
        Self {
            machine: SymbolHandle::invalid(),
            consequence: crate::CheckedBooleanExpression::Constant(false),
            antecedent: crate::CheckedBooleanExpression::Constant(true),
            hook_machine: SymbolHandle::invalid(),
            hook_clause: Default::default(),
            template_machine: SymbolHandle::invalid(),
        }
    }
}

impl ContextualCleanupEntryRequirement {
    /// One complete invocation obligation. Neither source flow nor Terminal
    /// publication may assume the consequence without its selected-edge guard.
    pub fn predicate(&self) -> crate::CheckedBooleanExpression {
        use crate::CheckedBooleanExpression;
        if self.antecedent == CheckedBooleanExpression::Constant(true) {
            self.consequence.clone()
        } else {
            CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Not(Box::new(
                    self.antecedent.clone(),
                ))),
                right: Box::new(self.consequence.clone()),
            }
        }
    }

    /// A bounded Boolean consumer can project its exact supported proposition
    /// from the complete row. This is not an alternative storage or inference
    /// route, and a wider formula cannot masquerade as one field assertion.
    pub fn direct_boolean_subject(
        &self,
    ) -> Option<(u32, &[crate::CheckedStructuralPredicatePathSegment], bool)> {
        use crate::CheckedBooleanExpression;
        let (subject, expected) = match &self.consequence {
            CheckedBooleanExpression::Not(subject) => (subject.as_ref(), false),
            subject => (subject, true),
        };
        let CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } = subject
        else {
            return None;
        };
        Some((*parameter_position, path, expected))
    }
}

#[cfg(test)]
mod index_tests {
    use super::{ContextualCleanupDisposition, ContextualCleanupFacts};

    #[test]
    fn selected_membership_preserves_full_handles_and_sparse_rosters() {
        let mut facts = ContextualCleanupFacts::default();
        let first = facts
            .dispositions
            .insert(ContextualCleanupDisposition::default());
        facts.selected_dispositions.insert(first);
        assert!(facts.is_selected_disposition(first));
        let last = facts
            .dispositions
            .insert(ContextualCleanupDisposition::default());
        facts.selected_dispositions.insert(last);
        assert!(facts.is_selected_disposition(last));
        assert!(facts.dispositions.free(first));
        assert!(facts.is_selected_disposition(last));
        let replacement = facts
            .dispositions
            .insert(ContextualCleanupDisposition::default());
        assert_eq!(first.arena_index(), replacement.arena_index());
        assert_ne!(first.generation(), replacement.generation());
        assert!(!facts.is_selected_disposition(replacement));
        facts.selected_dispositions.insert(replacement);
        assert!(facts.is_selected_disposition(replacement));
    }
}
