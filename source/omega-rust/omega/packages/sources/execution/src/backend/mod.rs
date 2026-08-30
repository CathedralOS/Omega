//! Host backend selection and preparation of confined resolver launches.

mod host;
mod observation;
mod preparation;
mod request;

#[cfg(target_os = "macos")]
use crate::confinement;
use crate::model::ResolverExecutionBackendIdentity;
use std::path::Path;

#[derive(Debug)]
pub struct ResolverExecutionBackend {
    pub(crate) identity: ResolverExecutionBackendIdentity,
    #[cfg(target_os = "macos")]
    pub(crate) sandbox_metadata: confinement::macos::ExecutableMetadataIdentity,
}

#[derive(Clone, Copy)]
pub(crate) struct ResolverExecutionAuthorityRoots<'a> {
    pub(crate) discovery_read_root: Option<&'a Path>,
    pub(crate) inspection_read_root: Option<&'a Path>,
    pub(crate) mutable_root: Option<&'a Path>,
}

#[cfg(test)]
mod tests;
