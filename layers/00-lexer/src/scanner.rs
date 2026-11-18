//! The byte-level scanner: one pass over the source that produces every token or the first error.

use crate::lex_error::{LexError, LexErrorKind};
use crate::punctuation::Punctuation;
use crate::quoted_bytes::{EscapeProblem, read_escape};
use crate::span::Span;
use crate::token::{Radix, Token, TokenKind};

pub(crate) struct Scanner<'source> {
    source_bytes: &'source [u8], // Already checked to be valid UTF-8 by `lex`.
    offset: usize, // The next byte to read. Everything before it is inside an emitted token.
}

impl<'source> Scanner<'source> {
    pub(crate) fn new(source_bytes: &'source [u8]) -> Self {
        Self {
            source_bytes,
            offset: 0,
        }
    }

    /// Appends every token of the source to `tokens`, which the caller has already emptied.
    pub(crate) fn scan_all(mut self, tokens: &mut Vec<Token>) -> Result<(), LexError> {
        // Measured on 26 MB of Omega source: one token per 3.4 bytes, because whitespace and
        // punctuation are tokens and are mostly one byte long. Reserving a token per 3 bytes
        // covers the typical file without the list having to grow and copy itself. A list that
        // is being reused usually has the room already, and then this does nothing.
        tokens.reserve(self.source_bytes.len() / 3 + 16);

        while let Some(first_byte) = self.peek(0) {
            let token_start = self.offset;
            let kind = self.scan_token(token_start, first_byte)?;
            tokens.push(Token {
                kind,
                span: Span::new(token_start, self.offset),
            });
        }

        Ok(())
    }

    /// Scans the token that begins at `token_start`, leaving `offset` one past its last byte.
    fn scan_token(&mut self, token_start: usize, first_byte: u8) -> Result<TokenKind, LexError> {
        match first_byte {
            byte if is_whitespace(byte) => {
                self.skip_while(is_whitespace);
                Ok(TokenKind::Whitespace)
            }
            // Comments are tested before punctuation, or `//` would lex as two slashes.
            b'/' if self.peek(1) == Some(b'/') => {
                // We stop before the line feed and let it join the next whitespace token.
                self.skip_while(|byte| byte != b'\n');
                Ok(TokenKind::LineComment)
            }
            b'/' if self.peek(1) == Some(b'*') => self.scan_block_comment(token_start),
            b'"' => self.scan_quoted(token_start),
            b'0'..=b'9' => self.scan_number(token_start, first_byte),
            b'r' if self.get_raw_literal_prefix_end().is_some() => {
                let prefix_end = self.get_raw_literal_prefix_end().unwrap_or(self.offset + 1);
                Err(LexError::new(
                    LexErrorKind::RawLiteralPrefix,
                    Span::new(token_start, prefix_end),
                ))
            }
            byte if is_word_start(byte) => {
                self.skip_while(is_word_continue);
                Ok(TokenKind::Word)
            }
            byte if !byte.is_ascii() => {
                let character_end = token_start + get_utf8_character_byte_count(byte);
                Err(LexError::new(
                    LexErrorKind::NonAsciiOutsideCommentOrQuoted,
                    Span::new(token_start, character_end.min(self.source_bytes.len())),
                ))
            }
            _ => match Punctuation::longest_match(&self.source_bytes[token_start..]) {
                Some(punctuation) => {
                    self.offset += punctuation.get_spelling().len();
                    Ok(TokenKind::Punctuation(punctuation))
                }
                None => Err(LexError::new(
                    LexErrorKind::UnknownCharacter,
                    Span::new(token_start, token_start + 1),
                )),
            },
        }
    }

    fn scan_block_comment(&mut self, token_start: usize) -> Result<TokenKind, LexError> {
        self.offset += 2;
        let mut open_comment_count = 1usize;

        // Only the two delimiters matter. Every other byte, including non-ASCII ones, is skipped unread.
        while let Some(byte) = self.peek(0) {
            match (byte, self.peek(1)) {
                (b'/', Some(b'*')) => {
                    self.offset += 2;
                    open_comment_count += 1;
                }
                (b'*', Some(b'/')) => {
                    self.offset += 2;
                    open_comment_count -= 1;
                    if open_comment_count == 0 {
                        return Ok(TokenKind::BlockComment);
                    }
                }
                _ => self.offset += 1,
            }
        }

        Err(LexError::new(
            LexErrorKind::UnterminatedBlockComment,
            Span::new(token_start, self.source_bytes.len()),
        ))
    }

