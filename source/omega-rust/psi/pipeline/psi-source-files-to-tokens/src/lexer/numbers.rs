use psi_source::Span;
use psi_tokens::{
    FloatLiteralKind, IntegerLiteralKind, NumericBase, NumericLiteralKind, TokenKind,
};

use super::{LexedToken, is_identifier_continue, is_identifier_start};

impl<'source> super::Lexer<'source> {
    pub(super) fn lex_number(&mut self, start: usize, first: char) -> LexedToken {
        if first == '.' {
            return self.lex_leading_dot_float(start);
        }

        let mut end = start + first.len_utf8();
        let mut integer_kind = IntegerLiteralKind::default();
        let mut float_kind = FloatLiteralKind::default();

        if first == '0' {
            match self.peek_character() {
                Some('b') | Some('B') => {
                    integer_kind.base = NumericBase::Binary;
                    end = self.consume_base_prefix(end);
                    integer_kind.empty_digits = !self.consume_digits_with_underscores(&mut end, 2);
                }
                Some('o') | Some('O') => {
                    integer_kind.base = NumericBase::Octal;
                    end = self.consume_base_prefix(end);
                    integer_kind.empty_digits = !self.consume_digits_with_underscores(&mut end, 8);
                }
                Some('x') | Some('X') => {
                    integer_kind.base = NumericBase::Hexadecimal;
                    end = self.consume_base_prefix(end);
                    integer_kind.empty_digits = !self.consume_digits_with_underscores(&mut end, 16);
                }
                _ => {
                    self.consume_digits_with_underscores(&mut end, 10);
                }
            }
        } else {
            self.consume_digits_with_underscores(&mut end, 10);
        }

        let mut is_float = false;
        if integer_kind.base == NumericBase::Decimal && self.peek_character() == Some('.') {
            let next_after_dot = self.peek_nth_character(1);
            if next_after_dot.is_some_and(|next| next.is_ascii_digit())
                || next_after_dot.is_none()
                || next_after_dot.is_some_and(|next| next != '.' && !is_identifier_start(next))
            {
                is_float = true;
                if let Some((dot_index, dot)) = self.chars.next() {
                    end = dot_index + dot.len_utf8();
                }
                self.consume_digits_with_underscores(&mut end, 10);
            }
        }

        if integer_kind.base == NumericBase::Decimal
            && matches!(self.peek_character(), Some('e') | Some('E'))
        {
            is_float = true;
            float_kind.has_exponent = true;
            if let Some((exp_index, exp)) = self.chars.next() {
                end = exp_index + exp.len_utf8();
            }

            if matches!(self.peek_character(), Some('+') | Some('-'))
                && let Some((sign_index, sign)) = self.chars.next()
            {
                end = sign_index + sign.len_utf8();
            }

            float_kind.empty_exponent = !self.consume_digits_with_underscores(&mut end, 10);
        }

        let has_suffix = self.consume_literal_suffix(&mut end);

        LexedToken {
            kind: if is_float {
                TokenKind::NumericLiteral(NumericLiteralKind::Float(FloatLiteralKind {
                    has_exponent: float_kind.has_exponent,
                    empty_exponent: float_kind.empty_exponent,
                    has_suffix,
                }))
            } else {
                TokenKind::NumericLiteral(NumericLiteralKind::Integer(IntegerLiteralKind {
                    base: integer_kind.base,
                    empty_digits: integer_kind.empty_digits,
                    has_suffix,
                }))
            },
            span: Span::new(start, end),
        }
    }

    fn lex_leading_dot_float(&mut self, start: usize) -> LexedToken {
        let mut end = start + '.'.len_utf8();
        let mut float_kind = FloatLiteralKind::default();

        self.consume_digits_with_underscores(&mut end, 10);

        if matches!(self.peek_character(), Some('e') | Some('E')) {
            float_kind.has_exponent = true;
            if let Some((exp_index, exp)) = self.chars.next() {
                end = exp_index + exp.len_utf8();
            }

            if matches!(self.peek_character(), Some('+') | Some('-'))
                && let Some((sign_index, sign)) = self.chars.next()
            {
                end = sign_index + sign.len_utf8();
            }

            float_kind.empty_exponent = !self.consume_digits_with_underscores(&mut end, 10);
        }

        float_kind.has_suffix = self.consume_literal_suffix(&mut end);

        LexedToken {
            kind: TokenKind::NumericLiteral(NumericLiteralKind::Float(float_kind)),
            span: Span::new(start, end),
        }
    }

    fn peek_nth_character(&self, offset: usize) -> Option<char> {
        let mut clone = self.chars.clone();
        for _ in 0..offset {
            clone.next()?;
        }
        clone.peek().map(|(_, character)| *character)
    }

    fn consume_base_prefix(&mut self, current_end: usize) -> usize {
        if let Some((prefix_index, prefix)) = self.chars.next() {
            prefix_index + prefix.len_utf8()
        } else {
            current_end
        }
    }

    fn consume_digits_with_underscores(&mut self, end: &mut usize, radix: u32) -> bool {
        let mut saw_digit = false;

        while let Some((next_index, next)) = self.chars.peek().copied() {
            if next == '_' {
                *end = next_index + next.len_utf8();
                self.chars.next();
                continue;
            }

            if next.is_digit(radix) {
                saw_digit = true;
                *end = next_index + next.len_utf8();
                self.chars.next();
                continue;
            }

            break;
        }

        saw_digit
    }

    fn consume_literal_suffix(&mut self, end: &mut usize) -> bool {
        let Some(next) = self.peek_character() else {
            return false;
        };
        if !is_identifier_start(next) {
            return false;
        }

        let mut has_suffix = false;
        while let Some((next_index, next)) = self.chars.peek().copied() {
            if !is_identifier_continue(next) {
                break;
            }

            has_suffix = true;
            *end = next_index + next.len_utf8();
            self.chars.next();
        }

        has_suffix
    }
}
