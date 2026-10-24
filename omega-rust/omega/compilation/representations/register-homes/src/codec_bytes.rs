//! Shared byte-append primitives for the crate's canonical encoders: u64
//! length framing, option tags, and length-prefixed element lists. Each call
//! site names its own artifact so panic text stays authored at the domain.

#[cfg(test)]
pub(crate) use optimization_core::codec_bytes::patch_encoded;
pub(crate) use optimization_core::codec_bytes::push_encoded;
/// u64 length prefix. `artifact` keeps each encoder's own panic text.
pub(crate) fn length(bytes: &mut Vec<u8>, value: usize, artifact: &'static str) {
    bytes.extend_from_slice(
        &u64::try_from(value)
            .unwrap_or_else(|_| panic!("{artifact} length fits u64"))
            .to_le_bytes(),
    );
}

/// 0/1 tag followed by the payload only when present.
pub(crate) fn option<T>(
    bytes: &mut Vec<u8>,
    value: Option<T>,
    encode: impl FnOnce(&mut Vec<u8>, T),
) {
    match value {
        None => bytes.push(0),
        Some(value) => {
            bytes.push(1);
            encode(bytes, value);
        }
    }
}

/// Length prefix followed by each element in order.
pub(crate) fn list<'a, T: 'a>(
    bytes: &mut Vec<u8>,
    values: &'a [T],
    artifact: &'static str,
    mut encode: impl FnMut(&mut Vec<u8>, &'a T),
) {
    length(bytes, values.len(), artifact);
    for value in values {
        encode(bytes, value);
    }
}

/// One artifact's canonical envelope: the wire magic and version that precede
/// `IDENTITY[32] ++ CONTENT`. Framing and admission live here so no codec
/// reassembles the sequence, and the identity an encoder already derived from
/// its content is the one that is framed.
pub(crate) struct Envelope {
    magic: &'static [u8; 8],
    version: u32,
}

impl Envelope {
    pub(crate) const fn new(magic: &'static [u8; 8], version: u32) -> Self {
        Self { magic, version }
    }

    /// Frames the committed identity and content behind this envelope's magic
    /// and version, in the one canonical order.
    pub(crate) fn frame(&self, identity: [u8; 32], content: &[u8]) -> Vec<u8> {
        let mut encoded = Vec::with_capacity(self.magic.len() + 4 + identity.len() + content.len());
        encoded.extend_from_slice(self.magic);
        encoded.extend_from_slice(&self.version.to_le_bytes());
        encoded.extend_from_slice(&identity);
        encoded.extend_from_slice(content);
        encoded
    }

    /// Admits an artifact in the one canonical order — exact magic, expected
    /// version, then the committed identity — and leaves the cursor at the
    /// content that identity covers. The decoder's own wrong-magic /
    /// unsupported-version error is returned.
    pub(crate) fn open<'a, E>(
        &self,
        encoded: &'a [u8],
        wrong_magic: impl FnOnce() -> E,
        unsupported_version: impl FnOnce(u32) -> E,
    ) -> Result<(crate::cursor::Cursor<'a>, [u8; 32]), E>
    where
        E: From<crate::cursor::Truncated> + From<crate::cursor::TakeError>,
    {
        let mut cursor = crate::cursor::Cursor::new(encoded);
        if cursor.take(self.magic.len())? != self.magic.as_slice() {
            return Err(wrong_magic());
        }
        let version = cursor.u32()?;
        if version != self.version {
            return Err(unsupported_version(version));
        }
        let identity = cursor.array()?;
        Ok((cursor, identity))
    }
}

/// The shared canonical identity formula: `Sha256(domain-tag ++ content)`.
/// Every terminal artifact identity in this crate is this digest over its own
/// authored domain tag and canonical content bytes.
pub(crate) fn domain_identity(domain: &[u8], content: &[u8]) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
    digest.raw(content);
    digest.finish().into()
}
