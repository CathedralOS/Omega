//! Preserve the canonical byte atom protocol independently of its writer.
use super::byte_atom;

fn original_byte_atom(tag: &str, value: &[u8]) -> String {
    let mut output = String::with_capacity(tag.len() + value.len().saturating_mul(2) + 24);
    output.push_str(tag);
    output.push('(');
    output.push_str(&value.len().to_string());
    output.push(':');
    for byte in value {
        output.push_str(&format!("{byte:02x}"));
    }
    output.push(')');
    output
}

#[test]
fn byte_atoms_match_original_identity_encoding() {
    for tag in ["", "package-owner", "toolchain-source-owner", "\0é(:)"] {
        for length in [0, 1, 2, 15, 16, 31, 32, 33, 63, 64, 255, 256, 4097] {
            let bytes = (0..length)
                .map(|position| (position as u8).wrapping_mul(73))
                .collect::<Vec<_>>();
            assert_eq!(byte_atom(tag, &bytes), original_byte_atom(tag, &bytes));
        }
        for byte in 0..=u8::MAX {
            assert_eq!(byte_atom(tag, &[byte]), original_byte_atom(tag, &[byte]));
        }
    }
    assert_eq!(byte_atom("x", &[0, 15, 16, 255]), "x(4:000f10ff)");
}
