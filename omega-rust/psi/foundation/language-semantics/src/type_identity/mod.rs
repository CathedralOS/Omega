//! Bounded package-owner traversal of the canonical type identity grammar.
//!
//! This validates structural framing, not source typing or declaration access.
//! Literal/path payloads remain opaque. Consumers own any binder replacement
//! grammar and the single resource account shared with containing identities.
//!
//! The grammar reads the exact-owner identities of retained package policy,
//! which `TypedTrees::exact_owner_type_identity` and `exact_owner_nominal_identity`
//! render. `vocabulary` closes every tag that text can carry: each reading
//! position matches its tags exhaustively, so a production is either read or
//! refused with a reason that says why no retained policy contains it.

mod callable_display;
mod framing;
mod grammar;
mod index;
mod lifetime_partition;
pub mod vocabulary;
pub use callable_display::callable_display_path;
pub use lifetime_partition::normalize_requirement_lifetime_partition;

/// Closed failures without source/compiler handles or allocated diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeIdentityVisitError {
    MalformedIdentity,
    ResourceLimitExceeded,
    AllocationFailed,
    UnknownPackage,
    UnsupportedEmbeddedName,
    /// A production the renderer writes that no retained package policy contains.
    Unadmitted(vocabulary::Unadmitted),
}

impl std::fmt::Display for TypeIdentityVisitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::MalformedIdentity => "malformed canonical type identity",
            Self::ResourceLimitExceeded => "canonical identity resource limit exceeded",
            Self::AllocationFailed => "canonical identity allocation failed",
            Self::UnknownPackage => "canonical identity refers to an unknown package",
            Self::UnsupportedEmbeddedName => "unsupported embedded canonical name",
            Self::Unadmitted(vocabulary::Unadmitted::InvalidIdentity) => {
                "canonical identity carries a marker of an invalid identity"
            }
            Self::Unadmitted(vocabulary::Unadmitted::ApplicationScope) => {
                "canonical identity carries a region of an application scope"
            }
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
    /// A name outside the vocabulary is a caller-supplied binder replacement. The
    /// consumer may traverse its own framed grammar here using this same resource
    /// state. Ordinary binder text is not searched for embedded package spellings.
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

/// Byte offset at which the grammar stopped reading `identity`, for failure
/// reports. `None` when the whole identity reads. The walk charges no budget and
/// never descends into embedded names, so it goes no deeper than a budgeted
/// visit that already failed; call it only after one has. Atoms decoded into
/// their own text (range bounds, qualified names) are read by a nested reader,
/// so for a failure inside one the offset is where the enclosing atom ended.
pub fn malformed_identity_offset(identity: &str) -> Option<usize> {
    struct Unbudgeted;
    impl TypeIdentityPackageOwnerVisitor for Unbudgeted {
        fn enter(&mut self) -> Result {
            Ok(())
        }
        fn leave(&mut self) {}
        fn reserve(&mut self, _: usize) -> Result {
            Ok(())
        }
        fn package_owner(&mut self, _: [u8; 32]) -> Result {
            Ok(())
        }
        fn embedded_name(&mut self, _: &str) -> Result {
            Ok(())
        }
    }
    let mut reader = framing::Reader::new(identity);
    match grammar::type_identity(&mut reader, &mut Unbudgeted).and_then(|()| reader.finish()) {
        Err(Error::MalformedIdentity | Error::Unadmitted(_)) => Some(reader.position()),
        _ => None,
    }
}

type Error = TypeIdentityVisitError;
type Result<T = (), E = Error> = std::result::Result<T, E>;

fn unadmitted(reason: vocabulary::Unadmitted) -> Error {
    Error::Unadmitted(reason)
}

fn scoped<T>(
    visitor: &mut dyn TypeIdentityPackageOwnerVisitor,
    action: impl FnOnce(&mut dyn TypeIdentityPackageOwnerVisitor) -> Result<T>,
) -> Result<T> {
    visitor.enter()?;
    let result = action(visitor);
    visitor.leave();
    result
}
