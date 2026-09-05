//! Bounded package-owner traversal of the canonical type identity grammar.
//!
//! This validates structural framing, not source typing or declaration access.
//! Literal/path payloads remain opaque. Consumers own any binder replacement
//! grammar and the single resource account shared with containing identities.

mod framing;
mod grammar;
mod index;

/// Closed failures without source/compiler handles or allocated diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeIdentityVisitError {
    MalformedIdentity,
    ResourceLimitExceeded,
    AllocationFailed,
    UnknownPackage,
    UnsupportedEmbeddedName,
}

impl std::fmt::Display for TypeIdentityVisitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MalformedIdentity => "malformed canonical type identity",
            Self::ResourceLimitExceeded => "canonical identity resource limit exceeded",
            Self::AllocationFailed => "canonical identity allocation failed",
            Self::UnknownPackage => "canonical identity refers to an unknown package",
            Self::UnsupportedEmbeddedName => "unsupported embedded canonical name",
        })
    }
}

impl std::error::Error for TypeIdentityVisitError {}

/// Resource and semantic callbacks shared with the containing record visitor.
pub trait TypeIdentityPackageOwnerVisitor {
    /// Charge one node and one active depth before visiting it. Failed entry
    /// must leave active depth unchanged; every successful entry gets `leave`.
    fn enter(&mut self) -> Result<(), TypeIdentityVisitError>;
    fn leave(&mut self);
    /// Charge requested owned bytes before allocating an exact unescape buffer.
    /// Borrowed input, allocator overhead and stack buffers are excluded.
    fn reserve(&mut self, owned_bytes: usize) -> Result<(), TypeIdentityVisitError>;
    fn package_owner(&mut self, digest: [u8; 32]) -> Result<(), TypeIdentityVisitError>;
    /// A non-nominal name is a caller-supplied binder replacement. The consumer
    /// may traverse its own framed grammar here using this same resource state.
    /// Ordinary binder text is not searched for embedded package spellings.
    fn embedded_name(&mut self, name: &str) -> Result<(), TypeIdentityVisitError>;
}

/// Visit package owners in one complete runtime type identity. All recursive
/// work uses `visitor`'s existing budget; no counters are reset at this entry.
pub fn visit_type_identity_package_owners(
    identity: &str,
    visitor: &mut impl TypeIdentityPackageOwnerVisitor,
) -> Result<(), TypeIdentityVisitError> {
    let mut reader = framing::Reader::new(identity);
    grammar::type_identity(&mut reader, visitor)?;
    reader.finish()
}

type Error = TypeIdentityVisitError;
type Result<T = (), E = Error> = std::result::Result<T, E>;

fn scoped<T>(
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    action: impl FnOnce(&mut dyn TypeIdentityPackageOwnerVisitor) -> Result<T>,
) -> Result<T> {
    visitor.enter()?;
    let result = action(visitor);
    visitor.leave();
    result
}
