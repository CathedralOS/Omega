//! The byte-level scanner: one pass over the source that produces every token or the first error.

use crate::errors::lex_error::LexError;
use crate::errors::lex_error_kind::LexErrorKind;
use crate::lexing::byte_class::{BYTE_CLASSES, ByteClass, IS_WORD_CONTINUE};
use crate::lexing::quoted_escape_error::QuotedEscapeError;
use crate::lexing::quoted_literal_decoder::read_escape;
use crate::locations::source_span::SourceSpan;
use crate::tokens::integer_radix::IntegerRadix;
use crate::tokens::punctuation::Punctuation;
use crate::tokens::token::Token;
use crate::tokens::token_kind::TokenKind;

pub(crate) struct TokenScanner<'source> {
    source_bytes: &'source [u8], // Already checked to be valid UTF-8 by `lex_into`.
    scan_offset: usize,          // The next byte to read. Everything before it is inside an emitted token.
}

impl<'source> TokenScanner<'source> {
    pub(crate) fn new(source_bytes: &'source [u8]) -> Self {
        Self { source_bytes, scan_offset: 0 }
    }

    /// Appends every token of the source to `tokens`, which the caller has already emptied.
    pub(crate) fn scan_all(
        mut self,
        tokens: &mut Vec<Token>,
    ) -> Result<(), LexError> {
        // Measured on 26 MB of Omega source: one token per 3.4 bytes, because whitespace and
        // punctuation are tokens and are mostly one byte long. Reserving a token per 3 bytes
        // covers the typical file without the list having to grow and copy itself. A list that
        // is being reused usually has the room already, and then this does nothing.
        tokens.reserve(self.source_bytes.len() / 3 + 16);

        // :HotLoop
        // Nine tokens in ten are whitespace, a word or punctuation, and each of those is decided
        // here from one table lookup with the position held in a local. Numbers, quoted literals
        // and block comments are rare and have more rules, so they go to the methods below, which
        // keep the position in `self.scan_offset` and hand back where they stopped.
        let source_bytes = self.source_bytes;
        let mut scan_offset = 0usize;

        while let Some(&first_byte) = source_bytes.get(scan_offset) {
            let token_start = scan_offset;
            let (token_kind, token_end) = match BYTE_CLASSES[usize::from(first_byte)] {
                ByteClass::Whitespace => {
                    // A plain loop on purpose. Most whitespace runs are a single byte, so the loop
                    // exits at once and is guessed right; the eight-byte window used for words
                    // was measured 15 percent slower here. See experiment 5 in EXPERIMENTS.md.
                    let whitespace_end = find_run_end(source_bytes, token_start + 1, |source_byte| {
                        BYTE_CLASSES[usize::from(source_byte)] == ByteClass::Whitespace
                    });

                    (TokenKind::Whitespace, whitespace_end)
                }
                ByteClass::WordStart => {
                    let word_end = find_word_end(source_bytes, token_start + 1);

                    // Only the one-letter word `r` can be the start of a raw literal, since the
                    // next byte has to be a quote or a hash and neither can continue a word.
                    if first_byte == b'r'
                        && word_end == token_start + 1
                        && let Some(raw_prefix_end) = find_raw_literal_prefix_end(source_bytes, token_start)
                    {
                        return Err(LexError::new(LexErrorKind::RawLiteralPrefix, SourceSpan::new(token_start, raw_prefix_end)));
                    }

                    (TokenKind::Word, word_end)
                }
                ByteClass::Punctuation => {
                    let Some(punctuation) = match_punctuation_at(source_bytes, token_start, first_byte) else {
                        return Err(unknown_character(token_start));
                    };

                    (TokenKind::Punctuation(punctuation), token_start + punctuation.get_spelling().len())
                }
                // A slash is a comment when the next byte says so, and punctuation otherwise.
                ByteClass::Slash => match source_bytes.get(token_start + 1) {
                    // We stop before the line feed and let it join the next whitespace token.
                    Some(b'/') => (
                        TokenKind::LineComment,
                        find_run_end(source_bytes, token_start + 2, |source_byte| source_byte != b'\n'),
                    ),
                    Some(b'*') => self.scan_block_comment(token_start)?,
                    // @Speed: the same lines as the punctuation case above, written out twice on
                    // purpose. Sharing them through one function measured about 1.5 percent
                    // slower in every build we tried. See experiment 11 in EXPERIMENTS.md.
                    _ => {
                        let Some(punctuation) = match_punctuation_at(source_bytes, token_start, first_byte) else {
                            return Err(unknown_character(token_start));
                        };

                        (TokenKind::Punctuation(punctuation), token_start + punctuation.get_spelling().len())
                    }
                },
                ByteClass::Digit => self.scan_number(token_start, first_byte)?,
                ByteClass::Quote => self.scan_quoted(token_start)?,
                ByteClass::NonAscii => {
                    let character_end = token_start + get_utf8_character_byte_count(first_byte);

                    return Err(LexError::new(
                        LexErrorKind::NonAsciiOutsideCommentOrQuoted,
                        SourceSpan::new(token_start, character_end.min(source_bytes.len())),
                    ));
                }
                ByteClass::Unknown => return Err(unknown_character(token_start)),
            };

            tokens.push(Token::new(token_kind, SourceSpan::new(token_start, token_end)));
            scan_offset = token_end;
        }

        Ok(())
    }

