#![forbid(unsafe_code)]

//! Psi-owned loading of root Omega source files.

use std::path::Path;

use psi_diagnostics::Diagnostic;
use psi_source::{SourceFile, SourceMap};

#[derive(Debug, Default)]
pub struct Resolver {
    source_map: SourceMap,
}

impl Resolver {
    pub fn load_root(&mut self, path: &Path) -> Result<&SourceFile, Diagnostic> {
        let source = std::fs::read_to_string(path).map_err(|error| {
            Diagnostic::error(format!("failed to read {}: {error}", path.display()))
        })?;

        Ok(self.source_map.add(path.to_path_buf(), source))
    }
}
