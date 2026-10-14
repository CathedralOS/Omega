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
        } = subject
        else {
            return None;
        };
        Some((*parameter_position, path, expected))
    }
}
