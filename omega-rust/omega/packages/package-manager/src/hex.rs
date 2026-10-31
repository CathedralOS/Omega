use std::fmt;

/// Shared lowercase-hex display wrapper for digest bytes rendered in reports
/// and lockfile text.
pub(crate) struct Hex<'a, B: AsRef<[u8]> + ?Sized>(pub &'a B);

impl<B: AsRef<[u8]> + ?Sized> fmt::Display for Hex<'_, B> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_ref() {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Parse exactly 64 lowercase hexadecimal digits — the inverse of [`Hex`]'s
/// display form. `None` for any other length or a non-hexadecimal byte.
pub(crate) fn hex_digest(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return None;
    }
    let mut digest = [0; 32];
    for (index, byte) in digest.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(digest)
}
