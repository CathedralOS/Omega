/*
    We resolve one Git request into exact package source custody: `request`
    owns the acquisition and selection input, `recorded` recovers historical
    revisions under accepted metadata, and `workspace` applies Omega
    declaration semantics for named workspace members.
*/

/*
    Git package source resolution belongs to the package source boundary
    (`resolution/source/git/mod.rs`).
*/

mod recorded;
mod request;
pub(crate) mod workspace;

pub(crate) use recorded::{RecordedGitPackage, recover_recorded_git_source};
pub(crate) use request::GitPackageSourceRequest;