    fn scan_block_comment(
        &mut self,
        token_start: usize,
    ) -> Result<(TokenKind, usize), LexError> {
        self.scan_offset = token_start + 2;

        let mut open_comment_count = 1usize;

        // Only the two delimiters matter. Every other byte, including non-ASCII ones, is skipped unread.
        while let Some(current_byte) = self.peek(0) {
            match (current_byte, self.peek(1)) {
                (b'/', Some(b'*')) => {
                    self.scan_offset += 2;
                    open_comment_count += 1;
                }
                (b'*', Some(b'/')) => {
                    self.scan_offset += 2;
                    open_comment_count -= 1;

                    if open_comment_count == 0 {
                        return Ok((TokenKind::BlockComment, self.scan_offset));
                    }
                }
                _ => self.scan_offset += 1,
            }
        }

        Err(LexError::new(
            LexErrorKind::UnterminatedBlockComment,
            SourceSpan::new(token_start, self.source_bytes.len()),
        ))
    }

    fn scan_quoted(
        &mut self,
        token_start: usize,
    ) -> Result<(TokenKind, usize), LexError> {
        self.scan_offset = token_start + 1;

        loop {
            let Some(current_byte) = self.peek(0) else {
                return Err(self.unterminated_quoted(token_start));
            };

            match current_byte {
                b'"' => {
                    self.scan_offset += 1;

                    return Ok((TokenKind::Quoted, self.scan_offset));
                }
                // The spec rejects raw line breaks so a literal's bytes do not depend on checkout line endings.
                b'\n' | b'\r' => {
                    return Err(LexError::new(
                        LexErrorKind::NewlineInQuoted,
                        SourceSpan::new(self.scan_offset, self.scan_offset + 1),
                    ));
                }
                b'\\' => match read_escape(&self.source_bytes[self.scan_offset..]) {
                    Ok((_, spelled_byte_count)) => self.scan_offset += spelled_byte_count,
                    Err(quoted_escape_error) => {
                        return Err(self.escape_error(token_start, quoted_escape_error));
                    }
                },
                _ => self.scan_offset += 1,
            }
        }
    }

    fn unterminated_quoted(
        &self,
        token_start: usize,
    ) -> LexError {
        LexError::new(LexErrorKind::UnterminatedQuoted, SourceSpan::new(token_start, self.source_bytes.len()))
    }