    fn scan_quoted(&mut self, token_start: usize) -> Result<TokenKind, LexError> {
        self.offset += 1;

        loop {
            let Some(byte) = self.peek(0) else {
                return Err(self.unterminated_quoted(token_start));
            };

            match byte {
                b'"' => {
                    self.offset += 1;
                    return Ok(TokenKind::Quoted);
                }
                // The spec rejects raw line breaks so a literal's bytes do not depend on checkout line endings.
                b'\n' | b'\r' => {
                    return Err(LexError::new(
                        LexErrorKind::NewlineInQuoted,
                        Span::new(self.offset, self.offset + 1),
                    ));
                }
                b'\\' => match read_escape(&self.source_bytes[self.offset..]) {
                    Ok((_, spelled_byte_count)) => self.offset += spelled_byte_count,
                    Err(escape_problem) => {
                        return Err(self.escape_error(token_start, escape_problem));
                    }
                },
                _ => self.offset += 1,
            }
        }
    }

    fn unterminated_quoted(&self, token_start: usize) -> LexError {
        LexError::new(
            LexErrorKind::UnterminatedQuoted,
            Span::new(token_start, self.source_bytes.len()),
        )
    }

    /// Builds the error for a bad escape whose backslash is at `offset`.
    fn escape_error(&self, token_start: usize, escape_problem: EscapeProblem) -> LexError {
        let backslash_offset = self.offset;
        match escape_problem {
            EscapeProblem::EndOfSource => self.unterminated_quoted(token_start),
            EscapeProblem::Codepoint => LexError::new(
                LexErrorKind::CodepointEscape,
                Span::new(backslash_offset, backslash_offset + 2),
            ),
            // We point at `\x` only. The two bytes after it may be anything, including half a character.
            EscapeProblem::InvalidHex => LexError::new(
                LexErrorKind::InvalidHexEscape,
                Span::new(backslash_offset, backslash_offset + 2),
            ),
            EscapeProblem::Unknown => {
                let escaped_character_byte_count = self
                    .source_bytes
                    .get(backslash_offset + 1)
                    .map_or(0, |&escaped_byte| {
                        get_utf8_character_byte_count(escaped_byte)
                    });
                let error_end = (backslash_offset + 1 + escaped_character_byte_count)
                    .min(self.source_bytes.len());
                LexError::new(
                    LexErrorKind::UnknownEscape,
                    Span::new(backslash_offset, error_end),
                )
            }
        }
    }

    fn scan_number(&mut self, token_start: usize, first_digit: u8) -> Result<TokenKind, LexError> {
        let radix = match (first_digit, self.peek(1)) {
            (b'0', Some(b'x')) => Radix::Hexadecimal,
            (b'0', Some(b'b')) => Radix::Binary,
            (b'0', Some(b'o')) => Radix::Octal,
            _ => Radix::Decimal,
        };

        if radix != Radix::Decimal {
            self.offset += radix.get_prefix_byte_count();
            if !self.skip_digits(radix) {
                return Err(LexError::new(
                    LexErrorKind::MissingRadixDigits,
                    Span::new(token_start, self.offset),
                ));
            }
            // Without this, `0b102` would silently lex as the two numbers `0b10` and `2`.
            if self.peek(0).is_some_and(|byte| byte.is_ascii_digit()) {
                return Err(LexError::new(
                    LexErrorKind::DigitOutsideRadix,
                    Span::new(self.offset, self.offset + 1),
                ));
            }
            let suffix_start = self.scan_suffix()?;
            return Ok(TokenKind::Integer {
                radix,
                suffix_start,
            });
        }

        self.skip_digits(Radix::Decimal);
        let mut is_float = false;

        // A fraction needs a digit after the dot. That keeps `1..2` a range and `1.foo` a member access.
        if self.peek(0) == Some(b'.') && self.peek(1).is_some_and(|byte| byte.is_ascii_digit()) {
            is_float = true;
            self.offset += 1;
            self.skip_digits(Radix::Decimal);
        }

        if self.peek(0) == Some(b'e') {
            let has_sign = matches!(self.peek(1), Some(b'+' | b'-'));
            let exponent_digits_offset = if has_sign { 2 } else { 1 };
            if self
                .peek(exponent_digits_offset)
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                is_float = true;
                self.offset += exponent_digits_offset;
                self.skip_digits(Radix::Decimal);
            } else if has_sign {
                return Err(LexError::new(
                    LexErrorKind::MissingExponentDigits,
                    Span::new(token_start, self.offset + 2),
                ));
            }
            // Otherwise the `e` is not an exponent. It falls through to the suffix check, which rejects it.
        }

