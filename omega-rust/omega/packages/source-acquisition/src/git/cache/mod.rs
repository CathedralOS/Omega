/*
Git repository cache creation, retained custody verification, and invalidation.

We store cached bare Git repositories in per-locator, per-revision,
per-transport cache entries (`identity`). `creation` initializes new entries
with an empty template and compiler-owned canonical configuration (`SHA-1` or
`SHA-256`), `repository` opens and verifies retained entries (`VerifiedGitRepository`)
while rejecting external object indirection (`objects/info/alternates`,
`objects/info/http-alternates`, `commondir`), and `snapshots` retains the
immutable `snapshots/` publication collection inside each entry.
*/

/*
Why invalidation removes metadata rather than trusting partial state:

When a cache entry fails custody or object authentication, `invalidation`
removes `GIT_CACHE_METADATA` through the retained capability directory so
subsequent resolutions treat the entry as incomplete. On a later acquisition
with network access (`retry`), `discard_incomplete_entry` verifies custody of
the incomplete entry and removes the entire entry directory rather than
reconstructing metadata over unverified objects.
*/

pub(crate) mod configuration;
pub(crate) mod creation;
pub(crate) mod custody;
pub(crate) mod identity;
pub(crate) mod invalidation;
pub(crate) mod repository;
pub(crate) mod retry;
pub(crate) mod snapshots;
