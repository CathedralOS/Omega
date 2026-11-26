use wire::fnv1a::Fnv1a;

use super::{SchemaNode, SchemaNodeHandle, SchemaQueryAuthority, SchemaShape};

fn optional_text(hash: &mut Fnv1a, value: &Option<String>) {
    hash.flag(value.is_some());
    if let Some(value) = value {
        hash.string_framed(value);
    }
}

fn optional_uint(hash: &mut Fnv1a, value: Option<u64>) {
    hash.flag(value.is_some());
    if let Some(value) = value {
        hash.u64(value);
    }
}

fn query_authority(hash: &mut Fnv1a, authority: &SchemaQueryAuthority) {
    let (tag, requester_identity) = match authority {
        SchemaQueryAuthority::OwningScope { requester_identity } => (0, requester_identity),
        SchemaQueryAuthority::ForeignScope { requester_identity } => (1, requester_identity),
    };
    hash.byte(tag);
    hash.string_framed(requester_identity);
}

pub(super) fn graph_revision(authority: &SchemaQueryAuthority, nodes: &[SchemaNode]) -> u64 {
    fn handle(hash: &mut Fnv1a, value: SchemaNodeHandle) {
        hash.u64(value.0 as u64);
    }
    fn handles(hash: &mut Fnv1a, values: &[SchemaNodeHandle]) {
        hash.u64(values.len() as u64);
        for value in values {
            handle(hash, *value);
        }
    }
    fn identities(hash: &mut Fnv1a, values: &[u64]) {
        hash.u64(values.len() as u64);
        for value in values {
            hash.u64(*value);
        }
    }

    let mut hash = Fnv1a::new();
    hash.bytes(b"omega.reflect.schema.v2");
    query_authority(&mut hash, authority);
    hash.u64(nodes.len() as u64);
    for node in nodes {
        match node {
            SchemaNode::Invalid => hash.byte(0),
            SchemaNode::Declaration(declaration) => {
                hash.byte(1);
                hash.string_framed(&declaration.owner_identity);
                hash.byte(declaration.identity_is_hermetic as u8);
                hash.string_framed(&declaration.name);
                hash.byte(match declaration.shape {
                    SchemaShape::Empty => 0,
                    SchemaShape::Record => 1,
                    SchemaShape::Sum => 2,
                    SchemaShape::Mixed => 3,
                    SchemaShape::Primitive(_) => 4,
                });
                if let SchemaShape::Primitive(atom) = declaration.shape {
                    hash.string_framed(&atom.identity());
                }
                hash.string_framed(&declaration.supply);
                hash.string_framed(&declaration.multiplicity);
                hash.byte(0); // Reserved absent property in existing schema revisions.
                hash.u64(declaration.type_parameters.len() as u64);
                for parameter in &declaration.type_parameters {
                    hash.string_framed(parameter.name.as_str());
                    hash.string_framed(parameter.kind);
                    optional_text(&mut hash, &parameter.type_identity);
                    hash.string_framed(&parameter.bounds);
                }
                hash.u64(declaration.lifetime_parameters.len() as u64);
                for name in &declaration.lifetime_parameters {
                    hash.string_framed(name);
                }
                optional_text(&mut hash, &declaration.application_identity);
                handles(&mut hash, &declaration.members);
                identities(&mut hash, &declaration.retired_identities);
            }
            SchemaNode::Field(field) => {
                hash.byte(2);
                hash.string_framed(&field.member_identity);
                hash.string_framed(&field.name);
                hash.u64(u64::from(field.declaration_position));
                optional_uint(&mut hash, field.stable_number);
                hash.byte(field.erased as u8);
                hash.string_framed(&field.qualified_type);
                handles(&mut hash, &field.nominal_references);
                handle(&mut hash, field.owner_case);
            }
            SchemaNode::Case(case) => {
                hash.byte(3);
                hash.string_framed(&case.member_identity);
                hash.string_framed(&case.name);
                hash.u64(u64::from(case.declaration_position));
                optional_uint(&mut hash, case.stable_number);
                handles(&mut hash, &case.payload_fields);
                identities(&mut hash, &case.retired_payload_identities);
            }
            SchemaNode::NominalReference(reference) => {
                hash.byte(4);
                hash.string_framed(&reference.owner_identity);
                hash.string_framed(&reference.owner_kind);
                hash.string_framed(&reference.name);
                hash.string_framed(&reference.application_identity);
            }
        }
    }
    hash.finish_or(1)
}
