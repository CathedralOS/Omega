use filesystem_authority::FsGrants;

use crate::FilesystemSponsor;

/// One compiler-issued logical root over retained storage, not a host path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapturedFilesystemRoot {
    pub identity: crate::FilesystemGrantRootIdentity,
    pub snapshot: source::SourceSnapshot,
}

/// How checked-tree execution serves a program's filesystem capability.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FilesystemAccess {
    #[default]
    Virtual,
    RealUnscoped,
    RealScoped(FsGrants),
    RealScopedSponsored {
        grants: FsGrants,
        sponsor: FilesystemSponsor,
    },
    /// Captured logical inputs plus separately authorized physical roots.
    /// No captured root participates in host path resolution. Native-only
    /// operations on its descriptors explicitly refuse; pure-result reuse is
    /// still separate from this filesystem-capable execution mode.
    CapturedScoped {
        inputs: Vec<CapturedFilesystemRoot>,
        grants: FsGrants,
        sponsor: Option<FilesystemSponsor>,
    },
}
