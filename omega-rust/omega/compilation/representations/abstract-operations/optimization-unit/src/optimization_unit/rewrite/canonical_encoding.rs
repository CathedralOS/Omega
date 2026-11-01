//! Model-neutral canonical writers shared by rewrite identities and candidate encoding.
pub(super) use super::super::integer_intervals::{encode_integer_type, encode_integer_value};

use super::{ScalarType, ValueDefinitionSite};
/// The rewrite-candidate definition-site table, which is **not** the canonical
/// [`super::ValueDefinitionSite`] identity encoding.
///
/// These tags are one-based (`1`/`2`/`3`) while
/// [`crate::encode_value_definition_site_identity`] is zero-based (`0`/`1`/`2`),
/// matching the one-based convention the rest of this file uses for scalar
/// types, integer types and integer values. Both tables are already baked into
/// ratified identities — rewrite candidates and transformation-ledger records
/// on one side, plan and artifact identities on the other — so they cannot be
/// merged without changing one of them. Keep this copy, and do not "fix" the
/// off-by-one.
pub(super) fn encode_definition_site(bytes: &mut Vec<u8>, site: ValueDefinitionSite) {
    match site {
        ValueDefinitionSite::FunctionParameter(position) => {
            bytes.push(1);
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        ValueDefinitionSite::BlockParameter { block, position } => {
            bytes.push(2);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, position.to_le_bytes());
        }
        ValueDefinitionSite::Node { block, node } => {
            bytes.push(3);
            crate::codec_bytes::push_encoded(bytes, block.get().to_le_bytes());
            crate::codec_bytes::push_encoded(bytes, node.to_le_bytes());
        }
    }
}

pub(super) fn encode_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("canonical candidate list length fits u64")
            .to_le_bytes(),
    );
}

pub(super) fn encode_scalar_type(bytes: &mut Vec<u8>, scalar_type: ScalarType) {
    super::super::encode_scalar_type(
        bytes,
        scalar_type,
        |bytes, tag| bytes.push(tag),
        encode_integer_type,
    )
}
