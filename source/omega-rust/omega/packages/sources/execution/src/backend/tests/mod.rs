use super::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
#[cfg(windows)]
use crate::ResolverExecutionGuaranteeDisposition;
use crate::ResolverExecutionPhase;
use std::path::Path;

mod observations;
#[cfg(unix)]
mod unix;
mod validation;

fn inspection_root() -> std::path::PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary inspection root")
}
