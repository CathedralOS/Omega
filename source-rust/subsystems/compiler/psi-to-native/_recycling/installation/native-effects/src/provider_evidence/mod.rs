pub(crate) mod component_eras;
pub(crate) use ::wire::digest_encoder;
pub(crate) mod executable_scopes;
pub(crate) mod providers;

macro_rules! evidence_digest {
    ($(#[$attr:meta])* $name:ident, from: $(#[$fdoc:meta])* $fv:vis) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            $(#[$fdoc])*
            $fv const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
    ($(#[$attr:meta])* $name:ident) => {
        $(#[$attr])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

pub(crate) use evidence_digest;

pub(crate) fn identity_text(
    identities: &terminal_psi::TerminalIdentityTable,
    index: terminal_psi::TerminalIdentityIndex,
) -> &str {
    identities.text(index).unwrap_or_default()
}
