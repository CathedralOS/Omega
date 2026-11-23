//! The lookup tables that let the scanner decide what a byte means without comparing it to anything.

/// What the first byte of a token tells the scanner to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteClass {
    Whitespace,
    WordStart,
    Digit,
    Quote,
    Slash, // Kept apart from other punctuation because it may begin a comment.
    NonAscii,
    OtherAscii, // The punctuation vocabulary decides whether this starts a token.
}

/// The class of every possible byte, so that choosing a token's rule is one lookup.
pub(crate) const BYTE_CLASSES: [ByteClass; 256] = build_byte_classes();

/// Whether a byte can appear inside a word after its first byte: a letter, a digit or an underscore.
pub(crate) const IS_WORD_CONTINUE: [bool; 256] = build_is_word_continue();

const fn build_byte_classes() -> [ByteClass; 256] {
    let mut byte_classes = [ByteClass::OtherAscii; 256];
    let mut byte_value = 0usize;

    while byte_value < 256 {
        byte_classes[byte_value] = match byte_value as u8 {
            b' ' | b'\t' | b'\r' | b'\n' => ByteClass::Whitespace,
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => ByteClass::WordStart,
            b'0'..=b'9' => ByteClass::Digit,
            b'"' => ByteClass::Quote,
            b'/' => ByteClass::Slash,
            0x80..=0xFF => ByteClass::NonAscii,
            _ => ByteClass::OtherAscii,
        };
        byte_value += 1;
    }

    byte_classes
}

const fn build_is_word_continue() -> [bool; 256] {
    let mut is_word_continue = [false; 256];
    let mut byte_value = 0usize;

    while byte_value < 256 {
        is_word_continue[byte_value] = matches!(BYTE_CLASSES[byte_value], ByteClass::WordStart | ByteClass::Digit);
        byte_value += 1;
    }

    is_word_continue
}

#[cfg(test)]
mod tests {
    use super::{ByteClass, build_byte_classes, build_is_word_continue};
    use crate::Punctuation;

    #[test]
    fn punctuation_reaches_the_scanner_rule_for_its_first_byte() {
        let classes = build_byte_classes();
        for (spelling, _) in Punctuation::ORDERED_SPELLINGS {
            let byte = spelling.as_bytes()[0];
            assert_eq!(
                classes[usize::from(byte)],
                if byte == b'/' { ByteClass::Slash } else { ByteClass::OtherAscii },
                "{spelling}"
            );
        }
    }

    #[test]
    fn a_word_start_can_also_continue_a_word_and_so_can_a_digit() {
        let continues_word = build_is_word_continue();
        for byte_value in 0u8..=255 {
            let expected_is_word_continue = byte_value.is_ascii_alphanumeric() || byte_value == b'_';

            assert_eq!(continues_word[usize::from(byte_value)], expected_is_word_continue, "byte {byte_value:#04x}");
        }
    }
}
