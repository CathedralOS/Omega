//! Member selection keys, scoped members and expected members.

use crate::build_time_evaluation::machine_execution::reflection::schema_graph::{
    FieldDescription, SchemaNode, SchemaNodeHandle, SemanticSchemaGraph, describe_field,
    member_identity_or_name,
};
use crate::build_time_evaluation::machine_execution::reflection::selection::{
    SelectionChoice, SelectionProjection, SelectionRecord,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::data::{DataDefinition, DataMember};

/// An exact member key minted by a receiver's projection. A policy cannot
/// construct keys; it resolves names or identities into them, so the recorded
/// selection retains the resolved member. The pinned type and scope travel in
/// the key so a key minted against a stale member pair rejects at selection
/// instead of projecting onto a drifted member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberSelectionKey {
    pub(crate) member_identity: String,
    pub(crate) qualified_type: Option<String>,
    owner_case_identity: Option<String>,
    erased: bool,
}

impl MemberSelectionKey {
    pub fn member_identity(&self) -> &str {
        &self.member_identity
    }

    /// The member's complete qualified type; `None` for case members.
    pub fn qualified_type(&self) -> Option<&str> {
        self.qualified_type.as_deref()
    }

    /// Erased members are described but receive no runtime borrow.
    pub fn is_erased(&self) -> bool {
        self.erased
    }
}

/// The member's role in the declaring type's shape — the distinction a
/// visitation callback reads on its `FieldInfo` argument to tell an
/// active-case report from a field borrow. Cases carry the enclosing sum as
/// their nominal subject; payloads carry their owning case's identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    /// A record or common field.
    Field,
    /// A sum case, visited through the active-case callback.
    Case,
    /// A case payload member.
    PayloadField,
}

/// One in-scope member of a projection, derived from the authorized schema
/// graph by the receiver and re-derived from the typed trees by replay. The
/// two derivations must agree; a snapshot replaying cleanly is the witness.
///
/// `pub(super)` so the visitation slice reuses this exact member record —
/// its `FieldInfo` contexts and carrier checks must not drift from what a
/// selection selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScopedMember {
    pub(crate) member_identity: String,
    pub(crate) name: String,
    pub(crate) kind: MemberKind,
    /// Authored declaration order within the member's owning scope (the
    /// top-level sequence for record fields and cases, the case's payload
    /// sequence for payload members).
    pub(crate) declaration_position: u32,
    /// Optional authored stable number.
    pub(crate) stable_number: Option<u64>,
    pub(crate) qualified_type: Option<String>,
    pub(crate) owner_case_identity: Option<String>,
    pub(crate) erased: bool,
    /// First nominal owner identity in the member type's discovery order —
    /// the carrier a selected operation must cover. A self-edge resolves to
    /// the subject's own declaration; a case member's carrier is the
    /// enclosing sum itself. `None` for member types with no nominal
    /// subject, which admit only exclusions.
    pub(crate) head_nominal_identity: Option<String>,
}

impl ScopedMember {
    pub(crate) fn key(&self) -> MemberSelectionKey {
        MemberSelectionKey {
            member_identity: self.member_identity.clone(),
            qualified_type: self.qualified_type.clone(),
            owner_case_identity: self.owner_case_identity.clone(),
            erased: self.erased,
        }
    }

    /// Whether a key names exactly this member. The whole key is the match:
    /// member identities can repeat across distinct case scopes (two cases
    /// may each declare a `value` payload), so identity alone is not a
    /// unique handle.
    pub(crate) fn matches_key(&self, key: &MemberSelectionKey) -> bool {
        self.member_identity == key.member_identity
            && self.qualified_type == key.qualified_type
            && self.owner_case_identity == key.owner_case_identity
            && self.erased == key.erased
    }

    pub(crate) fn record(&self, choice: SelectionChoice) -> SelectionRecord {
        SelectionRecord {
            member_identity: self.member_identity.clone(),
            qualified_type: self.qualified_type.clone(),
            owner_case_identity: self.owner_case_identity.clone(),
            erased: self.erased,
            choice,
        }
    }
}

/// Whether a frozen record names exactly this member — the full-key match
/// replay uses when re-binding records to re-derived members.
pub(crate) fn record_matches(record: &SelectionRecord, member: &ScopedMember) -> bool {
    record.member_identity == member.member_identity
        && record.qualified_type == member.qualified_type
        && record.owner_case_identity == member.owner_case_identity
        && record.erased == member.erased
}

/// Apply the frozen projection to a candidate member set. Shared by the
/// receiver (graph-derived members) and replay (tree-derived members) so the
/// two sides cannot drift on what "in scope" means.
pub(crate) fn project(
    members: &[ScopedMember],
    projection: &SelectionProjection,
) -> Result<Vec<ScopedMember>, String> {
    match projection {
        SelectionProjection::DeclaredMembers => Ok(members.to_vec()),
        SelectionProjection::RuntimeMembers => Ok(members
            .iter()
            .filter(|member| !member.erased)
            .cloned()
            .collect()),
        SelectionProjection::Members(identities) => {
            let mut scoped = Vec::with_capacity(identities.len());
            let mut seen: Vec<&str> = Vec::with_capacity(identities.len());
            for identity in identities {
                if seen.contains(&identity.as_str()) {
                    return Err(format!(
                        "selection projection lists member `{identity}` twice"
                    ));
                }
                seen.push(identity.as_str());
                let mut matches = members
                    .iter()
                    .filter(|member| member.member_identity == *identity);
                match (matches.next(), matches.next()) {
                    (Some(member), None) => scoped.push(member.clone()),
                    (Some(_), Some(_)) => {
                        return Err(format!(
                            "selection projection member `{identity}` is ambiguous in the bound schema"
                        ));
                    }
                    _ => {
                        return Err(format!(
                            "selection projection member `{identity}` is not a member of the bound schema"
                        ));
                    }
                }
            }
            Ok(scoped)
        }
    }
}

