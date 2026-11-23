//! The text form of the lexer's output that the golden fixtures compare against.
//!
//! This lives with the tests and not in the crate, because only the tests read it. It is written
//! against the lexer's public API alone, so the dump shows what any caller of the lexer can see.

use omega_lexer::{LexError, LineIndex, Token, TokenKind, decode_quoted_bytes};
use std::fmt::Write;

/*
    One line per token: where it starts, what kind it is, how it is spelled,
    and for literals the parts the lexer worked out.

        3:17     Integer      0xFFFFu16  radix=16 digits=FFFF suffix=u16

    Whitespace tokens are left out. They are in the token list, and the tiling
    test checks them, but printing them would bury every dump in lines that say
    nothing; where the whitespace was is visible from the positions.

    The dump is plain ASCII. A byte outside the printable range is written as
    `\x{NN}`, with braces so it cannot be mistaken for a `\xNN` escape that was
    really in the source.
*/

/// Renders accepted tokens for a golden fixture. Whitespace tokens are omitted.
pub fn dump_tokens(
    source_bytes: &[u8],
    tokens: &[Token],
) -> String {
    let line_index = LineIndex::new(source_bytes);
    let mut token_dump = String::new();

    for token in tokens {
        let token_kind = token.get_kind();
        let token_span = token.get_span();

        if token_kind == TokenKind::Whitespace {
            continue;
        }

        let source_position = line_index.get_position(token_span.get_start_offset());
        let location = format!("{}:{}", source_position.get_line(), source_position.get_column());
        let spelling_bytes = token_span.slice(source_bytes).unwrap_or_default();
        let spelling = escape_for_dump(spelling_bytes);
        let kind_label = token_kind.get_name();

        // Writing to a String cannot fail, so the results are discarded.
        let _ = write!(token_dump, "{location:<8} {kind_label:<12} {spelling}");

        match token_kind {
            TokenKind::Integer { radix, suffix_start_offset } => {
                let digits_start = token_span.get_start_offset() + radix.get_prefix_byte_count();
                let digits = escape_for_dump(
                    source_bytes
                        .get(digits_start..suffix_start_offset)
                        .unwrap_or_default(),
                );

                let _ = write!(token_dump, "  radix={} digits={digits}", radix.get_base());

                write_suffix(&mut token_dump, source_bytes, suffix_start_offset, token_span.get_end_offset());
            }
            TokenKind::Float { suffix_start_offset } => {
                let digits = escape_for_dump(
                    source_bytes
                        .get(token_span.get_start_offset()..suffix_start_offset)
                        .unwrap_or_default(),
                );

                let _ = write!(token_dump, "  digits={digits}");

                write_suffix(&mut token_dump, source_bytes, suffix_start_offset, token_span.get_end_offset());
            }
            TokenKind::Quoted => match decode_quoted_bytes(spelling_bytes) {
                Some(decoded_bytes) => {
                    let _ = write!(token_dump, "  bytes=[");

                    for (decoded_byte_index, decoded_byte) in decoded_bytes.iter().enumerate() {
                        let separator = if decoded_byte_index == 0 { "" } else { " " };
                        let _ = write!(token_dump, "{separator}{decoded_byte:02x}");
                    }

                    let _ = write!(token_dump, "]");
                }
                None => {
                    let _ = write!(token_dump, "  bytes=<undecodable>");
                }
            },
            _ => {}
        }

        token_dump.push('\n');
    }

    token_dump
}

/// Renders a refusal for a golden fixture: the rule broken, where, and the bytes at fault.
pub fn dump_error(
    source_bytes: &[u8],
    lex_error: &LexError,
) -> String {
    let error_kind = lex_error.get_kind();
    let error_span = lex_error.get_span();
    let source_position = LineIndex::new(source_bytes).get_position(error_span.get_start_offset());
    let offending_bytes = escape_for_dump(error_span.slice(source_bytes).unwrap_or_default());

    format!(
        "error {:?} at {}:{} bytes {}..{}\n  text: {}\n  {}\n",
        error_kind,
        source_position.get_line(),
        source_position.get_column(),
        error_span.get_start_offset(),
        error_span.get_end_offset(),
        offending_bytes,
        error_kind.get_message()
    )
}

fn write_suffix(
    token_dump: &mut String,
    source_bytes: &[u8],
    suffix_start_offset: usize,
    token_end: usize,
) {
    if suffix_start_offset < token_end {
        let suffix = escape_for_dump(
            source_bytes
                .get(suffix_start_offset..token_end)
                .unwrap_or_default(),
        );

        let _ = write!(token_dump, " suffix={suffix}");
    }
}

fn escape_for_dump(raw_bytes: &[u8]) -> String {
    let mut escaped_text = String::with_capacity(raw_bytes.len());

    for &raw_byte in raw_bytes {
        if (0x20..=0x7E).contains(&raw_byte) {
            escaped_text.push(char::from(raw_byte));
        } else {
            let _ = write!(escaped_text, "\\x{{{raw_byte:02x}}}");
        }
    }

    escaped_text
}
