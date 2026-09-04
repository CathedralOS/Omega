use super::{ResolverExecutionAuthorityRoots, ResolverExecutionBackend};
use crate::ResolverExecutionPhase;
use std::path::{Path, PathBuf};

#[cfg(unix)]
mod unix;
mod validation;

fn inspection_root() -> std::path::PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temporary inspection root")
}

fn resolver_executable() -> PathBuf {
    #[cfg(windows)]
    let path = Path::new(r"C:\Windows\System32\cmd.exe");
    #[cfg(not(windows))]
    let path = Path::new("/bin/sh");
    path.canonicalize().expect("canonical resolver executable")
}

fn backend() -> ResolverExecutionBackend {
    ResolverExecutionBackend::open(&resolver_executable(), &[] as &[PathBuf])
        .expect("open resolver backend")
}
