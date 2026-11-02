//! Immutable program-data projection of the existing semantic graph.
//!
//! This is descriptive storage, never a new source of semantic authority.
//! The ordinary core views read these records; no compiler handles survive.

use super::{SchemaNode, SchemaQueryAuthority, SchemaShape, SemanticSchemaGraph};

const HEADER_WORDS: usize = 8;
const NODE_WORDS: usize = 34;

fn word(bytes: &mut [u8], slot: usize, value: u64) {
    bytes[slot * 8..slot * 8 + 8].copy_from_slice(&value.to_le_bytes());
}

fn span(bytes: &mut Vec<u8>, slot: usize, value: &[u8]) {
    let start = bytes.len() as u64;
    bytes.extend_from_slice(value);
    word(bytes, slot, start);
    word(bytes, slot + 1, value.len() as u64);
}

fn numbers(values: impl IntoIterator<Item = u64>) -> Vec<u8> {
    values.into_iter().flat_map(u64::to_le_bytes).collect()
}

fn strings(values: impl IntoIterator<Item = impl AsRef<str>>) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in values {
        let value = value.as_ref().as_bytes();
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    }
    bytes
}

fn record(node: &SchemaNode) -> Vec<u8> {
    let mut bytes = vec![0; NODE_WORDS * 8];
    match node {
        SchemaNode::Invalid => {}
        SchemaNode::Declaration(value) => {
            word(&mut bytes, 0, 1);
            word(
                &mut bytes,
                6,
                match value.shape {
                    SchemaShape::Empty => 0,
                    SchemaShape::Record => 1,
                    SchemaShape::Sum => 2,
                    SchemaShape::Mixed => 3,
                },
            );
            word(&mut bytes, 7, u64::from(value.identity_is_hermetic));
            span(&mut bytes, 8, value.owner_identity.as_bytes());
            span(&mut bytes, 10, value.name.as_bytes());
            span(
                &mut bytes,
                16,
                value
                    .application_identity
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes(),
            );
            span(&mut bytes, 18, value.supply.as_bytes());
            span(&mut bytes, 20, value.multiplicity.as_bytes());
            span(
                &mut bytes,
                22,
                value.carry.as_deref().unwrap_or("").as_bytes(),
            );
            span(
                &mut bytes,
                24,
                &numbers(value.members.iter().map(|h| h.index() as u64)),
            );
            span(
                &mut bytes,
                26,
                &numbers(value.retired_identities.iter().copied()),
            );
            let mut parameters = Vec::new();
            for parameter in &value.type_parameters {
                parameters.extend(strings([
                    parameter.name.as_str(),
                    parameter.kind,
                    parameter.type_identity.as_deref().unwrap_or(""),
                    parameter.bounds.as_str(),
                ]));
            }
            span(&mut bytes, 30, &parameters);
            span(&mut bytes, 32, &strings(&value.lifetime_parameters));
        }
        SchemaNode::Field(value) => {
            word(&mut bytes, 0, 2);
            word(&mut bytes, 1, u64::from(value.declaration_position));
            word(&mut bytes, 2, u64::from(value.stable_number.is_some()));
            word(&mut bytes, 3, value.stable_number.unwrap_or(0));
            word(&mut bytes, 4, u64::from(value.erased));
            word(&mut bytes, 5, value.owner_case.index() as u64);
            span(&mut bytes, 8, value.member_identity.as_bytes());
            span(&mut bytes, 10, value.name.as_bytes());
            span(&mut bytes, 12, value.qualified_type.as_bytes());
            span(
                &mut bytes,
                28,
                &numbers(value.nominal_references.iter().map(|h| h.index() as u64)),
            );
        }
        SchemaNode::Case(value) => {
            word(&mut bytes, 0, 3);
            word(&mut bytes, 1, u64::from(value.declaration_position));
            word(&mut bytes, 2, u64::from(value.stable_number.is_some()));
            word(&mut bytes, 3, value.stable_number.unwrap_or(0));
            span(&mut bytes, 8, value.member_identity.as_bytes());
            span(&mut bytes, 10, value.name.as_bytes());
            span(
                &mut bytes,
                24,
                &numbers(value.payload_fields.iter().map(|h| h.index() as u64)),
            );
            span(
                &mut bytes,
                26,
                &numbers(value.retired_payload_identities.iter().copied()),
            );
        }
        SchemaNode::NominalReference(value) => {
            word(&mut bytes, 0, 4);
            span(&mut bytes, 8, value.owner_identity.as_bytes());
            span(&mut bytes, 10, value.name.as_bytes());
            span(&mut bytes, 14, value.owner_kind.as_bytes());
            span(&mut bytes, 16, value.application_identity.as_bytes());
        }
    }
    bytes
}

impl SemanticSchemaGraph {
    /// Materialize all nodes and exact edges in a versioned, unbounded image.
    /// Offsets select bytes only inside this image, not compiler storage.
    pub fn runtime_image(&self) -> Vec<u8> {
        let mut bytes = vec![0; HEADER_WORDS * 8 + self.nodes.len() * 16];
        word(&mut bytes, 0, 1);
        word(&mut bytes, 1, self.nodes.len() as u64);
        word(&mut bytes, 2, self.root.index() as u64);
        word(&mut bytes, 3, self.revision);
        let (authority, requester) = match &self.authority {
            SchemaQueryAuthority::OwningScope { requester_identity } => (0, requester_identity),
            SchemaQueryAuthority::ForeignScope { requester_identity } => (1, requester_identity),
        };
        word(&mut bytes, 4, authority);
        span(&mut bytes, 5, requester.as_bytes());
        for (index, node) in self.nodes.iter().enumerate() {
            span(&mut bytes, HEADER_WORDS + index * 2, &record(node));
        }
        bytes
    }
}
