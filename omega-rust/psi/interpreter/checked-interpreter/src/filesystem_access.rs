use filesystem_authority::FsGrants;

use crate::FilesystemSponsor;

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
}
