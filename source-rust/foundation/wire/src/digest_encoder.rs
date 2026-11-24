//! SHA-256 input framing and lowercase hexadecimal output.

use sha2::{Digest, Sha256};

#[derive(Clone)]
pub struct DomainDigestEncoder(Sha256);

impl DomainDigestEncoder {
    pub fn new(domain: &[u8]) -> Self {
        let mut digest = Sha256::new();
        digest.update(domain);
        Self(digest)
    }

    pub fn finish(self) -> [u8; 32] {
        self.0.finalize().into()
    }

    pub fn byte(&mut self, value: u8) {
        self.0.update([value]);
    }

    pub fn len(&mut self, value: usize) {
        self.0.update((value as u64).to_le_bytes());
    }

    pub fn u16(&mut self, value: u16) {
        self.0.update(value.to_le_bytes());
    }

    pub fn u32(&mut self, value: u32) {
        self.0.update(value.to_le_bytes());
    }

    pub fn u64(&mut self, value: u64) {
        self.0.update(value.to_le_bytes());
    }

    pub fn i64(&mut self, value: i64) {
        self.0.update(value.to_le_bytes());
    }

    /// Appends a field whose width or boundary is already defined by the format.
    pub fn raw(&mut self, bytes: &[u8]) {
        self.0.update(bytes);
    }

    /// Appends a little-endian `u64` byte length followed by the field.
    pub fn bytes(&mut self, bytes: &[u8]) {
        self.len(bytes.len());
        self.0.update(bytes);
    }

    pub fn string(&mut self, value: &str) {
        self.bytes(value.as_bytes());
    }

    pub fn strings(&mut self, values: &[String]) {
        self.len(values.len());
        for value in values {
            self.string(value);
        }
    }

    pub fn optional_string(&mut self, value: Option<&str>) {
        match value {
            Some(value) => {
                self.byte(1);
                self.string(value);
            }
            None => self.byte(0),
        }
    }

    pub fn bool(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    pub fn content(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = Self::new(domain);
        digest.u64(bytes.len() as u64);
        digest.raw(bytes);
        digest.finish()
    }
}

pub fn content_identity(content: &[u8]) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"");
    digest.raw(content);
    digest.finish()
}

pub fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(DIGITS[(byte >> 4) as usize] as char);
        text.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    text
}
