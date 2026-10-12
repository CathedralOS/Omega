//! Selected owned death edges, including the symbolic edges of generic bodies.
//!
//! The ownership producer records remaining custody once. Cleanup effects and
//! contextual obligations consume this same selection; an untouched generic
//! template retains its own rows beside each closed specialization. These rows
//! select disposition, not entry assumptions or invented authored requirements.
use arena::{Arena, Handle, HandleSpan};
use facts::{Place, PlaceSegment};

use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupFacts {
    pub dispositions: Arena<ContextualCleanupDisposition>,
    /// Exact candidate deaths for owners transferred on selected evaluation
    /// paths. These share ordinary hook admission, but are not unconditional
    /// permission events. Lowering must retain the selected live frontier.
    pub selected_dispositions: Arena<Handle<ContextualCleanupDisposition>>,
    /// Source-to-instance correspondence produced by machine specialization.
    /// These join selected occurrences; they do not grant hook prerequisites.
    pub applications: Arena<ContextualCleanupApplication>,
    /// Actual selected hook effects in the caller storage/alias namespace.
    /// These rows retain the original disposition and exact captured receiver;
    /// their absence or incomplete frame never establishes preservation.
    pub hook_effects: Arena<ContextualCleanupHookEffect>,
    pub hook_write_places: Arena<Place>,
    pub hook_write_segments: Arena<PlaceSegment>,
    pub exclusions: Arena<Place>,
    pub segments: Arena<PlaceSegment>,
    /// Open source windows at exact evaluated invocations. Their storage is
    /// selected with deaths; invocation effects are checked only after the
    /// selected cleanup dependencies complete those effects.
    pub window_invocations: Arena<StorageWindowInvocation>,
    /// Bound generic obligations are entry inputs to source flow. Their
    /// storage survives refreshing the selected death occurrences.
    pub entry_requirements: ContextualCleanupEntryRequirements,
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
    pub root: facts::PlaceRoot,
    /// A displaced borrowed referent owns the stored value's type, not its
    /// access route's reference carrier. Zero retains the ordinary owned-place
    /// type query. The source window producer supplies this only at replacement.
    pub disposed_type: typed_trees::types::TypeReferenceHandle,
    /// The selected path uses the flow ownership segment arena. Excluded
    /// subtrees below use this family's segment arena instead.
    pub path: HandleSpan<PlaceSegment>,
    pub exclusions: HandleSpan<Place>,
}

/// The same lexical death edge in a retained template and one exact instance.
/// Both handles belong to `ContextualCleanupFacts::dispositions`; refresh them
/// with that arena rather than carrying scratch handles across flow sweeps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupApplication {
    pub template: Handle<ContextualCleanupDisposition>,
    pub instance: Handle<ContextualCleanupDisposition>,
}

/// One issued hook footprint on its original selected death occurrence.
/// Complete empty writes are genuine producer output; incomplete writes have
/// no preservation authority. Receiver and writes use this family's dedicated
/// hook_write_segments arena, independently of ownership/exclusion segments.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupHookEffect {
    pub disposition: Handle<ContextualCleanupDisposition>,
    pub hook_machine: SymbolHandle,
    pub receiver: Place,
    pub complete: bool,
    pub writes: HandleSpan<Place>,
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
        } = subject
        else {
            return None;
        };
        Some((*parameter_position, path, expected))
    }
}

impl ContextualCleanupFacts {
    /// Select only a footprint issued for this same disposition, hook and
    /// exact captured receiver. A duplicate or unresolved association stays
    /// absent; complete/opaque remains on the selected row itself.
    pub fn hook_effect(
        &self,
        disposition: Handle<ContextualCleanupDisposition>,
        hook: SymbolHandle,
        root: facts::PlaceRoot,
        path: &[PlaceSegment],
    ) -> Handle<ContextualCleanupHookEffect> {
        let mut matching = self.hook_effects.iter().filter(|(_, effect)| {
            effect.disposition == disposition
                && effect.hook_machine == hook
                && effect.receiver.root == root
                && self.hook_write_segments.span(effect.receiver.segments) == Some(path)
        });
        let Some((handle, _)) = matching.next() else {
            return Default::default();
        };
        if matching.next().is_some() {
            return Default::default();
        }
        handle
    }
}
