//! One selected executable and preparation of bounded resolver launches.

mod preparation;
mod request;

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct ResolverExecutionBackend {
    executable: PathBuf,
}

#[derive(Clone, Copy)]
pub(crate) struct ResolverExecutionAuthorityRoots<'a> {
    pub(crate) discovery_read_root: Option<&'a Path>,
    pub(crate) inspection_read_root: Option<&'a Path>,
    pub(crate) mutable_root: Option<&'a Path>,
}

#[cfg(test)]
mod tests;
