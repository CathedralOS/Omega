// Native stages name what an optimization phase consumed and produced by
// 32-byte digests, so a later stage can refuse a record from another run.
//
// Every identity hashes its own NUL-terminated domain string, the `u64`
// little-endian canonical byte length and the canonical bytes, so equal bytes
// under two identity types never collide. `CanonicalIdentityEncoder` lets a
// caller that knows the length up front stream the same bytes without
// building one buffer.

const IDENTITY_WIDTH: usize = 32;

fn domain_digest(domain: &[u8], canonical: &[u8]) -> [u8; IDENTITY_WIDTH] {
    let mut digest = ::wire::digest_encoder::DomainDigestEncoder::new(domain);
    digest.bytes(canonical);
    digest.finish()
}

pub struct CanonicalIdentityEncoder(::wire::digest_encoder::DomainDigestEncoder);

impl CanonicalIdentityEncoder {
    fn new(domain: &[u8], canonical_len: u64) -> Self {
        let mut digest = ::wire::digest_encoder::DomainDigestEncoder::new(domain);
        digest.u64(canonical_len);
        Self(digest)
    }

    pub fn update(&mut self, bytes: &[u8]) {
        self.0.raw(bytes);
    }

    fn finish(self) -> [u8; IDENTITY_WIDTH] {
        self.0.finish()
    }
}

macro_rules! canonical_identity {
    ($name:ident => $domain:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; IDENTITY_WIDTH]);

        impl $name {
            pub fn from_canonical_bytes(canonical: &[u8]) -> Self {
                Self(domain_digest($domain, canonical))
            }

            pub fn canonical_encoder(canonical_len: u64) -> CanonicalIdentityEncoder {
                CanonicalIdentityEncoder::new($domain, canonical_len)
            }

            pub fn from_canonical_encoder(encoder: CanonicalIdentityEncoder) -> Self {
                Self(encoder.finish())
            }

            pub const fn from_bytes(bytes: [u8; IDENTITY_WIDTH]) -> Self {
                Self(bytes)
            }

            pub const fn bytes(self) -> [u8; IDENTITY_WIDTH] {
                self.0
            }

            pub fn encode(self) -> [u8; IDENTITY_WIDTH] {
                self.0
            }
        }
    };
}

canonical_identity!(OptimizationUnitIdentity => b"omega.optimization-unit-identity.v1\0");
canonical_identity!(OptimizationValidatorIdentity => b"omega.optimization-validator-identity.v1\0");
canonical_identity!(
    AcceptedObligationFactIdentity => b"omega.accepted-obligation-fact-identity.v1\0"
);
canonical_identity!(
    OptimizationIdentityBundleIdentity => b"omega.optimization-identity-bundle-identity.v2\0"
);
canonical_identity!(
    PrePhysicalOptimizationManifestIdentity =>
        b"omega.pre-physical-optimization-manifest-identity.v3\0"
);
canonical_identity!(
    PostAllocationOptimizationManifestIdentity =>
        b"omega.post-allocation-optimization-manifest-identity.v1\0"
);
canonical_identity!(
    SelectedLoweringOptimizationCompletionIdentity =>
        b"omega.selected-lowering-optimization-completion-identity.v1\0"
);
canonical_identity!(
    PreAllocationOptimizationCompletionIdentity =>
        b"omega.pre-allocation-optimization-completion-identity.v1\0"
);
canonical_identity!(
    OptimizedAbstractPlanProjectionIdentity =>
        b"omega.optimized-abstract-plan-projection-identity.v3\0"
);
canonical_identity!(
    OptimizedBoundaryOccurrenceIdentity => b"omega.optimized-boundary-occurrence-identity.v1\0"
);
canonical_identity!(
    OptimizedOperatorOccurrenceIdentity => b"omega.optimized-operator-occurrence-identity.v1\0"
);
canonical_identity!(
    NativeOptimizationProjectionIdentity => b"omega.native-optimization-projection-identity.v2\0"
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_domains_are_distinct_for_equal_canonical_bytes() {
        let mut digests = vec![
            OptimizationUnitIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationValidatorIdentity::from_canonical_bytes(b"same").bytes(),
            AcceptedObligationFactIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizationIdentityBundleIdentity::from_canonical_bytes(b"same").bytes(),
            PrePhysicalOptimizationManifestIdentity::from_canonical_bytes(b"same").bytes(),
            PostAllocationOptimizationManifestIdentity::from_canonical_bytes(b"same").bytes(),
            SelectedLoweringOptimizationCompletionIdentity::from_canonical_bytes(b"same").bytes(),
            PreAllocationOptimizationCompletionIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedAbstractPlanProjectionIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedBoundaryOccurrenceIdentity::from_canonical_bytes(b"same").bytes(),
            OptimizedOperatorOccurrenceIdentity::from_canonical_bytes(b"same").bytes(),
            NativeOptimizationProjectionIdentity::from_canonical_bytes(b"same").bytes(),
        ];
        let count = digests.len();
        digests.sort_unstable();
        digests.dedup();
        assert_eq!(digests.len(), count);
    }

    #[test]
    fn streamed_and_contiguous_digests_agree() {
        let mut encoder = OptimizationUnitIdentity::canonical_encoder(6);
        encoder.update(b"abc");
        encoder.update(b"def");
        assert_eq!(
            OptimizationUnitIdentity::from_canonical_encoder(encoder),
            OptimizationUnitIdentity::from_canonical_bytes(b"abcdef")
        );
    }
}
