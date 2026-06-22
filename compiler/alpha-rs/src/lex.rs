// Lexer: source bytes -> tokens. Platform-independent.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TokKind {
    Ident,
    Int,
    LBrace,
    RBrace,
    LParen,
    RParen,
    Semi,
    Colon,
    ColonColon,
    Comma,
    Dot,
    Amp,
    Eq,
    Plus,
    Minus,
    Star,
    Slash,
    Eof,
}

#[derive(Clone, Copy)]
pub struct Token {
    pub kind: TokKind,
    pub start: usize,
    pub len: usize,
}

pub fn tok_text<'a>(t: &Token, src: &'a [u8]) -> &'a [u8] {
    &src[t.start..t.start + t.len]
}

pub fn lex(src: &[u8]) -> Result<Vec<Token>, String> {
    let mut toks = Vec::new();
    let mut i = 0usize;
    let n = src.len();
    while i < n {
        let c = src[i];
        if c == b' ' || c == b'\t' || c == b'\r' || c == b'\n' {
            i += 1;
            continue;
        }
        if c == b'/' && i + 1 < n && src[i + 1] == b'/' {
            while i < n && src[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < n && (src[i].is_ascii_alphanumeric() || src[i] == b'_') {
                i += 1;
            }
            toks.push(Token { kind: TokKind::Ident, start, len: i - start });
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < n && src[i].is_ascii_digit() {
                i += 1;
            }
            toks.push(Token { kind: TokKind::Int, start, len: i - start });
            continue;
        }
        if c == b':' && i + 1 < n && src[i + 1] == b':' {
            toks.push(Token { kind: TokKind::ColonColon, start: i, len: 2 });
            i += 2;
            continue;
        }
        let kind = match c {
            b'{' => TokKind::LBrace,
            b'}' => TokKind::RBrace,
            b'(' => TokKind::LParen,
            b')' => TokKind::RParen,
            b';' => TokKind::Semi,
            b':' => TokKind::Colon,
            b',' => TokKind::Comma,
            b'.' => TokKind::Dot,
            b'&' => TokKind::Amp,
            b'=' => TokKind::Eq,
            b'+' => TokKind::Plus,
            b'-' => TokKind::Minus,
            b'*' => TokKind::Star,
            b'/' => TokKind::Slash,
            _ => {
                return Err(format!(
                    "alpha-onramp: lex error: unexpected byte {:?} at offset {}",
                    c as char, i
                ))
            }
        };
        toks.push(Token { kind, start: i, len: 1 });
        i += 1;
    }
    toks.push(Token { kind: TokKind::Eof, start: n, len: 0 });
    Ok(toks)
}
