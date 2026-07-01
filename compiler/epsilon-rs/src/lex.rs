// Lexer: source bytes -> tokens. Platform-independent.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Ident,
    Int,
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Semi,
    Colon,
    ColonColon,
    Comma,
    Dot,
    Amp,
    Pipe,
    Caret,
    Eq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    Ne,
    Shl, // <<
    Shr, // >>
    Arrow, // ->
    Str,   // string literal; span (start..start+len) covers the bytes BETWEEN the quotes
    Eof,
}

#[derive(Clone, Copy)]
pub struct Token {
    pub kind: TokenKind,
    pub start: usize,
    pub len: usize,
}

pub fn token_text<'a>(token: &Token, source: &'a [u8]) -> &'a [u8] {
    &source[token.start..token.start + token.len]
}

pub fn lex(source: &[u8]) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut position = 0usize;
    let source_len = source.len();
    while position < source_len {
        let byte = source[position];
        if byte == b' ' || byte == b'\t' || byte == b'\r' || byte == b'\n' {
            position += 1;
            continue;
        }
        if byte == b'/' && position + 1 < source_len && source[position + 1] == b'/' {
            while position < source_len && source[position] != b'\n' {
                position += 1;
            }
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = position;
            while position < source_len
                && (source[position].is_ascii_alphanumeric() || source[position] == b'_')
            {
                position += 1;
            }
            tokens.push(Token { kind: TokenKind::Ident, start, len: position - start });
            continue;
        }
        if byte.is_ascii_digit() {
            let start = position;
            while position < source_len && source[position].is_ascii_digit() {
                position += 1;
            }
            tokens.push(Token { kind: TokenKind::Int, start, len: position - start });
            continue;
        }
        if byte == b'"' {
            let start = position + 1;
            position += 1;
            while position < source_len && source[position] != b'"' {
                position += 1;
            }
            if position >= source_len {
                return Err(format!(
                    "alpha-onramp: lex error: unterminated string starting at offset {}",
                    start - 1
                ));
            }
            tokens.push(Token { kind: TokenKind::Str, start, len: position - start });
            position += 1; // consume closing quote
            continue;
        }
        if byte == b'=' && position + 1 < source_len && source[position + 1] == b'=' {
            tokens.push(Token { kind: TokenKind::EqEq, start: position, len: 2 });
            position += 2;
            continue;
        }
        if byte == b'!' && position + 1 < source_len && source[position + 1] == b'=' {
            tokens.push(Token { kind: TokenKind::Ne, start: position, len: 2 });
            position += 2;
            continue;
        }
        if byte == b'-' && position + 1 < source_len && source[position + 1] == b'>' {
            tokens.push(Token { kind: TokenKind::Arrow, start: position, len: 2 });
            position += 2;
            continue;
        }
        if byte == b'<' {
            let next = if position + 1 < source_len { source[position + 1] } else { 0 };
            let (kind, len) = match next {
                b'<' => (TokenKind::Shl, 2),
                b'=' => (TokenKind::Le, 2),
                _ => (TokenKind::Lt, 1),
            };
            tokens.push(Token { kind, start: position, len });
            position += len;
            continue;
        }
        if byte == b'>' {
            let next = if position + 1 < source_len { source[position + 1] } else { 0 };
            let (kind, len) = match next {
                b'>' => (TokenKind::Shr, 2),
                b'=' => (TokenKind::Ge, 2),
                _ => (TokenKind::Gt, 1),
            };
            tokens.push(Token { kind, start: position, len });
            position += len;
            continue;
        }
        if byte == b':' && position + 1 < source_len && source[position + 1] == b':' {
            tokens.push(Token { kind: TokenKind::ColonColon, start: position, len: 2 });
            position += 2;
            continue;
        }
        let kind = match byte {
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b';' => TokenKind::Semi,
            b':' => TokenKind::Colon,
            b',' => TokenKind::Comma,
            b'.' => TokenKind::Dot,
            b'&' => TokenKind::Amp,
            b'|' => TokenKind::Pipe,
            b'^' => TokenKind::Caret,
            b'=' => TokenKind::Eq,
            b'+' => TokenKind::Plus,
            b'-' => TokenKind::Minus,
            b'*' => TokenKind::Star,
            b'/' => TokenKind::Slash,
            b'%' => TokenKind::Percent,
            _ => {
                return Err(format!(
                    "alpha-onramp: lex error: unexpected byte {:?} at offset {}",
                    byte as char, position
                ))
            }
        };
        tokens.push(Token { kind, start: position, len: 1 });
        position += 1;
    }
    tokens.push(Token { kind: TokenKind::Eof, start: source_len, len: 0 });
    Ok(tokens)
}
