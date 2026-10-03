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
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContextualCleanupDisposition {
    /// Stable source occurrence selected by the ownership producer. This is
    /// not a second permission ledger: finalized claims and provenance remain
    /// in flow ownership, including any conditional claim joins.
    pub machine: SymbolHandle,
    pub state: SymbolHandle,
    pub source: PermissionEventSource,
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
