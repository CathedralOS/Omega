//! Borrowed delimiters and exact, precharged unescaping of semantic atoms.

use super::vocabulary::{Spelled, Vocabulary, is_vocabulary_spelling};
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

    /// Bytes consumed so far; where a malformed identity stopped being readable.
    pub(super) fn position(&self) -> usize {
        self.position
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

    fn peek_token(&self) -> Option<&'a str> {
        let rest = &self.text[self.position..];
        let length = rest
            .bytes()
            .take_while(|byte| byte.is_ascii_lowercase() || matches!(byte, b'-' | b'_'))
            .count();
        (length != 0).then(|| &rest[..length])
    }

    /// Consume the next tag as a member of `F`'s closed vocabulary. An unknown
    /// tag is left unread, so the failure stands where that tag begins.
    pub(super) fn production<F: Vocabulary>(&mut self) -> Result<F> {
        let token = self.peek_token().ok_or(Error::MalformedIdentity)?;
        let form = F::parse(token).ok_or(Error::MalformedIdentity)?;
        self.position += token.len();
        Ok(form)
    }

    /// The next tag, if `F` spells it.
    pub(super) fn peek<F: Vocabulary>(&self) -> Option<F> {
        self.peek_token().and_then(F::parse)
    }

    /// The tag after the comma at this position, if `F` spells it.
    pub(super) fn peek_after_comma<F: Vocabulary>(&self) -> Option<F> {
        self.next(b',').then_some(())?;
        Self {
            text: self.text,
            position: self.position + 1,
        }
        .peek()
    }

    /// The next tag, if `F` spells it and it opens a parenthesis.
    pub(super) fn peek_compound<F: Vocabulary>(&self) -> Option<F> {
        let token = self.peek_token()?;
        self.opens(token).then(|| F::parse(token)).flatten()
    }

    /// Whether any tag of the whole vocabulary opens a parenthesis here.
    pub(super) fn opens_vocabulary_compound(&self) -> bool {
        self.peek_token()
            .is_some_and(|token| self.opens(token) && is_vocabulary_spelling(token))
    }

    fn opens(&self, token: &str) -> bool {
        self.text.as_bytes().get(self.position + token.len()) == Some(&b'(')
    }

    /// Consume the next tag, which must be `tag`. Any other is left unread.
    pub(super) fn expect_tag(&mut self, tag: impl Spelled) -> Result {
        match self.peek_token() {
            Some(token) if token == tag.spelling() => {
                self.position += token.len();
                Ok(())
            }
            _ => Err(Error::MalformedIdentity),
        }
    }

    /// The parenthesized text of an atom whose tag is already consumed. The
    /// count is the number of decoded bytes, which escapes shorten.
    fn raw_value(&mut self) -> Result<(&'a str, usize)> {
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

    /// An atom named `tag` whose text carries no package owner.
    pub(super) fn opaque(
        &mut self,
        tag: impl Spelled,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        visitor.enter()?;
        let result = self
            .expect_tag(tag)
            .and_then(|()| self.raw_value())
            .map(|_| ());
        visitor.leave();
        result
    }

    /// The same, for an atom whose tag was already consumed as a production.
    pub(super) fn leaf_opaque(
        &mut self,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result {
        visitor.enter()?;
        let result = self.raw_value().map(|_| ());
        visitor.leave();
        result
    }

    /// An atom named `tag` whose unescaped text the caller reads further.
    pub(super) fn atom(
        &mut self,
        tag: impl Spelled,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result<Cow<'a, str>> {
        visitor.enter()?;
        let result = self
            .expect_tag(tag)
            .and_then(|()| self.unescaped_value(visitor));
        visitor.leave();
        result
    }

    pub(super) fn leaf_atom(
        &mut self,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result<Cow<'a, str>> {
        visitor.enter()?;
        let result = self.unescaped_value(visitor);
        visitor.leave();
        result
    }

    fn unescaped_value(
        &mut self,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result<Cow<'a, str>> {
        let (raw, decoded) = self.raw_value()?;
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
    }

    /// A length-prefixed hexadecimal byte string, after its tag was consumed.
    pub(super) fn leaf_byte_atom(
        &mut self,
        visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    ) -> Result<&'a str> {
        visitor.enter()?;
        let result = self.hexadecimal_value();
        visitor.leave();
        result
    }

    fn hexadecimal_value(&mut self) -> Result<&'a str> {
        let (raw, _) = self.raw_value()?;
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
    }
}
