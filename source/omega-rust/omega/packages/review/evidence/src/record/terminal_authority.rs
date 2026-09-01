use super::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use omega_effects::{TerminalAuthorityDisposition, provider_plan::ServiceSchemaDigest};

/// One explicit consumer permission for an exact requirement in one complete
/// normalized service schema.
///
/// This is review evidence for supplied policy, not evidence that the
/// permission was exercised or that any terminal mechanism was admitted.
/// Construction remains compiler-owned so readable service, requirement,
/// provider, and risk labels cannot manufacture a permission row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageReviewTerminalAuthorityPermission {
    pub(crate) service: PackageReviewNominalIdentity,
    pub(crate) service_schema: ServiceSchemaDigest,
    pub(crate) requirement_identity: String,
    pub(crate) permitted: TerminalAuthorityDisposition,
}

impl PackageReviewTerminalAuthorityPermission {
    pub const fn service(&self) -> &PackageReviewNominalIdentity {
        &self.service
    }

    pub const fn service_schema(&self) -> ServiceSchemaDigest {
        self.service_schema
    }

    pub fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    pub const fn permitted(&self) -> &TerminalAuthorityDisposition {
        &self.permitted
    }

    pub(crate) fn canonical_cmp(&self, other: &Self) -> std::cmp::Ordering {
        canonical_nominal_cmp(&self.service, &other.service)
            .then_with(|| {
                self.service_schema
                    .as_bytes()
                    .cmp(other.service_schema.as_bytes())
            })
            .then_with(|| {
                canonical_string_cmp(&self.requirement_identity, &other.requirement_identity)
            })
    }
}

fn canonical_nominal_cmp(
    left: &PackageReviewNominalIdentity,
    right: &PackageReviewNominalIdentity,
) -> std::cmp::Ordering {
    let (left_tag, left_digest) = canonical_nominal_owner(left.owner());
    let (right_tag, right_digest) = canonical_nominal_owner(right.owner());
    left_tag
        .cmp(&right_tag)
        .then_with(|| left_digest.cmp(&right_digest))
        .then_with(|| canonical_string_cmp(left.path(), right.path()))
}

fn canonical_nominal_owner(owner: PackageReviewNominalOwner) -> (u8, [u8; 32]) {
    match owner {
        PackageReviewNominalOwner::Package(package) => (0, package.digest()),
        PackageReviewNominalOwner::ToolchainSource(source) => (1, source.digest()),
        PackageReviewNominalOwner::Unresolved => (2, [0; 32]),
    }
}

fn canonical_string_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    u64::try_from(left.len())
        .expect("bounded review identity length fits u64")
        .to_le_bytes()
        .cmp(
            &u64::try_from(right.len())
                .expect("bounded review identity length fits u64")
                .to_le_bytes(),
        )
        .then_with(|| left.as_bytes().cmp(right.as_bytes()))
}
