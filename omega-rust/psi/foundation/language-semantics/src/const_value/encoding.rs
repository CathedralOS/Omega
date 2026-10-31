//! The existing canonical value format's bounded structural writer.

use super::{
    DecodedCanonicalConstValue, MAX_DECODE_DEPTH, MAX_DECODED_NODES, MAX_ENCODING_BYTES, framed,
};

pub(super) fn encode(value: &DecodedCanonicalConstValue) -> Option<String> {
    node(value, 0, &mut 0)
}

fn node(value: &DecodedCanonicalConstValue, depth: usize, nodes: &mut usize) -> Option<String> {
    if depth >= MAX_DECODE_DEPTH || *nodes >= MAX_DECODED_NODES {
        return None;
    }
    *nodes += 1;
    let encoding = match value {
        DecodedCanonicalConstValue::Integer { type_name, value } => {
            framed("integer", [type_name.as_str(), value.to_string().as_str()])
        }
        DecodedCanonicalConstValue::Boolean(value) => framed("boolean", [value.to_string()]),
        DecodedCanonicalConstValue::Float { .. } => return None,
        DecodedCanonicalConstValue::Array { type_name, values } => {
            let mut pieces = vec![type_name.clone()];
            for value in values {
                pieces.push(node(value, depth + 1, nodes)?);
            }
            framed("array", pieces)
        }
        DecodedCanonicalConstValue::Record { type_name, fields } => framed(
            "record",
            fields_encoding(vec![type_name.clone()], fields, depth, nodes)?,
        ),
        DecodedCanonicalConstValue::Variant {
            type_name,
            case_name,
            fields,
        } => framed(
            "variant",
            fields_encoding(
                vec![type_name.clone(), case_name.clone()],
                fields,
                depth,
                nodes,
            )?,
        ),
    };
    (encoding.len() <= MAX_ENCODING_BYTES).then_some(encoding)
}

fn fields_encoding(
    mut pieces: Vec<String>,
    fields: &[(String, DecodedCanonicalConstValue)],
    depth: usize,
    nodes: &mut usize,
) -> Option<Vec<String>> {
    for (name, value) in fields {
        pieces.push(name.clone());
        pieces.push(node(value, depth + 1, nodes)?);
    }
    Some(pieces)
}