    /// Builds the error for a bad escape whose backslash is at `scan_offset`.
    fn escape_error(
        &self,
        token_start: usize,
        quoted_escape_error: QuotedEscapeError,
    ) -> LexError {
        let backslash_offset = self.scan_offset;

        match quoted_escape_error {
            QuotedEscapeError::EndOfSource => self.unterminated_quoted(token_start),
            QuotedEscapeError::Codepoint => LexError::new(LexErrorKind::CodepointEscape, SourceSpan::new(backslash_offset, backslash_offset + 2)),
            // We point at `\x` only. The two bytes after it may be anything, including half a character.
            QuotedEscapeError::InvalidHex => LexError::new(LexErrorKind::InvalidHexEscape, SourceSpan::new(backslash_offset, backslash_offset + 2)),
            QuotedEscapeError::Unknown => {
                let escaped_character_byte_count = self
                    .source_bytes
                    .get(backslash_offset + 1)
                    .map_or(0, |&escaped_byte| get_utf8_character_byte_count(escaped_byte));
                let error_end = (backslash_offset + 1 + escaped_character_byte_count).min(self.source_bytes.len());

                LexError::new(LexErrorKind::UnknownEscape, SourceSpan::new(backslash_offset, error_end))
            }
        }
    }

    fn scan_number(
        &mut self,
        token_start: usize,
        first_digit: u8,
    ) -> Result<(TokenKind, usize), LexError> {
        self.scan_offset = token_start;

        let radix = match (first_digit, self.peek(1)) {
            (b'0', Some(b'x')) => IntegerRadix::Hexadecimal,
            (b'0', Some(b'b')) => IntegerRadix::Binary,
            (b'0', Some(b'o')) => IntegerRadix::Octal,
            _ => IntegerRadix::Decimal,
        };

        if radix != IntegerRadix::Decimal {
            self.scan_offset += radix.get_prefix_byte_count();

            if !self.skip_digits(radix) {
                return Err(LexError::new(LexErrorKind::MissingRadixDigits, SourceSpan::new(token_start, self.scan_offset)));
            }

            // Without this, `0b102` would silently lex as the two numbers `0b10` and `2`.
            if self
                .peek(0)
                .is_some_and(|following_byte| following_byte.is_ascii_digit())
            {
                return Err(LexError::new(
                    LexErrorKind::DigitOutsideRadix,
                    SourceSpan::new(self.scan_offset, self.scan_offset + 1),
                ));
            }

            let suffix_start_offset = self.scan_suffix()?;

            return Ok((TokenKind::Integer { radix, suffix_start_offset }, self.scan_offset));
        }

        self.skip_digits(IntegerRadix::Decimal);

        let mut is_float = false;

        // A fraction needs a digit after the dot. That keeps `1..2` a range and `1.foo` a member access.
        if self.peek(0) == Some(b'.')
            && self
                .peek(1)
                .is_some_and(|fraction_byte| fraction_byte.is_ascii_digit())
        {
            is_float = true;
            self.scan_offset += 1;
            self.skip_digits(IntegerRadix::Decimal);
        }

        if self.peek(0) == Some(b'e') {
            let has_sign = matches!(self.peek(1), Some(b'+' | b'-'));
            let exponent_digits_offset = if has_sign { 2 } else { 1 };

            if self
                .peek(exponent_digits_offset)
                .is_some_and(|exponent_byte| exponent_byte.is_ascii_digit())
            {
                is_float = true;
                self.scan_offset += exponent_digits_offset;
                self.skip_digits(IntegerRadix::Decimal);
            } else if has_sign {
                return Err(LexError::new(
                    LexErrorKind::MissingExponentDigits,
                    SourceSpan::new(token_start, self.scan_offset + 2),
                ));
            }
            // Otherwise the `e` is not an exponent. It falls through to the suffix check, which rejects it.
        }

        let suffix_start_offset = self.scan_suffix()?;

        let token_kind = if is_float {
            TokenKind::Float { suffix_start_offset }
        } else {
            TokenKind::Integer { radix, suffix_start_offset }
        };

        Ok((token_kind, self.scan_offset))
    }

    /// Skips digits of `radix` and underscores. Returns whether at least one digit was seen.
    fn skip_digits(
        &mut self,
        radix: IntegerRadix,
    ) -> bool {
        let mut saw_digit = false;

        while let Some(current_byte) = self.peek(0) {
            if current_byte == b'_' {
                self.scan_offset += 1;
            } else if char::from(current_byte).is_digit(radix.get_base()) {
                saw_digit = true;
                self.scan_offset += 1;
            } else {
                break;
            }
        }

        saw_digit
    }

