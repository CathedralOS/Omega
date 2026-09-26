//! Owned qualified schema-graph construction and independent replay.
//!
//! A [`SemanticSchemaGraph`] is the frozen, storage-independent description of
//! one selected data application: a root declaration node plus member nodes
//! for every field and case in authored declaration order, nominal-reference
//! edges for the type applications member types mention, and retired stable
//! numbers kept as descriptive history. Construction requires an explicit
//! [`SchemaQueryAuthority`]; replay re-derives every correspondence from the
//! typed trees so a forged or stale graph cannot stand in for the bound
//! declaration.
//!
//! This file owns the semantic schema graph. `schema_nodes.rs` carries
//! node handles, nodes, descriptions, shapes and query authorities,
//! `graph_construction.rs` constructs the graph from nominal expectations,
//! `graph_replay.rs` replays and checks a graph and `tests.rs` holds the
//! graph tests.

mod graph_construction;
mod graph_replay;
mod schema_nodes;
#[cfg(test)]
mod tests;

pub use graph_construction::construct_semantic_schema_graph;
pub(crate) use graph_construction::{describe_field, exact_symbol_identity, resolve_subject};
pub use graph_replay::replay_semantic_schema_graph;
pub(crate) use graph_replay::{
    check_authority_claim, check_subject_application, member_identity_or_name,
};
pub use schema_nodes::{
    CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
    SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape, TypeParameterDescription,
};

/// An owned, frozen semantic schema graph for one selected type application.
#[derive(Debug, Clone, PartialEq)]
pub struct SemanticSchemaGraph {
    /// Node 0 is `SchemaNode::Invalid`; `root` is node 1.
    nodes: Vec<SchemaNode>,
    root: SchemaNodeHandle,
    authority: SchemaQueryAuthority,
    /// Report-only FNV revision over the frozen contents. This value indexes
    /// nothing and authorizes nothing: replay reconstructs the exact
    /// correspondence instead of trusting the fingerprint (the repository's
    /// report-only-fingerprint rule).
    revision: u64,
}

impl SemanticSchemaGraph {
    pub fn root(&self) -> SchemaNodeHandle {
        self.root
    }

    pub fn authority(&self) -> &SchemaQueryAuthority {
        &self.authority
    }

    /// Report-only content fingerprint; `replay_semantic_schema_graph` is the
    /// authority, never this value.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Resolve a graph-local handle. Invalid and out-of-range handles borrow
    /// the dummy node so callers cannot alias storage outside the graph.
    pub fn node(&self, handle: SchemaNodeHandle) -> &SchemaNode {
        self.nodes
            .get(handle.index())
            .unwrap_or(&SchemaNode::Invalid)
    }

    /// The root declaration description.
    pub fn declaration(&self) -> &DeclarationDescription {
        let SchemaNode::Declaration(declaration) = self.node(self.root) else {
            unreachable!("the schema root is always a declaration node");
        };
        declaration
    }

    /// Borrowed record/common field descriptions in authored declaration
    /// order — the `schema.fields()` projection. Erased members are included:
    /// declaration inspection describes them even though runtime visitation
    /// will not borrow them.
    pub fn fields(&self) -> impl Iterator<Item = (SchemaNodeHandle, &FieldDescription)> {
        self.declaration()
            .members
            .iter()
            .copied()
            .filter_map(|handle| match self.node(handle) {
                SchemaNode::Field(field) => Some((handle, field)),
                _ => None,
            })
    }

    /// Borrowed case descriptions in authored declaration order — the
    /// `schema.cases()` projection. Case payload members borrow through
    /// `CaseDescription::payload_fields`.
    pub fn cases(&self) -> impl Iterator<Item = (SchemaNodeHandle, &CaseDescription)> {
        self.declaration()
            .members
            .iter()
            .copied()
            .filter_map(|handle| match self.node(handle) {
                SchemaNode::Case(case) => Some((handle, case)),
                _ => None,
            })
    }
}