/// Derive the in-scope-checkable member set from the authorized schema graph,
/// in authored order: each field, each case, then each case's payload
/// members. This is the producer walk; replay's `expected_members` is its
/// independent tree-derived counterpart.
pub(crate) fn graph_members(graph: &SemanticSchemaGraph) -> Result<Vec<ScopedMember>, String> {
    let declaration = graph.declaration();
    let mut members = Vec::new();
    for handle in &declaration.members {
        match graph.node(*handle) {
            SchemaNode::Field(field) => {
                members.push(graph_field_member(graph, field, MemberKind::Field, None)?)
            }
            SchemaNode::Case(case) => {
                members.push(ScopedMember {
                    member_identity: case.member_identity.clone(),
                    name: case.name.clone(),
                    kind: MemberKind::Case,
                    declaration_position: case.declaration_position,
                    stable_number: case.stable_number,
                    qualified_type: None,
                    owner_case_identity: None,
                    erased: false,
                    head_nominal_identity: Some(declaration.owner_identity.clone()),
                });
                for payload in &case.payload_fields {
                    let SchemaNode::Field(field) = graph.node(*payload) else {
                        return Err(format!(
                            "schema `{}` case `{}` payload is not a field node",
                            declaration.name, case.name
                        ));
                    };
                    members.push(graph_field_member(
                        graph,
                        field,
                        MemberKind::PayloadField,
                        Some(case.member_identity.clone()),
                    )?);
                }
            }
            _ => {}
        }
    }
    Ok(members)
}

fn graph_field_member(
    graph: &SemanticSchemaGraph,
    field: &FieldDescription,
    kind: MemberKind,
    owner_case_identity: Option<String>,
) -> Result<ScopedMember, String> {
    // The head nominal in discovery order is the carrier a selected
    // operation must cover: the declaration for a named/generic head, the
    // element for arrays and slices, the referee for references, the base
    // for constrained types — and the subject itself for a self-edge.
    let head_nominal_identity = match field.nominal_references.first() {
        None => None,
        Some(handle) if *handle == graph.root() => Some(graph.declaration().owner_identity.clone()),
        Some(handle) => match graph.node(*handle) {
            SchemaNode::NominalReference(reference) => Some(reference.owner_identity.clone()),
            _ => {
                return Err(format!(
                    "schema member `{}` nominal edge is not a reference node",
                    field.name
                ));
            }
        },
    };
    Ok(ScopedMember {
        member_identity: field.member_identity.clone(),
        name: field.name.clone(),
        kind,
        declaration_position: field.declaration_position,
        stable_number: field.stable_number,
        qualified_type: Some(field.qualified_type.clone()),
        owner_case_identity,
        erased: field.erased,
        head_nominal_identity,
    })
}

/// Re-derive the member set of `data` from the typed trees, in the same
/// authored order `graph_members` produces — the independent counterpart
/// replay checks frozen records against.
pub(crate) fn expected_members(
    typed: &TypedTrees,
    data: &DataDefinition,
    owner_identity: &str,
    application_identity: Option<&str>,
) -> Result<Vec<ScopedMember>, String> {
    let mut members = Vec::new();
    for (position, member) in typed.data_members(data).iter().enumerate() {
        match member {
            DataMember::Field(field) => {
                let (description, expectations) = describe_field(
                    typed,
                    field,
                    owner_identity,
                    position as u32,
                    SchemaNodeHandle::INVALID,
                    application_identity,
                )?;
                members.push(ScopedMember {
                    member_identity: description.member_identity,
                    name: description.name,
                    kind: MemberKind::Field,
                    declaration_position: description.declaration_position,
                    stable_number: description.stable_number,
                    qualified_type: Some(description.qualified_type),
                    owner_case_identity: None,
                    erased: description.erased,
                    head_nominal_identity: expectations
                        .first()
                        .map(|expectation| expectation.owner_identity.clone()),
                });
            }
            DataMember::Variant(variant) => {
                let case_identity = member_identity_or_name(
                    typed,
                    variant.symbol,
                    owner_identity,
                    variant.name.as_str(),
                );
                members.push(ScopedMember {
                    member_identity: case_identity.clone(),
                    name: variant.name.as_str().to_owned(),
                    kind: MemberKind::Case,
                    declaration_position: position as u32,
                    stable_number: variant.identity,
                    qualified_type: None,
                    owner_case_identity: None,
                    erased: false,
                    head_nominal_identity: Some(owner_identity.to_owned()),
                });
                for (payload_position, payload_field) in
                    typed.data_payload_fields(variant).iter().enumerate()
                {
                    let (description, expectations) = describe_field(
                        typed,
                        payload_field,
                        owner_identity,
                        payload_position as u32,
                        SchemaNodeHandle::INVALID,
                        application_identity,
                    )?;
                    members.push(ScopedMember {
                        member_identity: description.member_identity,
                        name: description.name,
                        kind: MemberKind::PayloadField,
                        declaration_position: description.declaration_position,
                        stable_number: description.stable_number,
                        qualified_type: Some(description.qualified_type),
                        owner_case_identity: Some(case_identity.clone()),
                        erased: description.erased,
                        head_nominal_identity: expectations
                            .first()
                            .map(|expectation| expectation.owner_identity.clone()),
                    });
                }
            }
        }
    }
    Ok(members)
}
