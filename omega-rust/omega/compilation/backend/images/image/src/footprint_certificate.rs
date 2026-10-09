//! Full replay of footprint coverage, inventory and text derivation into one
//! certificate - reachable today only from tests, and pinned as source text by
//! tests/architecture/native_image_identity.rs.

use crate::{
    CompilerEntryFootprintBindingEvidence, CompilerFunctionValidationEvidence,
    CompilerTextValidationEvidence, PlacedExecutableRegionInventory,
};
#[cfg(test)]
use crate::{
    CompilerTextDerivationDigest, CompilerTextRelocationEnvelopeDigest, EncodedCompilerTextDigest,
    FinalCompilerTextDigest,
};
use diagnostics::Diagnostic;
use crate::final_image::{FNV_OFFSET, fingerprint_bytes};
use sha2::{Digest, Sha256};


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
