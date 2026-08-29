//! Canonical byte framing, hexadecimal conversion, and subject fingerprinting.

use super::super::subject::SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN;
use super::super::{CanonicalSourceClosureSubjectError, CanonicalSourceClosureSubjectFingerprint};
use sha2::{Digest, Sha256};

pub(in super::super) fn fingerprint(bytes: &[u8]) -> CanonicalSourceClosureSubjectFingerprint {
    let mut hasher = Sha256::new();
    hasher.update(SOURCE_CLOSURE_SUBJECT_FINGERPRINT_DOMAIN);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    CanonicalSourceClosureSubjectFingerprint(hasher.finalize().into())
}

pub(super) fn decode_hex_32(value: &str) -> Result<[u8; 32], CanonicalSourceClosureSubjectError> {
    let bytes = decode_hex(value).ok_or_else(|| {
        CanonicalSourceClosureSubjectError::new("invalid 32-byte hexadecimal value")
    })?;
    bytes
        .try_into()
        .map_err(|_| CanonicalSourceClosureSubjectError::new("invalid 32-byte hexadecimal value"))
}

fn decode_hex(value: &str) -> Option<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return None;
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|digits| {
            let high = hex_value(digits[0])?;
            let low = hex_value(digits[1])?;
            Some((high << 4) | low)
        })
        .collect()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

pub(in super::super) fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(char::from(HEX[usize::from(byte >> 4)]));
        encoded.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    encoded
}

pub(super) struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    pub(super) fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub(super) fn fixed(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    pub(super) fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub(super) fn u16(&mut self, value: u16) {
        self.fixed(&value.to_le_bytes());
    }

    pub(super) fn u32(&mut self, value: u32) {
        self.fixed(&value.to_le_bytes());
    }

    pub(super) fn count(&mut self, value: usize) -> Result<(), CanonicalSourceClosureSubjectError> {
        self.u32(u32::try_from(value).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical sequence count exceeds u32")
        })?);
        Ok(())
    }

    pub(super) fn bytes_bounded(
        &mut self,
        value: &[u8],
        maximum_bytes: usize,
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        if value.len() > maximum_bytes {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical field exceeds its byte limit",
            ));
        }
        self.count(value.len())?;
        self.fixed(value);
        Ok(())
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

pub(in super::super) struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    pub(in super::super) fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], CanonicalSourceClosureSubjectError> {
        let end = self.cursor.checked_add(count).ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("source-closure subject offset overflow")
        })?;
        let bytes = self.bytes.get(self.cursor..end).ok_or_else(|| {
            CanonicalSourceClosureSubjectError::new("truncated source-closure subject")
        })?;
        self.cursor = end;
        Ok(bytes)
    }

    pub(in super::super) fn expect_fixed(
        &mut self,
        expected: &[u8],
    ) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.take(expected.len())? == expected {
            Ok(())
        } else {
            Err(CanonicalSourceClosureSubjectError::new(
                "invalid source-closure subject header",
            ))
        }
    }

    pub(in super::super) fn byte(&mut self) -> Result<u8, CanonicalSourceClosureSubjectError> {
        Ok(self.take(1)?[0])
    }

    pub(in super::super) fn u16(&mut self) -> Result<u16, CanonicalSourceClosureSubjectError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    pub(in super::super) fn u32(&mut self) -> Result<u32, CanonicalSourceClosureSubjectError> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub(in super::super) fn count(
        &mut self,
        maximum: usize,
    ) -> Result<usize, CanonicalSourceClosureSubjectError> {
        let count = usize::try_from(self.u32()?).map_err(|_| {
            CanonicalSourceClosureSubjectError::new("canonical count exceeds platform range")
        })?;
        if count > maximum {
            return Err(CanonicalSourceClosureSubjectError::new(
                "canonical count exceeds its resource limit",
            ));
        }
        Ok(count)
    }

    pub(in super::super) fn bytes(
        &mut self,
        maximum: usize,
    ) -> Result<&'a [u8], CanonicalSourceClosureSubjectError> {
        let count = self.count(maximum)?;
        self.take(count)
    }

    pub(in super::super) fn string(
        &mut self,
        maximum: usize,
    ) -> Result<String, CanonicalSourceClosureSubjectError> {
        String::from_utf8(self.bytes(maximum)?.to_vec())
            .map_err(|_| CanonicalSourceClosureSubjectError::new("canonical string is not UTF-8"))
    }

    pub(in super::super) fn array_32(
        &mut self,
    ) -> Result<[u8; 32], CanonicalSourceClosureSubjectError> {
        Ok(self.take(32)?.try_into().unwrap())
    }

    pub(in super::super) fn finish(self) -> Result<(), CanonicalSourceClosureSubjectError> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(CanonicalSourceClosureSubjectError::new(
                "source-closure subject has trailing bytes",
            ))
        }
    }
}
