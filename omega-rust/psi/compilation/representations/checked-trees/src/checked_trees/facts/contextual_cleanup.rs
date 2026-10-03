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
    /// Source-to-instance correspondence produced by machine specialization.
    /// These join selected occurrences; they do not grant hook prerequisites.
    pub applications: Arena<ContextualCleanupApplication>,
    pub exclusions: Arena<Place>,
    pub segments: Arena<PlaceSegment>,
    /// Bound generic obligations are entry inputs to source flow. Their
    /// storage survives refreshing the selected death occurrences.
    pub entry_requirements: ContextualCleanupEntryRequirements,
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

/// Instantiated cleanup prerequisites, distinct from authored `requires`.
/// The shared structural predicate path also names their exact invocation
/// namespace during Terminal publication.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupEntryRequirements {
    pub rows: Arena<ContextualCleanupEntryRequirement>,
    pub paths: Arena<crate::CheckedStructuralPredicatePathSegment>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextualCleanupEntryRequirement {
    pub machine: SymbolHandle,
    pub parameter_position: u32,
    pub path: HandleSpan<crate::CheckedStructuralPredicatePathSegment>,
    pub expected: bool,
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
            parameter_position: 0,
            path: HandleSpan::default(),
            expected: false,
            antecedent: crate::CheckedBooleanExpression::Constant(true),
            hook_machine: SymbolHandle::invalid(),
            hook_clause: Default::default(),
            template_machine: SymbolHandle::invalid(),
        }
    }
}

impl ContextualCleanupEntryRequirement {
    pub fn predicate(
        &self,
        paths: &Arena<crate::CheckedStructuralPredicatePathSegment>,
    ) -> crate::CheckedBooleanExpression {
        use crate::CheckedBooleanExpression;
        let field = CheckedBooleanExpression::StructuralParameterField {
            parameter_position: self.parameter_position,
            path: paths.span_or_empty(self.path).to_vec(),
        };
        let consequence = if self.expected { field } else { CheckedBooleanExpression::Not(Box::new(field)) };
        if self.antecedent == CheckedBooleanExpression::Constant(true) {
            consequence
        } else {
            CheckedBooleanExpression::Or {
                left: Box::new(CheckedBooleanExpression::Not(Box::new(self.antecedent.clone()))),
                right: Box::new(consequence),
            }
        }
    }
}
