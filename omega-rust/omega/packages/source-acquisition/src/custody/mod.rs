/*
We enforce filesystem custody across local snapshots and Git cache entries.

`tree` authenticates retained cache trees and ancestry from the filesystem root
down, `platform` applies host ownership, mode, and macOS extended ACL policy,
`lock` serializes per-entry cache operations through retained parent handles,
and `publication` manages private staging directories and atomic publication.
*/

/*
Shared per-user cache directories are exposed to concurrent compiler invocations,
symlink or junction substitution, and permissive ACLs. Binding every cache entry,
lock file, and staging directory to a capability-rooted parent handle — and
reconciling pathname identity before and after mutations — prevents path-swap
races and external write access from contaminating cached sources.
*/

/*
- Capability-rooted tree and ancestry verification: `tree`
- Host ownership, permission modes, and macOS ACL checks: `platform`
- Contended per-entry advisory lock custody: `lock`
- Private staging and atomic directory publication: `publication`
*/

pub(crate) mod lock;
pub(crate) mod platform;
pub(crate) mod publication;
pub(crate) mod tree;

#[cfg(test)]
mod tests;
