use register_model::RegisterUnitId;

pub(super) fn encode_units(bytes: &mut Vec<u8>, units: &[RegisterUnitId]) {
    crate::codec_bytes::encode_le_list(bytes, units, |unit| unit.0.to_le_bytes());
}

pub fn encode_ids(bytes: &mut Vec<u8>, values: impl ExactSizeIterator<Item = u64>) {
    crate::codec_bytes::encode_len(bytes, values.len());
    for value in values {
        crate::codec_bytes::push_encoded(bytes, value.to_le_bytes());
    }
}
