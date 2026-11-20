use super::{SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape};

mod fingerprint {
    pub const OFFSET_BASIS: u64 = wire::fnv1a::Fnv1a::OFFSET_BASIS;

    pub fn byte(hash: &mut u64, value: u8) {
        *hash ^= u64::from(value);
        *hash = hash.wrapping_mul(wire::fnv1a::Fnv1a::PRIME);
    }
    pub fn bytes(hash: &mut u64, value: &[u8]) {
        for value in value {
            byte(hash, *value);
        }
    }
    pub fn uint(hash: &mut u64, value: u64) {
        bytes(hash, &value.to_le_bytes());
    }
    pub fn text(hash: &mut u64, value: &str) {
        uint(hash, value.len() as u64);
        bytes(hash, value.as_bytes());
    }
    pub fn optional_text(hash: &mut u64, value: &Option<String>) {
        match value {
            Some(value) => {
                byte(hash, 1);
                text(hash, value);
            }
            None => byte(hash, 0),
        }
    }
    pub fn optional_uint(hash: &mut u64, value: Option<u64>) {
        match value {
            Some(value) => {
                byte(hash, 1);
                uint(hash, value);
            }
            None => byte(hash, 0),
        }
    }

    pub fn query_authority(hash: &mut u64, authority: &super::SchemaQueryAuthority) {
        match authority {
            super::SchemaQueryAuthority::OwningScope { requester_identity } => {
                byte(hash, 0);
                text(hash, requester_identity);
            }
            super::SchemaQueryAuthority::ForeignScope { requester_identity } => {
                byte(hash, 1);
                text(hash, requester_identity);
            }
        }
    }
}

pub(super) fn graph_revision(authority: &SchemaQueryAuthority, nodes: &[SchemaNode]) -> u64 {
    use fingerprint::{byte, bytes, optional_text, optional_uint, query_authority, text, uint};
    fn handle(hash: &mut u64, value: SchemaNodeHandle) {
        uint(hash, value.0 as u64);
    }
    fn handles(hash: &mut u64, values: &[SchemaNodeHandle]) {
        uint(hash, values.len() as u64);
        for value in values {
            handle(hash, *value);
        }
    }
    fn identities(hash: &mut u64, values: &[u64]) {
        uint(hash, values.len() as u64);
        for value in values {
            uint(hash, *value);
        }
    }

    let mut hash = fingerprint::OFFSET_BASIS;
    bytes(&mut hash, b"omega.reflect.schema.v2");
    query_authority(&mut hash, authority);
    uint(&mut hash, nodes.len() as u64);
    for node in nodes {
        match node {
            SchemaNode::Invalid => byte(&mut hash, 0),
            SchemaNode::Declaration(declaration) => {
                byte(&mut hash, 1);
                text(&mut hash, &declaration.owner_identity);
                byte(&mut hash, declaration.identity_is_hermetic as u8);
                text(&mut hash, &declaration.name);
                byte(
                    &mut hash,
                    match declaration.shape {
                        SchemaShape::Empty => 0,
                        SchemaShape::Record => 1,
                        SchemaShape::Sum => 2,
                        SchemaShape::Mixed => 3,
                        SchemaShape::Primitive(_) => 4,
                    },
                );
                if let SchemaShape::Primitive(atom) = declaration.shape {
                    text(&mut hash, &atom.identity());
                }
                text(&mut hash, &declaration.supply);
                text(&mut hash, &declaration.multiplicity);
                optional_text(&mut hash, &declaration.carry);
                uint(&mut hash, declaration.type_parameters.len() as u64);
                for parameter in &declaration.type_parameters {
                    text(&mut hash, parameter.name.as_str());
                    text(&mut hash, parameter.kind);
                    optional_text(&mut hash, &parameter.type_identity);
                    text(&mut hash, &parameter.bounds);
                }
                uint(&mut hash, declaration.lifetime_parameters.len() as u64);
                for name in &declaration.lifetime_parameters {
                    text(&mut hash, name);
                }
                optional_text(&mut hash, &declaration.application_identity);
                handles(&mut hash, &declaration.members);
                identities(&mut hash, &declaration.retired_identities);
            }
            SchemaNode::Field(field) => {
                byte(&mut hash, 2);
                text(&mut hash, &field.member_identity);
                text(&mut hash, &field.name);
                uint(&mut hash, u64::from(field.declaration_position));
                optional_uint(&mut hash, field.stable_number);
                byte(&mut hash, field.erased as u8);
                text(&mut hash, &field.qualified_type);
                handles(&mut hash, &field.nominal_references);
                handle(&mut hash, field.owner_case);
            }
            SchemaNode::Case(case) => {
                byte(&mut hash, 3);
                text(&mut hash, &case.member_identity);
                text(&mut hash, &case.name);
                uint(&mut hash, u64::from(case.declaration_position));
                optional_uint(&mut hash, case.stable_number);
                handles(&mut hash, &case.payload_fields);
                identities(&mut hash, &case.retired_payload_identities);
            }
            SchemaNode::NominalReference(reference) => {
                byte(&mut hash, 4);
                text(&mut hash, &reference.owner_identity);
                text(&mut hash, &reference.owner_kind);
                text(&mut hash, &reference.name);
                text(&mut hash, &reference.application_identity);
            }
        }
    }
    if hash == 0 { 1 } else { hash }
}
