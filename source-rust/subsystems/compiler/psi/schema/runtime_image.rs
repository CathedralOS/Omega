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
                    SchemaShape::Primitive(_) => 4,
                },
            );
            if let SchemaShape::Primitive(atom) = value.shape {
                use symbols::BuiltinTypeAtom;
                let (kind, width, signed) = match atom {
                    BuiltinTypeAtom::Bool => (0, 0, false),
                    BuiltinTypeAtom::Integer(carrier) => (1, carrier.width(), carrier.is_signed()),
                    BuiltinTypeAtom::Address => (2, 0, false),
                    BuiltinTypeAtom::F32 => (3, 0, false),
                    BuiltinTypeAtom::F64 => (4, 0, false),
                    _ => unreachable!("a primitive schema has a scalar atom"),
                };
                word(&mut bytes, 1, kind);
                word(&mut bytes, 2, width);
                word(&mut bytes, 3, u64::from(signed));
            }
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

fn image_word(bytes: &[u8], slot: usize) -> Result<u64, String> {
    let start = slot
        .checked_mul(8)
        .ok_or("schema image word offset overflows")?;
    let end = start
        .checked_add(8)
        .ok_or("schema image word extent overflows")?;
    let value = bytes
        .get(start..end)
        .ok_or("schema image lost a complete word")?;
    Ok(u64::from_le_bytes(
        value.try_into().expect("eight-byte word"),
    ))
}

fn image_span(bytes: &[u8], slot: usize) -> Result<&[u8], String> {
    let start = usize::try_from(image_word(bytes, slot)?)
        .map_err(|_| "schema image span offset exceeds this host")?;
    let length = usize::try_from(image_word(bytes, slot + 1)?)
        .map_err(|_| "schema image span length exceeds this host")?;
    let end = start
        .checked_add(length)
        .ok_or("schema image span overflows")?;
    bytes
        .get(start..end)
        .ok_or_else(|| "schema image span lies outside its owner".into())
}

fn image_text(bytes: &[u8], slot: usize) -> Result<String, String> {
    std::str::from_utf8(image_span(bytes, slot)?)
        .map(str::to_owned)
        .map_err(|error| format!("schema image text is invalid: {error}"))
}

fn image_numbers(bytes: &[u8], slot: usize) -> Result<Vec<u64>, String> {
    let values = image_span(bytes, slot)?;
    if !values.len().is_multiple_of(8) {
        return Err("schema image number sequence has a partial word".into());
    }
    values
        .as_chunks::<8>()
        .0
        .iter()
        .map(|word| image_word(word, 0))
        .collect()
}

fn image_handles(bytes: &[u8], slot: usize) -> Result<Vec<super::SchemaNodeHandle>, String> {
    image_numbers(bytes, slot)?
        .into_iter()
        .map(|value| {
            u32::try_from(value)
                .map(super::SchemaNodeHandle::from_index)
                .map_err(|_| "schema image node coordinate exceeds its representation".into())
        })
        .collect()
}

fn image_strings(bytes: &[u8], slot: usize) -> Result<Vec<String>, String> {
    let mut remaining = image_span(bytes, slot)?;
    let mut values = Vec::new();
    while !remaining.is_empty() {
        let length = usize::try_from(image_word(remaining, 0)?)
            .map_err(|_| "schema image text length exceeds this host")?;
        let end = 8usize
            .checked_add(length)
            .ok_or("schema image text extent overflows")?;
        let text = remaining
            .get(8..end)
            .ok_or("schema image lost its complete text")?;
        values.push(
            std::str::from_utf8(text)
                .map(str::to_owned)
                .map_err(|error| format!("schema image text is invalid: {error}"))?,
        );
        remaining = &remaining[end..];
    }
    Ok(values)
}

fn image_boolean(bytes: &[u8], slot: usize) -> Result<bool, String> {
    match image_word(bytes, slot)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err("schema image Boolean is not canonical".into()),
    }
}

