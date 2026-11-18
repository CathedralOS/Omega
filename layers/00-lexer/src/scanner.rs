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

        // :HotLoop
        // Nine tokens in ten are whitespace, a word or punctuation, and each of those is decided
        // here from one table lookup with the position held in a local. Numbers, quoted literals
        // and block comments are rare and have more rules, so they go to the methods below, which
        // keep the position in `self.offset`; we hand it over before the call and take it back
        // after.
        let source_bytes = self.source_bytes;
        let mut offset = 0usize;

        while let Some(&first_byte) = source_bytes.get(offset) {
            let token_start = offset;

            let kind = match BYTE_CLASSES[usize::from(first_byte)] {
                ByteClass::Whitespace => {
                    // A plain loop on purpose. Most whitespace runs are a single byte, so the loop
                    // exits at once and is guessed right; the eight-byte window used for words
                    // was measured 15 percent slower here. See experiment 5 in EXPERIMENTS.md.
                    offset = find_run_end(source_bytes, token_start + 1, |byte| {
                        BYTE_CLASSES[usize::from(byte)] == ByteClass::Whitespace
                    });
                    TokenKind::Whitespace
                }
                ByteClass::WordStart => {
                    offset = find_word_end(source_bytes, token_start + 1);
                    // Only the one-letter word `r` can be the start of a raw literal, since the
                    // next byte has to be a quote or a hash and neither can continue a word.
                    if first_byte == b'r'
                        && offset == token_start + 1
                        && let Some(prefix_end) =
                            find_raw_literal_prefix_end(source_bytes, token_start)
                    {
                        return Err(LexError::new(
                            LexErrorKind::RawLiteralPrefix,
                            Span::new(token_start, prefix_end),
                        ));
                    }
                    TokenKind::Word
                }
                ByteClass::Punctuation => {
                    let Some(punctuation) =
                        match_punctuation_at(source_bytes, token_start, first_byte)
                    else {
                        return Err(unknown_character(token_start));
                    };
                    offset = token_start + punctuation.get_spelling().len();
                    TokenKind::Punctuation(punctuation)
                }
                // A slash is a comment when the next byte says so, and punctuation otherwise.
                ByteClass::Slash => match source_bytes.get(token_start + 1) {
                    Some(b'/') => {
                        // We stop before the line feed and let it join the next whitespace token.
                        offset = find_run_end(source_bytes, token_start + 2, |byte| byte != b'\n');
                        TokenKind::LineComment
                    }
                    Some(b'*') => {
                        self.offset = token_start;
                        let kind = self.scan_block_comment(token_start)?;
                        offset = self.offset;
                        kind
                    }
                    _ => {
                        let Some(punctuation) =
                            match_punctuation_at(source_bytes, token_start, first_byte)
                        else {
                            return Err(unknown_character(token_start));
                        };
                        offset = token_start + punctuation.get_spelling().len();
                        TokenKind::Punctuation(punctuation)
                    }
                },
                ByteClass::Digit => {
                    self.offset = token_start;
                    let kind = self.scan_number(token_start, first_byte)?;
                    offset = self.offset;
                    kind
                }
                ByteClass::Quote => {
                    self.offset = token_start;
                    let kind = self.scan_quoted(token_start)?;
                    offset = self.offset;
                    kind
                }
                ByteClass::NonAscii => {
                    let character_end = token_start + get_utf8_character_byte_count(first_byte);
                    return Err(LexError::new(
                        LexErrorKind::NonAsciiOutsideCommentOrQuoted,
                        Span::new(token_start, character_end.min(source_bytes.len())),
                    ));
                }
                ByteClass::Unknown => return Err(unknown_character(token_start)),
            };

            tokens.push(Token {
                kind,
                span: Span::new(token_start, offset),
            });
        }

        Ok(())
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

    fn peek(&self, bytes_ahead: usize) -> Option<u8> {
        self.source_bytes.get(self.offset + bytes_ahead).copied()
    }

    fn skip_while(&mut self, should_skip: impl Fn(u8) -> bool) {
        while self.peek(0).is_some_and(&should_skip) {
            self.offset += 1;
        }
    }
}

/// What the first byte of a token tells the scanner to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteClass {
    Whitespace,
    WordStart,
    Digit,
    Quote,
    Slash, // Kept apart from other punctuation because it may begin a comment.
    Punctuation,
    NonAscii,
    Unknown, // An ASCII byte that is not part of Omega's syntax, such as `@` or a control byte.
}

/// The class of every possible byte, so that choosing a token's rule is one lookup.
const BYTE_CLASSES: [ByteClass; 256] = build_byte_classes();

const fn build_byte_classes() -> [ByteClass; 256] {
    let mut classes = [ByteClass::Unknown; 256];
    let mut byte = 0usize;
    while byte < 256 {
        classes[byte] = match byte as u8 {
            b' ' | b'\t' | b'\r' | b'\n' => ByteClass::Whitespace,
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => ByteClass::WordStart,
            b'0'..=b'9' => ByteClass::Digit,
            b'"' => ByteClass::Quote,
            b'/' => ByteClass::Slash,
            // @Volatile: must list every first byte in `Punctuation::ORDERED_SPELLINGS` except the
            // slash. A test below fails if the two ever disagree.
            b'+' | b'-' | b'*' | b'%' | b'<' | b'>' | b'=' | b'!' | b'&' | b'|' | b':' | b'.'
            | b'\'' | b'^' | b'~' | b'#' | b',' | b';' | b'(' | b')' | b'[' | b']' | b'{'
            | b'}' => ByteClass::Punctuation,
            0x80..=0xFF => ByteClass::NonAscii,
            _ => ByteClass::Unknown,
        };
        byte += 1;
    }
    classes
}

