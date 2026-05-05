use crate::lexer::{Token, TokenKind};

pub fn tokenize(source: &str) -> Vec<Token> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            for comment_ch in chars.by_ref() {
                if comment_ch == '\n' {
                    break;
                }
            }

            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let mut lexeme = String::from(ch);

            while let Some(next) = chars.peek() {
                if next.is_ascii_alphanumeric() || *next == '_' {
                    lexeme.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }

            tokens.push(Token {
                kind: TokenKind::Identifier,
                lexeme,
            });
            continue;
        }

        if ch.is_ascii_digit() {
            let mut lexeme = String::from(ch);

            while let Some(next) = chars.peek() {
                if next.is_ascii_digit() {
                    lexeme.push(*next);
                    chars.next();
                } else {
                    break;
                }
            }

            tokens.push(Token {
                kind: TokenKind::Integer,
                lexeme,
            });
            continue;
        }

        if ch == '"' {
            let mut lexeme = String::new();

            while let Some(next) = chars.next() {
                if next == '"' {
                    break;
                }

                if next == '\\' {
                    if let Some(escaped) = chars.next() {
                        lexeme.push('\\');
                        lexeme.push(escaped);
                    }
                } else {
                    lexeme.push(next);
                }
            }

            tokens.push(Token {
                kind: TokenKind::String,
                lexeme,
            });
            continue;
        }

        let mut lexeme = String::from(ch);

        if (ch == ':' && chars.peek() == Some(&':')) || (ch == '-' && chars.peek() == Some(&'>')) {
            lexeme.push(chars.next().expect("peeked character exists"));
        }

        tokens.push(Token {
            kind: TokenKind::Symbol,
            lexeme,
        });
    }

    tokens
}
