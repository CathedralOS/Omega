/*
We traverse source trees under capability-rooted no-follow handles and derive
their canonical content identity (`ResolvedSourceTree` / `ResolvedLocalSource`).

`capture` performs bounded, drift-checked traversal (`SourceTreePolicy`),
`filesystem` provides the underlying capability-relative directory and file
primitives, `identity` computes the `omega-source-tree-v4` domain digest, and
`resolved_tree` defines the source-neutral tree and entry records shared by
local and Git acquisition.
*/

/*
Because local directories and materialized snapshots must produce identical
content digests for identical trees across platforms, tree identity hashes
sorted relative path bytes, normalized directory modes (`0555`), executable bit
flags, file contents, and symlink target bytes while keeping host inode/ctime
observations strictly inside the capture drift guard.
*/

/*
- Bounded no-follow traversal and drift sweeps: `capture`
- Capability-relative open/read and drift comparison: `filesystem`
- Canonical `omega-source-tree-v4` hashing: `identity`
- Resolved tree and verified entry records: `resolved_tree`
*/

pub(crate) mod capture;
pub(crate) mod filesystem;
pub(crate) mod identity;
mod resolved_tree;

pub(crate) use resolved_tree::ResolvedSourceTree;
pub use resolved_tree::{
    ResolvedLocalSource, VerifiedPackageSourceEntry, VerifiedPackageSourceEntryKind,
};
