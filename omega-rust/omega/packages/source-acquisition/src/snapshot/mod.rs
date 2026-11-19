/*
We construct, seal, verify, and atomically publish immutable source snapshots
under capability-rooted directory handles.

Both Git materialization (`git`) and local-source snapshotting (`local`) share
this custody machinery so every published snapshot tree is created with
no-follow handles, sealed read-only (`0555` directories, `0444`/`0555` files on
Unix; readonly attribute on Windows), and verified against its metadata record.
*/

/*
Writing source files into a shared cache without staging and sealing would
expose downstream readers to half-written trees or in-place mutation. By staging
every snapshot in a process-unique directory (`PendingMaterializedSnapshot`),
sealing permissions before or immediately across publication, and removing
unpublished stages on drop, we guarantee published snapshot directories are
complete and immutable.
*/

/*
- Capability-relative file, directory, and symlink creation: `construction`
- Snapshot metadata encoding and verification: `metadata`
- Canonical read-only modes and drop cleanup: `permissions`
- Staging and atomic directory publication: `publication`
*/

pub(crate) mod construction;
pub(crate) mod metadata;
pub(crate) mod permissions;
pub(crate) mod publication;
