//! Semantic reflection schema graphs (wiki/spec/language/reflection.md).
//!
//! `reflect::schema<T>()` produces an OWNED `TypeSchema` description graph for
//! one explicitly selected, authorized type application. This module owns the
//! target-neutral content of that graph before any value materialization:
//! nodes describe declarations, fields, cases, and their relationships, and
//! every handle indexes only its owning graph — never compiler/evaluator
//! storage. Member and owner identities are recorded as canonical text
//! (`normalized_hermetic_symbol_identity` where package provenance exists,
//! the fully qualified display path otherwise), so a frozen graph remains
//! checkable after the producing arenas are gone.
//!
//! Two deliberately separated halves live here:
//!
//! - `construct_semantic_schema_graph` is the PRODUCER. It walks the typed
//!   declaration once under an explicit `SchemaQueryAuthority` and freezes the
//!   result. The authority is context data, not ambient permission: a foreign
//!   scope requires the subject to be public, because a query may not hide
//!   inaccessible members while claiming complete coverage.
//! - `replay_semantic_schema_graph` is the INDEPENDENT CHECK. It re-resolves
//!   the bound declaration from the recorded owner identity and recomputes
//!   every member, type, case-ownership, and retired-identity correspondence
//!   from the typed trees instead of trusting the producer's walk. A forged,
//!   stale, or corrupted graph rejects; the stored revision is report-only
//!   beside that exact replay (matching the repository's fingerprint rule).
//!
//! - `selection` is the TYPED SELECTION half on top of the graph. A
//!   `ScopedSelectionReceiver` projects a scoped subset of the authorized
//!   graph's members under a required contract; `freeze` seals the completed
//!   choices into an owned `SelectionSnapshot` with a canonical fingerprint;
//!   `replay_selection_snapshot` re-binds the subject and re-checks every
//!   member/type/requirement correspondence from the typed trees.
//!
//! `reflect::visit_runtime_fields` per-member calls and recursive derivations
//! are later slices on the same contract; none of them are required to read
//! or extend these graph and snapshot shapes.

mod schema_graph;
mod selection;

pub use schema_graph::{
    CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
    SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph,
    TypeParameterDescription, construct_semantic_schema_graph, replay_semantic_schema_graph,
};
pub use selection::{
    MemberSelectionKey, ScopedSelectionReceiver, SelectionChoice, SelectionCoverage,
    SelectionProjection, SelectionRecord, SelectionRequirement, SelectionSnapshot,
    replay_selection_snapshot,
};
