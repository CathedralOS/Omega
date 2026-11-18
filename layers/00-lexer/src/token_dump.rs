//! The text form of the lexer's output that the golden fixtures compare against.

use crate::lex_error::LexError;
use crate::quoted_bytes::decode_quoted_bytes;
use crate::source_position::LineIndex;
use crate::token::{Token, TokenKind};
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
pub fn dump_tokens(source_bytes: &[u8], tokens: &[Token]) -> String {
    let line_index = LineIndex::new(source_bytes);
    let mut dump = String::new();

    for token in tokens {
        if token.kind == TokenKind::Whitespace {
            continue;
        }

        let position = line_index.get_position(token.span.start);
        let location = format!("{}:{}", position.line, position.column);
        let spelling_bytes = token.span.slice(source_bytes).unwrap_or_default();
        let spelling = escape_for_dump(spelling_bytes);
        let kind_label = get_kind_label(token.kind);

        // Writing to a String cannot fail, so the results are discarded.
        let _ = write!(dump, "{location:<8} {kind_label:<12} {spelling}");
        match token.kind {
            TokenKind::Integer {
                radix,
                suffix_start,
            } => {
                let digits_start = token.span.start + radix.get_prefix_byte_count();
                let digits = escape_for_dump(
                    source_bytes
                        .get(digits_start..suffix_start)
                        .unwrap_or_default(),
                );
                let _ = write!(dump, "  radix={} digits={digits}", radix.get_base());
                write_suffix(&mut dump, source_bytes, suffix_start, token.span.end);
            }
            TokenKind::Float { suffix_start } => {
                let digits = escape_for_dump(
                    source_bytes
                        .get(token.span.start..suffix_start)
                        .unwrap_or_default(),
                );
                let _ = write!(dump, "  digits={digits}");
                write_suffix(&mut dump, source_bytes, suffix_start, token.span.end);
            }
            TokenKind::Quoted => match decode_quoted_bytes(spelling_bytes) {
                Some(decoded_bytes) => {
                    let _ = write!(dump, "  bytes=[");
                    for (byte_index, decoded_byte) in decoded_bytes.iter().enumerate() {
                        let separator = if byte_index == 0 { "" } else { " " };
                        let _ = write!(dump, "{separator}{decoded_byte:02x}");
                    }
                    let _ = write!(dump, "]");
                }
                None => {
                    let _ = write!(dump, "  bytes=<undecodable>");
                }
            },
            _ => {}
        }
        dump.push('\n');
    }

    dump
}

/// Renders a refusal for a golden fixture: the rule broken, where, and the bytes at fault.
pub fn dump_error(source_bytes: &[u8], error: &LexError) -> String {
    let position = LineIndex::new(source_bytes).get_position(error.span.start);
    let offending_bytes = escape_for_dump(error.span.slice(source_bytes).unwrap_or_default());
    format!(
        "error {:?} at {}:{} bytes {}..{}\n  text: {}\n  {}\n",
        error.kind,
        position.line,
        position.column,
        error.span.start,
        error.span.end,
        offending_bytes,
        error.kind.get_message()
    )
}

fn write_suffix(dump: &mut String, source_bytes: &[u8], suffix_start: usize, token_end: usize) {
    if suffix_start < token_end {
        let suffix = escape_for_dump(
            source_bytes
                .get(suffix_start..token_end)
                .unwrap_or_default(),
        );
        let _ = write!(dump, " suffix={suffix}");
    }
}

fn get_kind_label(kind: TokenKind) -> &'static str {
    match kind {
        TokenKind::Whitespace => "Whitespace",
        TokenKind::LineComment => "LineComment",
        TokenKind::BlockComment => "BlockComment",
        TokenKind::Word => "Word",
        TokenKind::Integer { .. } => "Integer",
        TokenKind::Float { .. } => "Float",
        TokenKind::Quoted => "Quoted",
        TokenKind::Punctuation(_) => "Punctuation",
    }
}

fn escape_for_dump(bytes: &[u8]) -> String {
    let mut escaped = String::with_capacity(bytes.len());
    for &byte in bytes {
        if (0x20..=0x7E).contains(&byte) {
            escaped.push(char::from(byte));
        } else {
            let _ = write!(escaped, "\\x{{{byte:02x}}}");
        }
    }
    escaped
}
