//! Recognize radix, fractional, exponent and suffix boundaries without deciding a value type.

use tokens::{FloatLiteralKind, IntegerLiteralKind, NumericBase, NumericLiteralKind, TokenKind};

use crate::{ByteScanner, is_identifier_continue, is_identifier_start};

impl ByteScanner<'_> {
    pub(crate) fn scan_number(&mut self, first: u8) -> TokenKind {
        let base = match (first, self.peek()) {
            (b'0', Some(b'b' | b'B')) => NumericBase::Binary,
            (b'0', Some(b'o' | b'O')) => NumericBase::Octal,
            (b'0', Some(b'x' | b'X')) => NumericBase::Hexadecimal,
            _ => NumericBase::Decimal,
        };
        if base != NumericBase::Decimal {
            self.offset += 1;
        }
        let saw_digit = self.scan_digits(base);

        let mut is_float = first == b'.';
        if base == NumericBase::Decimal
            && !is_float
            && self.peek() == Some(b'.')
            && !self
                .peek_ahead(1)
                .is_some_and(|next| next == b'.' || is_identifier_start(next))
        {
            is_float = true;
            self.offset += 1;
            self.scan_digits(NumericBase::Decimal);
        }

        let has_exponent = base == NumericBase::Decimal && matches!(self.peek(), Some(b'e' | b'E'));
        let empty_exponent = if has_exponent {
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            !self.scan_digits(NumericBase::Decimal)
        } else {
            false
        };

        let has_suffix = self.peek().is_some_and(is_identifier_start);
        if has_suffix {
            self.offset += 1;
            while self.peek().is_some_and(is_identifier_continue) {
                self.offset += 1;
            }
        }

        let kind = if is_float || has_exponent {
            NumericLiteralKind::Float(FloatLiteralKind {
                has_exponent,
                empty_exponent,
                has_suffix,
            })
        } else {
            NumericLiteralKind::Integer(IntegerLiteralKind {
                base,
                empty_digits: base != NumericBase::Decimal && !saw_digit,
                has_suffix,
            })
        };
        TokenKind::NumericLiteral(kind)
    }

    fn scan_digits(&mut self, base: NumericBase) -> bool {
        let mut saw_digit = false;
        while let Some(byte) = self.peek() {
            if byte != b'_' {
                let valid = match base {
                    NumericBase::Binary => matches!(byte, b'0' | b'1'),
                    NumericBase::Octal => matches!(byte, b'0'..=b'7'),
                    NumericBase::Decimal => byte.is_ascii_digit(),
                    NumericBase::Hexadecimal => byte.is_ascii_hexdigit(),
                };
                if !valid {
                    break;
                }
                saw_digit = true;
            }
            self.offset += 1;
        }
        saw_digit
    }
}
