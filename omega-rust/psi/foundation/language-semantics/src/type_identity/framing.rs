//! Borrowed delimiters and exact, precharged unescaping of semantic atoms.

use super::{Error, Result, TypeIdentityPackageOwnerVisitor};
use std::borrow::Cow;

pub(super) struct Reader<'a> {
    text: &'a str,
    position: usize,
}

impl<'a> Reader<'a> {
    pub(super) fn new(text: &'a str) -> Self {
        Self { text, position: 0 }
    }

    pub(super) fn finish(&self) -> Result {
        if self.position == self.text.len() {
            Ok(())
        } else {
            Err(Error::MalformedIdentity)
        }
    }

    pub(super) fn next(&self, byte: u8) -> bool {
        self.text.as_bytes().get(self.position) == Some(&byte)
    }

    pub(super) fn expect(&mut self, byte: u8) -> Result {
        if !self.next(byte) {
            return Err(Error::MalformedIdentity);
        }
        self.position += 1;
        Ok(())
    }

    pub(super) fn tag(&mut self) -> Result<&'a str> {
        let start = self.position;
        while self
            .text
            .as_bytes()
            .get(self.position)
            .is_some_and(|byte| byte.is_ascii_lowercase() || *byte == b'-')
        {
            self.position += 1;
        }
        if start == self.position {
            return Err(Error::MalformedIdentity);
        }
        Ok(&self.text[start..self.position])
    }

    pub(super) fn starts(&self, prefix: &str) -> bool {
        self.text[self.position..].starts_with(prefix)
    }

    fn raw_atom(&mut self, expected: &str) -> Result<(&'a str, usize)> {
        if self.tag()? != expected {
            return Err(Error::MalformedIdentity);
        }
        self.expect(b'(')?;
        let start = self.position;
        let mut decoded = 0usize;
        loop {
            match self.text.as_bytes().get(self.position).copied() {
                Some(b')') => {
                    let value = &self.text[start..self.position];
                    self.position += 1;
                    return Ok((value, decoded));
                }
                Some(b'\\') => {
                    self.position += 1;
                    if !matches!(
                        self.text.as_bytes().get(self.position),
                        Some(b'\\' | b'(' | b')' | b',')
                    ) {
                        return Err(Error::MalformedIdentity);
                    }
                }
                Some(b'(' | b',') | None => return Err(Error::MalformedIdentity),
                Some(_) => {}
            }
            self.position += 1;
            decoded += 1;
        }
    }

    pub(super) fn opaque(
        &mut self,
        tag: &str,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        visitor.enter()?;
        let result = self.raw_atom(tag).map(|_| ());
        visitor.leave();
        result
    }

    pub(super) fn atom(
        &mut self,
        tag: &str,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result<Cow<'a, str>> {
        visitor.enter()?;
        let result = (|| {
            let (raw, decoded) = self.raw_atom(tag)?;
            if raw.len() == decoded {
                return Ok(Cow::Borrowed(raw));
            }
            visitor.reserve(decoded)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(decoded)
                .map_err(|_| Error::AllocationFailed)?;
            let mut source = raw.bytes();
            while let Some(byte) = source.next() {
                bytes.push(if byte == b'\\' {
                    source.next().expect("prevalidated escape")
                } else {
                    byte
                });
            }
            Ok(Cow::Owned(
                String::from_utf8(bytes).map_err(|_| Error::MalformedIdentity)?,
            ))
        })();
        visitor.leave();
        result
    }

    pub(super) fn byte_atom(
        &mut self,
        tag: &str,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result<&'a str> {
        visitor.enter()?;
        let result = (|| {
            let (raw, _) = self.raw_atom(tag)?;
            let (length, hexadecimal) = raw.split_once(':').ok_or(Error::MalformedIdentity)?;
            if length.is_empty() || (length.len() > 1 && length.starts_with('0')) {
                return Err(Error::MalformedIdentity);
            }
            let length = length
                .bytes()
                .try_fold(0usize, |value, byte| {
                    if !byte.is_ascii_digit() {
                        return None;
                    }
                    value.checked_mul(10)?.checked_add(usize::from(byte - b'0'))
                })
                .ok_or(Error::MalformedIdentity)?;
            if length.checked_mul(2) != Some(hexadecimal.len())
                || !hexadecimal
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(Error::MalformedIdentity);
            }
            Ok(hexadecimal)
        })();
        visitor.leave();
        result
    }
}
