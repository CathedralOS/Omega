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
