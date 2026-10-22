use std::fmt;

/// A borrowed logical spelling or decoded literal, resolved through its stream.
/// No ownership travels with a token or with this temporary text view.
#[derive(Clone, Copy)]
pub enum TokenText<'text> {
    Source(&'text str),
    Bytes(&'text [u8]),
}

impl PartialEq for TokenText<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl Eq for TokenText<'_> {}

impl<'text> TokenText<'text> {
    #[inline]
    pub fn as_bytes(self) -> &'text [u8] {
        match self {
            Self::Source(text) => text.as_bytes(),
            Self::Bytes(bytes) => bytes,
        }
    }
    #[inline]
    pub fn try_as_str(self) -> Option<&'text str> {
        match self {
            Self::Source(text) => Some(text),
            Self::Bytes(bytes) => std::str::from_utf8(bytes).ok(),
        }
    }
    #[inline]
    pub fn as_str(self) -> &'text str {
        self.try_as_str()
            .expect("token text is not valid UTF-8; use `as_bytes` for byte literals")
    }
}

impl fmt::Debug for TokenText<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.try_as_str() {
            Some(text) => formatter.debug_tuple("TokenText").field(&text).finish(),
            None => formatter
                .debug_tuple("TokenBytes")
                .field(&self.as_bytes())
                .finish(),
        }
    }
}

impl PartialEq<&str> for TokenText<'_> {
    fn eq(&self, other: &&str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
impl PartialEq<TokenText<'_>> for &str {
    fn eq(&self, other: &TokenText<'_>) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}
