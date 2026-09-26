//! Filesystem grant roots, refusals, authorized paths and access.

use crate::FilesystemSponsor;
use crate::filesystem::CanonicalFilesystemMetadataIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemObservationProvider {
    /// Deterministic in-memory provider; no host filesystem was touched.
    Virtual,
    /// Real process filesystem without path grants. Build admission does not
    /// select this provider.
    RealUnscoped,
    /// Real filesystem constrained by compiler-supplied path grants.
    RealScoped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemGrantAccess {
    Read,
    Write,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemGrantRefusalReason {
    Unresolvable,
    OutsideGrantedRoots,
    UnrepresentableRootedPath,
    ObservationEvidenceLimitExceeded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FilesystemGrantRefusal {
    pub(crate) operand_ordinal: u8,
    pub(crate) access: FilesystemGrantAccess,
    pub(crate) reason: FilesystemGrantRefusalReason,
}

/// Compiler-issued identity for one scoped filesystem grant root.
///
/// The checked interpreter treats this as an opaque coordinate. The caller
/// owns its meaning (for example, Omega assigns distinct identities to the
/// immutable package source and writable build output roots). Zero is reserved
/// so an omitted/default identity cannot enter evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FilesystemGrantRootIdentity(u32);

/// Current compiler sponsorship ceiling for one canonical path beneath a
/// filesystem grant root. This is an evaluator evidence limit, not a language
/// limit.
pub const FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT: usize = 16 * 1024 * 1024;

/// Whether bytes are the canonical target-neutral spelling of a path beneath
/// one grant root. Authorized targets may denote the root itself; authored
/// rooted operands may not.
pub fn filesystem_root_relative_path_is_canonical(relative: &[u8], allow_empty: bool) -> bool {
    if relative.len() > FILESYSTEM_ROOT_RELATIVE_PATH_BYTE_LIMIT
        || relative.contains(&0)
        || std::str::from_utf8(relative).is_err()
    {
        return false;
    }
    if relative.is_empty() {
        return allow_empty;
    }
    if relative[0] == b'/'
        || relative.contains(&b'\\')
        || (relative.len() >= 2 && relative[1] == b':')
    {
        return false;
    }
    !relative
        .split(|byte| *byte == b'/')
        .any(|component| component.is_empty() || component == b"." || component == b"..")
}

impl FilesystemGrantRootIdentity {
    pub const fn new(value: u32) -> Option<Self> {
        if value == 0 { None } else { Some(Self(value)) }
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One compiler-supplied physical grant root and its evidence identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemGrantRoot {
    pub(crate) identity: FilesystemGrantRootIdentity,
    pub(crate) path: std::path::PathBuf,
    pub(crate) canonical_metadata: Option<CanonicalFilesystemMetadataIndex>,
}

impl FilesystemGrantRoot {
    pub fn new(identity: FilesystemGrantRootIdentity, path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            identity,
            path: path.into(),
            canonical_metadata: None,
        }
    }

    /// Attach canonical immutable-source metadata to this grant root.
    pub fn with_canonical_metadata(
        mut self,
        canonical_metadata: CanonicalFilesystemMetadataIndex,
    ) -> Self {
        self.canonical_metadata = Some(canonical_metadata);
        self
    }

    pub const fn identity(&self) -> FilesystemGrantRootIdentity {
        self.identity
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub const fn canonical_metadata(&self) -> Option<&CanonicalFilesystemMetadataIndex> {
        self.canonical_metadata.as_ref()
    }
}

/// One scoped path that passed the grant gate before host access.
///
/// `relative_path` uses `/` between UTF-8 components, never carries a leading
/// separator, and is empty for the root itself. It therefore contains no host
/// absolute path or compiler working-directory spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemAuthorizedPath {
    pub(crate) operand_ordinal: u8,
    pub(crate) access: FilesystemGrantAccess,
    pub(crate) root: FilesystemGrantRootIdentity,
    pub(crate) relative_path: Vec<u8>,
}

impl FilesystemAuthorizedPath {
    pub const fn operand_ordinal(&self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(&self) -> FilesystemGrantAccess {
        self.access
    }

    pub const fn root(&self) -> FilesystemGrantRootIdentity {
        self.root
    }

    pub fn relative_path(&self) -> &[u8] {
        &self.relative_path
    }
}

impl FilesystemGrantRefusal {
    pub const fn operand_ordinal(self) -> u8 {
        self.operand_ordinal
    }

    pub const fn access(self) -> FilesystemGrantAccess {
        self.access
    }

    pub const fn reason(self) -> FilesystemGrantRefusalReason {
        self.reason
    }
}

/// How the interpreter serves a program's `Filesystem` capability.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilesystemAccess {
    /// The deterministic in-memory filesystem (the default; the differential
    /// oracle). Hermetic: no real disk is ever touched.
    #[default]
    Virtual,
    /// The REAL host filesystem, UNSCOPED: fs ops act on real paths with the
    /// invoking process's full authority. The trust level of `build.rs`; for
    /// build.omg proper, prefer [`FilesystemAccess::RealScoped`] -- the grants
    /// ARE the audit surface (open-work #3's settled design).
    RealUnscoped,
    /// The REAL host filesystem behind PATH GRANTS (build.omg rung 2): reads
    /// must land under a read or write root, writes/creates/removes under a
    /// write root; anything else is refused with EACCES before the OS is
    /// touched. build.omg's shape: read = source tree, write = build dir.
    RealScoped(FsGrants),
    /// The same path authority, plus a compiler-owned resource sponsor shared
    /// across every build-machine evaluation in one package-review session.
    /// The program cannot inspect or enlarge this account.
    RealScopedSponsored {
        grants: FsGrants,
        sponsor: FilesystemSponsor,
    },
}

/// Path grants for [`FilesystemAccess::RealScoped`]. Roots are canonicalized
/// when the run starts (so symlinked spellings of a root work), and every
/// op's path is canonicalized before the prefix check (so `..` traversal and
/// symlinks INSIDE a granted tree that point OUTSIDE it are resolved and
/// refused, not string-matched).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FsGrants {
    /// Trees the program may READ from (open read-only, stat). A write root
    /// implicitly grants read-back -- staging then verifying is the normal
    /// build shape -- so these are the read-ONLY trees.
    pub read_roots: Vec<FilesystemGrantRoot>,
    /// Trees the program may WRITE under: create/truncate/append opens,
    /// remove, create_dir/remove_dir, and BOTH ends of a rename.
    pub write_roots: Vec<FilesystemGrantRoot>,
}
