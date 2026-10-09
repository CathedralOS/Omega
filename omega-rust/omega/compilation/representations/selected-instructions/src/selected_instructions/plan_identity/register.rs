//! Canonical virtual-register identity, including exact instruction-local scratch custody.
use super::encode_option_u16;
use super::encode_scalar_type;
use crate::VirtualRegister;
use optimization_unit::encode_value_definition_site_identity;

pub(super) fn encode(bytes: &mut Vec<u8>, register: &VirtualRegister) {
    crate::codec_bytes::push_encoded(bytes, register.id.0.to_le_bytes());
    encode_scalar_type(bytes, register.scalar_type);
    crate::codec_bytes::push_encoded(bytes, register.class.0.to_le_bytes());
    register.origin.encode_identity(bytes);
    match register.definition_site {
        Some(site) => {
            bytes.push(1);
            encode_value_definition_site_identity(bytes, site);
        }
        None => bytes.push(0),
    }
    encode_option_u16(bytes, register.entry_fixed_view.map(|view| view.0));
}
