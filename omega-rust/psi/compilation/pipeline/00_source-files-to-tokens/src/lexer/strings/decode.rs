/// Decode a literal that `lex_cooked_string_end` already validated: quoted,
/// with only `\"`, `\\`, `\n`, `\r`, `\t`, `\0` and `\xHH` escapes.
///
/// Every escape spells more bytes than it decodes to, so the quoted contents
/// bound the appended result. The stream owns the buffer across literals. Escape introducers are
/// ASCII and never occur inside a multi-byte UTF-8 sequence, so the text
/// between escapes is copied as whole byte runs.
pub(in crate::lexer) fn decode_string_literal(raw: &str, bytes: &mut Vec<u8>) {
    let contents = &raw.as_bytes()[1..raw.len() - 1];
    bytes.reserve(contents.len());
    let mut rest = contents;

    while let Some(escape) = rest.iter().position(|&byte| byte == b'\\') {
        bytes.extend_from_slice(&rest[..escape]);
        let (decoded, spelled_length) = match rest[escape + 1] {
            b'"' => (b'"', 2),
            b'\\' => (b'\\', 2),
            b'n' => (b'\n', 2),
            b'r' => (b'\r', 2),
            b't' => (b'\t', 2),
            b'0' => (b'\0', 2),
            b'x' => {
                let high = hex_digit_value(rest[escape + 2]);
                let low = hex_digit_value(rest[escape + 3]);
                ((high << 4) | low, 4)
            }
            other => unreachable!("validated string literal contains escape byte {other:#04x}"),
        };
        bytes.push(decoded);
        rest = &rest[escape + spelled_length..];
    }
    bytes.extend_from_slice(rest);
}

fn hex_digit_value(digit: u8) -> u8 {
    match digit {
        b'0'..=b'9' => digit - b'0',
        b'a'..=b'f' => digit - b'a' + 10,
        b'A'..=b'F' => digit - b'A' + 10,
        other => unreachable!("validated hex escape contains byte {other:#04x}"),
    }
}