        let suffix_start = self.scan_suffix()?;
        if is_float {
            Ok(TokenKind::Float { suffix_start })
        } else {
            Ok(TokenKind::Integer {
                radix,
                suffix_start,
            })
        }
    }

    /// Skips digits of `radix` and underscores. Returns whether at least one digit was seen.
    fn skip_digits(&mut self, radix: Radix) -> bool {
        let mut saw_digit = false;
        while let Some(byte) = self.peek(0) {
            if byte == b'_' {
                self.offset += 1;
            } else if char::from(byte).is_digit(radix.get_base()) {
                saw_digit = true;
                self.offset += 1;
            } else {
                break;
            }
        }
        saw_digit
    }

    /// Consumes the letters directly after a number and checks they are a valid suffix.
    /// Returns the offset where the suffix starts, which is the current offset when there is none.
    fn scan_suffix(&mut self) -> Result<usize, LexError> {
        let suffix_start = self.offset;
        self.skip_while(is_word_continue);
        let suffix_bytes = &self.source_bytes[suffix_start..self.offset];

        if suffix_bytes.is_empty() || is_valid_literal_suffix(suffix_bytes) {
            Ok(suffix_start)
        } else {
            Err(LexError::new(
                LexErrorKind::InvalidLiteralSuffix,
                Span::new(suffix_start, self.offset),
            ))
        }
    }

    /// If the source at `offset` is `r"` or `r#...#"`, returns the offset just past the opening quote.
    fn get_raw_literal_prefix_end(&self) -> Option<usize> {
        let mut prefix_offset = self.offset + 1;
        while self.source_bytes.get(prefix_offset) == Some(&b'#') {
            prefix_offset += 1;
        }
        (self.source_bytes.get(prefix_offset) == Some(&b'"')).then_some(prefix_offset + 1)
    }

    fn peek(&self, bytes_ahead: usize) -> Option<u8> {
        self.source_bytes.get(self.offset + bytes_ahead).copied()
    }

    fn skip_while(&mut self, should_skip: impl Fn(u8) -> bool) {
        while self.peek(0).is_some_and(&should_skip) {
            self.offset += 1;
        }
    }
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\r' | b'\n')
}

fn is_word_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_word_continue(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphanumeric()
}

/// `i`, `u`, `i<width>`, `u<width>`, `f32`, `f64`. A width is positive decimal with no leading zero.
fn is_valid_literal_suffix(suffix_bytes: &[u8]) -> bool {
    match suffix_bytes {
        b"f32" | b"f64" => true,
        [b'i' | b'u'] => true,
        [b'i' | b'u', first_width_digit, remaining_width_digits @ ..] => {
            matches!(first_width_digit, b'1'..=b'9')
                && remaining_width_digits.iter().all(u8::is_ascii_digit)
        }
        _ => false,
    }
}

/// How many bytes the UTF-8 character starting with `leading_byte` occupies.
/// The source is known to be valid UTF-8, so the leading byte alone decides.
fn get_utf8_character_byte_count(leading_byte: u8) -> usize {
    match leading_byte {
        0x00..=0x7F => 1,
        0x80..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xFF => 4,
    }
}
