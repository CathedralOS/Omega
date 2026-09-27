//! Validated deployment identity shared by build extraction and publication.
//! No source, Build value, signing, or filesystem operations belong here.

/// The authored application identifier: it supplies the GUI CodeDirectory
/// signing identity and `CFBundleIdentifier`
/// (wiki/spec/build/macos_application.md). Validation is deliberately
/// separate from the executable-name rule: nonempty ASCII in
/// `A-Z a-z 0-9 . -` with no empty `.`-separated segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApplicationIdentifier(String);

impl ApplicationIdentifier {
    /// Validate one explicit deployment identity. Build extraction separately
    /// decides whether an omitted or empty field means no authored identity.
    pub fn new(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.is_empty() {
            return Err("identifier is empty");
        }
        if !bytes.is_ascii() {
            return Err("identifier is not ASCII");
        }
        if bytes.iter().any(|byte| {
            !matches!(
                byte,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'
            )
        }) {
            return Err("identifier contains a byte outside `A-Z a-z 0-9 . -`");
        }
        if bytes
            .split(|byte| *byte == b'.')
            .any(|segment| segment.is_empty())
        {
            return Err("identifier has an empty `.`-separated segment");
        }
        Ok(Self(
            String::from_utf8(bytes.to_vec()).expect("ASCII identifier bytes are UTF-8"),
        ))
    }

    /// The validated identifier spelling bound into CodeDirectory signing and
    /// `CFBundleIdentifier` publication.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
