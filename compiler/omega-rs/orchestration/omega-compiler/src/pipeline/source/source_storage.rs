use crate::pipeline::frontend::ParsedSources;
use crate::pipeline::source::SourceFile;
use crate::source::SourceMap;
use psi_arena::Arena;
use psi_diagnostics::Diagnostic;
use psi_source::SourceOrigin;
use psi_syntax_trees::SyntaxTrees;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct SourceStorage {
    pub files: Arena<SourceFile>,
    pub sources: SourceMap,
    pub syntax_trees: SyntaxTrees,
    default_package_root: PathBuf,
    package_roots: Vec<PathBuf>,
    toolchain_root: Option<PathBuf>,
}

impl SourceStorage {
    pub fn for_compilation(default_package_root: PathBuf, toolchain_root: PathBuf) -> Self {
        Self {
            default_package_root: normalize_directory(default_package_root),
            toolchain_root: Some(normalize_directory(toolchain_root)),
            ..Self::default()
        }
    }

    pub fn register_package_root(&mut self, package_root: PathBuf) {
        let package_root = normalize_directory(package_root);
        if !self.package_roots.contains(&package_root) {
            self.package_roots.push(package_root);
            self.package_roots
                .sort_by_key(|root| std::cmp::Reverse(root.components().count()));
        }
    }

    pub fn extend(&mut self, parsed: ParsedSources) -> Result<(), Vec<Diagnostic>> {
        for parsed_source in parsed.sources.span_or_empty(parsed.batch) {
            let (package_root, origin) = self.source_metadata(&parsed_source.path);
            let added = self.sources.add_with_metadata(
                parsed_source.path.clone(),
                parsed_source.source.to_string(),
                package_root,
                origin,
            );

            debug_assert_eq!(added.source_id, parsed_source.source_id);

            self.files.append(SourceFile {
                source_id: parsed_source.source_id,
                path: parsed_source.path.clone(),
                root_items: parsed_source.root_items.clone(),
            });
        }

        Ok(())
    }

    pub fn next_source_id(&self) -> usize {
        self.sources.len()
    }

    pub fn file_count(&self) -> usize {
        self.files.len()
    }

    fn source_metadata(&self, path: &Path) -> (PathBuf, SourceOrigin) {
        if let Some(toolchain_root) = &self.toolchain_root
            && path.starts_with(toolchain_root)
        {
            let language = toolchain_root.join("language");
            let package_root = ["core", "std"]
                .into_iter()
                .map(|name| language.join(name))
                .find(|root| path.starts_with(root))
                .unwrap_or_else(|| toolchain_root.clone());
            return (package_root, SourceOrigin::Toolchain);
        }

        let package_root = self
            .package_roots
            .iter()
            .find(|root| path.starts_with(root))
            .cloned()
            .unwrap_or_else(|| self.default_package_root.clone());
        (package_root, SourceOrigin::User)
    }
}

fn normalize_directory(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}
