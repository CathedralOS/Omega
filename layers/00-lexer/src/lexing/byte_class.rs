//! The lookup tables that let the scanner decide what a byte means without comparing it to anything.

/// What the first byte of a token tells the scanner to do.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ByteClass {
    Whitespace,
    WordStart,
    Digit,
    Quote,
    Slash, // Kept apart from other punctuation because it may begin a comment.
    Punctuation,
    NonAscii,
    Unknown, // An ASCII byte that is not part of Omega's syntax, such as `@` or a control byte.
}

/// The class of every possible byte, so that choosing a token's rule is one lookup.
pub(crate) const BYTE_CLASSES: [ByteClass; 256] = build_byte_classes();

/// Whether a byte can appear inside a word after its first byte: a letter, a digit or an underscore.
pub(crate) const IS_WORD_CONTINUE: [bool; 256] = build_is_word_continue();

const fn build_byte_classes() -> [ByteClass; 256] {
    let mut byte_classes = [ByteClass::Unknown; 256];
    let mut byte_value = 0usize;

    while byte_value < 256 {
        byte_classes[byte_value] = match byte_value as u8 {
            b' ' | b'\t' | b'\r' | b'\n' => ByteClass::Whitespace,
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => ByteClass::WordStart,
            b'0'..=b'9' => ByteClass::Digit,
            b'"' => ByteClass::Quote,
            b'/' => ByteClass::Slash,
            // @Volatile: must list every first byte in `Punctuation::ORDERED_SPELLINGS` except the
            // slash. A test below fails if the two ever disagree.
            b'+' | b'-' | b'*' | b'%' | b'<' | b'>' | b'=' | b'!' | b'&' | b'|' | b':' | b'.' | b'\'' | b'^' | b'~' | b'#' | b',' | b';' | b'(' | b')'
            | b'[' | b']' | b'{' | b'}' => ByteClass::Punctuation,
            0x80..=0xFF => ByteClass::NonAscii,
            _ => ByteClass::Unknown,
        };
        byte_value += 1;
    }

    byte_classes
}

const fn build_is_word_continue() -> [bool; 256] {
    let mut is_word_continue = [false; 256];
    let mut byte_value = 0usize;

    while byte_value < 256 {
        is_word_continue[byte_value] = matches!(byte_value as u8, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_');
        byte_value += 1;
    }

    is_word_continue
}

#[cfg(test)]
mod tests {
    use crate::lexing::byte_class::{BYTE_CLASSES, ByteClass, IS_WORD_CONTINUE};
    use crate::tokens::punctuation::Punctuation;

    #[test]
    fn every_punctuation_first_byte_is_classed_as_punctuation_or_slash() {
        for (spelling, _) in Punctuation::ORDERED_SPELLINGS {
            let first_byte = spelling.as_bytes()[0];
            let byte_class = BYTE_CLASSES[usize::from(first_byte)];

            assert!(
                matches!(byte_class, ByteClass::Punctuation | ByteClass::Slash),
                "`{spelling}` starts with a byte classed {byte_class:?}, so the scanner would never try it"
            );
        }
    }

    #[test]
    fn every_byte_classed_as_punctuation_starts_some_spelling() {
        for byte_value in 0u8..=255 {
            if matches!(BYTE_CLASSES[usize::from(byte_value)], ByteClass::Punctuation | ByteClass::Slash) {
                assert!(
                    Punctuation::longest_match(&[byte_value]).is_some(),
                    "byte {byte_value:#04x} is classed as punctuation but matches no spelling"
                );
            }
        }
    }

    #[test]
    fn a_word_start_can_also_continue_a_word_and_so_can_a_digit() {
        for byte_value in 0u8..=255 {
            let byte_class = BYTE_CLASSES[usize::from(byte_value)];
            let expected_is_word_continue = matches!(byte_class, ByteClass::WordStart | ByteClass::Digit);

            assert_eq!(IS_WORD_CONTINUE[usize::from(byte_value)], expected_is_word_continue, "byte {byte_value:#04x}");
        }
    }
}