    /// Consumes the letters directly after a number and checks they are a valid suffix.
    /// Returns the offset where the suffix starts, which is the current offset when there is none.
    fn scan_suffix(&mut self) -> Result<usize, LexError> {
        let suffix_start_offset = self.scan_offset;

        self.skip_while(is_word_continue);

        let suffix_bytes = &self.source_bytes[suffix_start_offset..self.scan_offset];

        if suffix_bytes.is_empty() || is_valid_literal_suffix(suffix_bytes) {
            Ok(suffix_start_offset)
        } else {
            Err(LexError::new(
                LexErrorKind::InvalidLiteralSuffix,
                SourceSpan::new(suffix_start_offset, self.scan_offset),
            ))
        }
    }

    fn peek(
        &self,
        bytes_ahead: usize,
    ) -> Option<u8> {
        self.source_bytes.get(self.scan_offset + bytes_ahead).copied()
    }

    fn skip_while(
        &mut self,
        should_skip: impl Fn(u8) -> bool,
    ) {
        while self.peek(0).is_some_and(&should_skip) {
            self.scan_offset += 1;
        }
    }
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
        .position(|&source_byte| !continues_run(source_byte))
        .map_or(source_bytes.len(), |run_byte_count| run_start + run_byte_count)
}

/// Returns the offset of the first byte at or after `run_start` that cannot continue a word.
///
/// This does the same job as `find_run_end`, eight bytes at a time and without a loop that
/// stops on the data. A word's length is different every time, so a loop that exits at the
/// end of the word makes the processor guess the exit wrong about once per word, and a wrong
/// guess costs more than reading the whole word did. Here we look all eight bytes up, pack the
/// answers into a mask, and read the length off the mask; the only branch left asks whether the
/// word ran past the window, which is usually no.
fn find_word_end(
    source_bytes: &[u8],
    run_start: usize,
) -> usize {
    const WINDOW_BYTE_COUNT: usize = 8;

    let mut window_offset = run_start;

    while let Some(window_bytes) = source_bytes
        .get(window_offset..)
        .and_then(|remaining_bytes| remaining_bytes.first_chunk::<WINDOW_BYTE_COUNT>())
    {
        let mut continue_mask = 0u32;

        for (window_byte_index, &window_byte) in window_bytes.iter().enumerate() {
            continue_mask |= u32::from(IS_WORD_CONTINUE[usize::from(window_byte)]) << window_byte_index;
        }

        let word_byte_count = continue_mask.trailing_ones() as usize;

        if word_byte_count < WINDOW_BYTE_COUNT {
            return window_offset + word_byte_count;
        }

        window_offset += WINDOW_BYTE_COUNT;
    }

    // Fewer than eight bytes are left in the source; finish them one at a time.
    find_run_end(source_bytes, window_offset, is_word_continue)
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
fn find_raw_literal_prefix_end(
    source_bytes: &[u8],
    r_offset: usize,
) -> Option<usize> {
    let mut prefix_offset = r_offset + 1;

    while source_bytes.get(prefix_offset) == Some(&b'#') {
        prefix_offset += 1;
    }

    (source_bytes.get(prefix_offset) == Some(&b'"')).then_some(prefix_offset + 1)
}

fn unknown_character(character_offset: usize) -> LexError {
    LexError::new(LexErrorKind::UnknownCharacter, SourceSpan::new(character_offset, character_offset + 1))
}

fn is_word_continue(source_byte: u8) -> bool {
    IS_WORD_CONTINUE[usize::from(source_byte)]
}

/// `i`, `u`, `i<width>`, `u<width>`, `f32`, `f64`. A width is positive decimal with no leading zero.
fn is_valid_literal_suffix(suffix_bytes: &[u8]) -> bool {
    match suffix_bytes {
        b"f32" | b"f64" => true,
        [b'i' | b'u'] => true,
        [b'i' | b'u', first_width_digit, remaining_width_digits @ ..] => {
            matches!(first_width_digit, b'1'..=b'9') && remaining_width_digits.iter().all(u8::is_ascii_digit)
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
