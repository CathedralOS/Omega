#[derive(Debug, Clone, Copy)]
pub(crate) enum CountFrame {
    Wire,
    Canonical,
}

impl CountFrame {
    pub(crate) fn push(self, encoded: &mut Vec<u8>, count: usize) {
        match self {
            Self::Wire => ::wire::codec_bytes::push_count_u32(encoded, count),
            Self::Canonical => ::wire::codec_bytes::push_count_u64(encoded, count),
        }
    }
}

pub(crate) fn encode_work_counters(fields: [u64; 5]) -> [u8; 40] {
    let mut encoded = [0; 40];
    for (index, value) in fields.into_iter().enumerate() {
        ::wire::codec_bytes::patch_encoded(&mut encoded, index * 8, value.to_le_bytes());
    }
    encoded
}

pub(crate) fn decode_work_counters(encoded: &[u8]) -> Option<[u64; 5]> {
    let row: [u8; 40] = encoded.try_into().ok()?;
    let mut fields = [0; 5];
    let (chunks, _) = row.as_chunks::<8>();
    for (field, chunk) in fields.iter_mut().zip(chunks) {
        *field = u64::from_le_bytes(*chunk);
    }
    Some(fields)
}