/// Whether a byte can appear inside a word after its first byte: a letter, a digit or an underscore.
const IS_WORD_CONTINUE: [bool; 256] = build_is_word_continue();

const fn build_is_word_continue() -> [bool; 256] {
    let mut is_word_continue = [false; 256];
    let mut byte = 0usize;
    while byte < 256 {
        is_word_continue[byte] =
            matches!(byte as u8, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_');
        byte += 1;
    }
    is_word_continue
}

/// Returns the offset of the first byte at or after `run_start` that `continues_run` rejects,
/// or the end of the source if every remaining byte is accepted.
fn find_run_end(
    source_bytes: &[u8],
    run_start: usize,
    continues_run: impl Fn(u8) -> bool,
) -> usize {
    let Some(remaining_bytes) = source_bytes.get(run_start..) else {
        return source_bytes.len();
    };
    remaining_bytes
        .iter()
        .position(|&byte| !continues_run(byte))
        .map_or(source_bytes.len(), |run_byte_count| {
            run_start + run_byte_count
        })
}

/// Returns the offset of the first byte at or after `run_start` that cannot continue a word.
///
/// This does the same job as `find_run_end`, eight bytes at a time and without a loop that
/// stops on the data. A word's length is different every time, so a loop that exits at the
/// end of the word makes the processor guess the exit wrong about once per word, and a wrong
/// guess costs more than reading the whole word did. Here we look all eight bytes up, pack the
/// answers into a mask, and read the length off the mask; the only branch left asks whether the
/// word ran past the window, which is usually no.
fn find_word_end(source_bytes: &[u8], run_start: usize) -> usize {
    const WINDOW_BYTE_COUNT: usize = 8;

    let mut offset = run_start;
    while let Some(window) = source_bytes
        .get(offset..)
        .and_then(|remaining_bytes| remaining_bytes.first_chunk::<WINDOW_BYTE_COUNT>())
    {
        let mut continue_mask = 0u32;
        for (window_index, &byte) in window.iter().enumerate() {
            continue_mask |= u32::from(IS_WORD_CONTINUE[usize::from(byte)]) << window_index;
        }
        let word_byte_count = continue_mask.trailing_ones() as usize;
        if word_byte_count < WINDOW_BYTE_COUNT {
            return offset + word_byte_count;
        }
        offset += WINDOW_BYTE_COUNT;
    }

    // Fewer than eight bytes are left in the source; finish them one at a time.
    find_run_end(source_bytes, offset, |byte| {
        IS_WORD_CONTINUE[usize::from(byte)]
    })
}

/// Chooses the punctuation token that starts at `token_start`, whose first byte is `first_byte`.
fn match_punctuation_at(
    source_bytes: &[u8],
    token_start: usize,
    first_byte: u8,
) -> Option<Punctuation> {
    // Zero stands for "past the end of the source"; no punctuation spelling contains a zero byte.
    let second_byte = source_bytes.get(token_start + 1).copied().unwrap_or(0);
    let third_byte = source_bytes.get(token_start + 2).copied().unwrap_or(0);
    Punctuation::match_leading_bytes(first_byte, second_byte, third_byte)
}

/// If the source at `r_offset` is `r"` or `r#...#"`, returns the offset just past the opening quote.
fn find_raw_literal_prefix_end(source_bytes: &[u8], r_offset: usize) -> Option<usize> {
    let mut prefix_offset = r_offset + 1;
    while source_bytes.get(prefix_offset) == Some(&b'#') {
        prefix_offset += 1;
    }
    (source_bytes.get(prefix_offset) == Some(&b'"')).then_some(prefix_offset + 1)
}

fn unknown_character(character_offset: usize) -> LexError {
    LexError::new(
        LexErrorKind::UnknownCharacter,
        Span::new(character_offset, character_offset + 1),
    )
}

fn is_word_continue(byte: u8) -> bool {
    IS_WORD_CONTINUE[usize::from(byte)]
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

#[cfg(test)]
mod tests {
    use super::{BYTE_CLASSES, ByteClass, IS_WORD_CONTINUE};
    use crate::punctuation::Punctuation;

    #[test]
    fn every_punctuation_first_byte_is_classed_as_punctuation_or_slash() {
        for (spelling, _) in Punctuation::ORDERED_SPELLINGS {
            let first_byte = spelling.as_bytes()[0];
            let class = BYTE_CLASSES[usize::from(first_byte)];
            assert!(
                matches!(class, ByteClass::Punctuation | ByteClass::Slash),
                "`{spelling}` starts with a byte classed {class:?}, so the scanner would never try it"
            );
        }
    }

    #[test]
    fn every_byte_classed_as_punctuation_starts_some_spelling() {
        for byte in 0u8..=255 {
            if matches!(
                BYTE_CLASSES[usize::from(byte)],
                ByteClass::Punctuation | ByteClass::Slash
            ) {
                assert!(
                    Punctuation::longest_match(&[byte]).is_some(),
                    "byte {byte:#04x} is classed as punctuation but matches no spelling"
                );
            }
        }
    }

    #[test]
    fn a_word_start_can_also_continue_a_word_and_so_can_a_digit() {
        for byte in 0u8..=255 {
            let class = BYTE_CLASSES[usize::from(byte)];
            let expected = matches!(class, ByteClass::WordStart | ByteClass::Digit);
            assert_eq!(
                IS_WORD_CONTINUE[usize::from(byte)],
                expected,
                "byte {byte:#04x}"
            );
        }
    }
}