fn image_node(bytes: &[u8]) -> Result<SchemaNode, String> {
    use super::{
        CaseDescription, DeclarationDescription, FieldDescription, NominalReferenceDescription,
        SchemaNodeHandle, TypeParameterDescription,
    };
    if bytes.len() < NODE_WORDS * 8 {
        return Err("schema image lost a complete node".into());
    }
    let optional = |text: String| (!text.is_empty()).then_some(text);
    let position = || -> Result<u32, String> {
        u32::try_from(image_word(bytes, 1)?)
            .map_err(|_| "schema image declaration position exceeds its representation".into())
    };
    let stable = || {
        if image_boolean(bytes, 2)? {
            image_word(bytes, 3).map(Some)
        } else {
            Ok(None)
        }
    };
    Ok(match image_word(bytes, 0)? {
        0 => SchemaNode::Invalid,
        1 => {
            let parameters = image_strings(bytes, 30)?;
            if !parameters.len().is_multiple_of(4) {
                return Err("schema image lost a complete type parameter".into());
            }
            let parameters = parameters
                .as_chunks::<4>()
                .0
                .iter()
                .map(|parts| {
                    let kind = match parts[1].as_str() {
                        "type" => "type",
                        "const" => "const",
                        "value" => "value",
                        "machine" => "machine",
                        "implementation" => "implementation",
                        "proposition" => "proposition",
                        _ => {
                            return Err("schema image has an unknown parameter kind".to_owned());
                        }
                    };
                    Ok(TypeParameterDescription {
                        name: parts[0].clone(),
                        kind,
                        type_identity: optional(parts[2].clone()),
                        bounds: parts[3].clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            SchemaNode::Declaration(DeclarationDescription {
                owner_identity: image_text(bytes, 8)?,
                identity_is_hermetic: image_boolean(bytes, 7)?,
                name: image_text(bytes, 10)?,
                shape: match image_word(bytes, 6)? {
                    0 => SchemaShape::Empty,
                    1 => SchemaShape::Record,
                    2 => SchemaShape::Sum,
                    3 => SchemaShape::Mixed,
                    4 => {
                        use symbols::BuiltinTypeAtom;
                        let width = image_word(bytes, 2)?;
                        let signed = image_boolean(bytes, 3)?;
                        let atom = match (image_word(bytes, 1)?, width, signed) {
                            (0, 0, false) => BuiltinTypeAtom::Bool,
                            (1, width, signed) => BuiltinTypeAtom::Integer(
                                numerics::integer_carrier::FixedInteger::new(
                                    if signed {
                                        numerics::integer_carrier::IntegerSign::Signed
                                    } else {
                                        numerics::integer_carrier::IntegerSign::Unsigned
                                    },
                                    width,
                                )
                                .ok_or("primitive schema has an invalid integer carrier")?,
                            ),
                            (2, 0, false) => BuiltinTypeAtom::Address,
                            (3, 0, false) => BuiltinTypeAtom::F32,
                            (4, 0, false) => BuiltinTypeAtom::F64,
                            _ => {
                                return Err("primitive schema has an invalid scalar carrier".into());
                            }
                        };
                        SchemaShape::Primitive(atom)
                    }
                    _ => return Err("schema image has an unknown declaration shape".into()),
                },
                supply: image_text(bytes, 18)?,
                multiplicity: image_text(bytes, 20)?,
                carry: optional(image_text(bytes, 22)?),
                type_parameters: parameters,
                lifetime_parameters: image_strings(bytes, 32)?,
                application_identity: optional(image_text(bytes, 16)?),
                members: image_handles(bytes, 24)?,
                retired_identities: image_numbers(bytes, 26)?,
            })
        }
        2 => SchemaNode::Field(FieldDescription {
            member_identity: image_text(bytes, 8)?,
            name: image_text(bytes, 10)?,
            declaration_position: position()?,
            stable_number: stable()?,
            erased: image_boolean(bytes, 4)?,
            qualified_type: image_text(bytes, 12)?,
            nominal_references: image_handles(bytes, 28)?,
            owner_case: SchemaNodeHandle::from_index(
                u32::try_from(image_word(bytes, 5)?)
                    .map_err(|_| "schema image case coordinate exceeds its representation")?,
            ),
        }),
        3 => SchemaNode::Case(CaseDescription {
            member_identity: image_text(bytes, 8)?,
            name: image_text(bytes, 10)?,
            declaration_position: position()?,
            stable_number: stable()?,
            payload_fields: image_handles(bytes, 24)?,
            retired_payload_identities: image_numbers(bytes, 26)?,
        }),
        4 => SchemaNode::NominalReference(NominalReferenceDescription {
            owner_identity: image_text(bytes, 8)?,
            owner_kind: image_text(bytes, 14)?,
            name: image_text(bytes, 10)?,
            application_identity: image_text(bytes, 16)?,
        }),
        _ => return Err("schema image has an unknown node kind".into()),
    })
}

impl SemanticSchemaGraph {
    /// Decode the existing complete image as descriptive data. The source
    /// query/receipt owner authenticates where it came from; this operation
    /// neither reconstructs typed declarations nor grants query authority.
    pub fn from_runtime_image(bytes: &[u8]) -> Result<Self, String> {
        if image_word(bytes, 0)? != 2 {
            return Err("schema image has another version".into());
        }
        let count = usize::try_from(image_word(bytes, 1)?)
            .map_err(|_| "schema image node count exceeds this host")?;
        let available = bytes.len().saturating_sub(HEADER_WORDS * 8) / 16;
        if count == 0 || count > available {
            return Err("schema image node count exceeds its actual table".into());
        }
        let root = super::SchemaNodeHandle::from_index(
            u32::try_from(image_word(bytes, 2)?)
                .map_err(|_| "schema image root exceeds its representation")?,
        );
        let requester_identity = image_text(bytes, 5)?;
        let authority = match image_word(bytes, 4)? {
            0 => SchemaQueryAuthority::OwningScope { requester_identity },
            1 => SchemaQueryAuthority::ForeignScope { requester_identity },
            _ => return Err("schema image has another query scope".into()),
        };
        let nodes = (0..count)
            .map(|index| image_node(image_span(bytes, HEADER_WORDS + index * 2)?))
            .collect::<Result<Vec<_>, String>>()?;
        let graph = Self::from_nodes(nodes, root, authority)?;
        // Compare every encoded field, the root and the entire image,
        // including canonical offsets and the original change fingerprint.
        if graph.revision != image_word(bytes, 3)? || graph.runtime_image() != bytes {
            return Err("schema image differs from its complete canonical graph".into());
        }
        Ok(graph)
    }

    pub fn runtime_image(&self) -> Vec<u8> {
        let mut bytes = vec![0; HEADER_WORDS * 8 + self.nodes.len() * 16];
        word(&mut bytes, 0, 2);
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
