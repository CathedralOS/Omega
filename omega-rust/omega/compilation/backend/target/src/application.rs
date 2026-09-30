//! Application presentation and deployment identity shared by Build and images.
//! No source, Build value, signing, or filesystem operations belong here.

/// Portable presentation requested by the authored Console or Gui case.
/// This does not select an execution environment or enable bundle publication
/// independently of the selected target and requested output product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedApplicationIntent {
    Console,
    Gui,
}

/// Authored application and loader metadata shared by build selection,
/// retained proposals and publication. Presentation intent is independent of
/// the numeric PE subsystem; one must never be inferred from the other.
/// These values describe a requested image, not permission to publish it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NativeApplicationMetadata {
    /// Validated authored application name; absent for other project roles.
    pub name: Option<String>,
    pub intent: Option<HostedApplicationIntent>,
    /// Identity bound into signing and application-package publication.
    pub identifier: Option<ApplicationIdentifier>,
    pub subsystem: u16,
    /// The effective ELF loader of the realized target: the build's explicit
    /// per-target selection, else the target environment's declared default
    /// (wiki/spec/build/configuration.md#elf-executable-loader-selection).
    /// `None` for targets without `PT_INTERP`. It never selects dynamic
    /// linking: the image owner consumes it only when the emitted object
    /// retains normalized ELF imports.
    pub elf_interpreter: Option<crate::NormalizedElfInterpreterPlan>,
}

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
