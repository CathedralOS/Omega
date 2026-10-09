//! Full replay of footprint coverage, inventory and text derivation into one
//! certificate - reachable today only from tests, and pinned as source text by
//! tests/architecture/native_image_identity.rs.

macro_rules! footprint_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

footprint_digest!(FinalFootprintCoverageDigest);
footprint_digest!(FinalFootprintPlacementBindingDigest);
footprint_digest!(FinalFootprintCertificateDigest);
