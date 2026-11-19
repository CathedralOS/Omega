//! The source files supplied to the lexer. The caller has captured the bytes,
//! assigned identifiers and selected the origin and scope of each occurrence.
#![forbid(unsafe_code)]

use arena::{Arena, HandleSpan};
use source_text::SourceId;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

pub use source_provenance::DependencyScope;

/// Package-relative directory components in their declared order. The file name is not a module.
pub fn directory_namespace<'a>(
    path: &'a Path,
    package_root: &Path,
) -> Result<Vec<&'a str>, String> {
    let relative = path.strip_prefix(package_root).map_err(|_| {
        format!(
            "source `{}` is outside its owning package root `{}`",
            path.display(),
            package_root.display()
        )
    })?;
    let mut namespace = Vec::new();
    for component in relative.parent().into_iter().flat_map(Path::components) {
        let Component::Normal(directory) = component else {
            return Err(format!(
                "source `{}` has a noncanonical package directory",
                path.display()
            ));
        };
        let valid = directory.to_str().filter(|name| {
            let mut bytes = name.bytes();
            bytes
                .next()
                .is_some_and(|first| first == b'_' || first.is_ascii_alphabetic())
                && bytes.all(|byte| byte == b'_' || byte.is_ascii_alphanumeric())
        });
        namespace.push(valid.ok_or_else(|| format!(
            "directory `{}` in source `{}` cannot name an Omega namespace; expected an ASCII identifier",
            directory.to_string_lossy(), path.display()
        ))?);
    }
    Ok(namespace)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, record_storage::Record)]
pub enum SourceOrigin {
    #[default]
    User,
    Toolchain,
    // Compiler-issued contract sources retain custody without joining the core prelude.
    ToolchainContract,
}

impl SourceOrigin {
    /// Provenance only: closed contracts do not gain ambient name visibility.
    pub const fn is_toolchain(self) -> bool {
        matches!(self, Self::Toolchain | Self::ToolchainContract)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSource {
    pub source_id: SourceId,
    pub path: PathBuf,
    pub source: Arc<str>,
    pub origin: Option<SourceOrigin>,
    pub scope: DependencyScope,
}

impl Default for LoadedSource {
    fn default() -> Self {
        Self {
            source_id: SourceId::default(),
            path: PathBuf::default(),
            source: Arc::from(""),
            origin: None,
            scope: DependencyScope::Product,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoadedSources {
    pub sources: Arena<LoadedSource>,
    pub batch: HandleSpan<LoadedSource>,
}

impl FromIterator<LoadedSource> for LoadedSources {
    fn from_iter<Inputs: IntoIterator<Item = LoadedSource>>(inputs: Inputs) -> Self {
        let inputs = inputs.into_iter();
        let mut sources = Arena::with_capacity(inputs.size_hint().0);
        let batch = sources.insert_many(inputs);
        Self { sources, batch }
    }
}

#[cfg(test)]
mod directory_tests {
    use super::*;

    #[test]
    fn namespace_uses_only_the_owned_parent_directories() {
        for root in [
            Path::new("/signed/first-checkout"),
            Path::new("/other/location"),
        ] {
            assert_eq!(
                directory_namespace(&root.join("combat/ai/targeting.omg"), root).unwrap(),
                ["combat", "ai"]
            );
            assert!(
                directory_namespace(&root.join("program.omg"), root)
                    .unwrap()
                    .is_empty()
            );
            assert!(
                directory_namespace(&root.join("bad-name/program.omg"), root)
                    .unwrap_err()
                    .contains("bad-name")
            );
            assert!(directory_namespace(Path::new("/elsewhere/program.omg"), root).is_err());
        }
    }
}
