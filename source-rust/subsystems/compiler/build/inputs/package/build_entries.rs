//! Dependency Build entries belong to the acquisition that actually entered them.
use source_admission::{RetainedDeclaration, SourceOrigin};
use source_capture::CapturedSourceTree;
use source_text::Span;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, Default, PartialEq, Eq, record_storage::Record)]
pub struct PackageBuildEntries {
    // Package-relative source, byte commitment, and declaration name span.
    files: BTreeMap<String, ([u8; 32], Span)>,
}

impl PackageBuildEntries {
    pub fn from_selected<'a>(
        root: &Path,
        tree: &CapturedSourceTree,
        declarations: impl IntoIterator<Item = &'a RetainedDeclaration>,
    ) -> Result<Self, String> {
        let mut entries = Self::default();
        for declaration in declarations {
            let source = declaration.source();
            if source.origin == SourceOrigin::Toolchain
                && source.path == Path::new(source_admission::source_identity::DEFAULT_BUILD_PATH)
            {
                continue;
            }
            if declaration.kind() != symbols::SymbolKind::Machine
                || source.origin != SourceOrigin::User
                || source.dependency_scope != source_provenance::DependencyScope::Build
                || source.package_root != root
            {
                return Err(
                    "selected Build entry is outside its original source acquisition".into(),
                );
            }
            let file = source.logical_filename()?;
            let entry = (
                crate::digest_encoder::content_identity(source.source.as_bytes()),
                declaration.span(),
            );
            if entries
                .files
                .insert(file, entry)
                .is_some_and(|previous| previous != entry)
            {
                return Err("Build stages disagree on the selected declaration's source".into());
            }
        }
        entries.require_capture(tree)?;
        Ok(entries)
    }

    pub(crate) fn require_capture(&self, tree: &CapturedSourceTree) -> Result<(), String> {
        for (name, (digest, span)) in &self.files {
            let path = Path::new(name);
            let source = tree
                .read_to_string(path)
                .map_err(|error| error.to_string())?;
            if tree.canonicalize(path).map_err(|error| error.to_string())? != path
                || crate::digest_encoder::content_identity(source.as_bytes()) != *digest
                || source.get(span.start..span.end) != Some("build")
            {
                return Err(format!(
                    "entered Build declaration does not match {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    pub(super) fn hash_into(&self, digest: &mut crate::digest_encoder::DomainDigestEncoder) {
        digest.u64(self.files.len() as u64);
        for (name, (content, span)) in &self.files {
            digest.bytes(name.as_bytes());
            digest.raw(content);
            digest.u64(span.start as u64);
            digest.u64(span.end as u64);
        }
    }

    pub(crate) fn filenames(&self) -> impl Iterator<Item = &Path> {
        self.files.keys().map(Path::new)
    }

    pub(super) fn contains(&self, path: &Path) -> bool {
        path.to_str().is_some_and(|name| {
            if std::path::MAIN_SEPARATOR == '/' {
                self.files.contains_key(name)
            } else {
                self.files
                    .contains_key(&name.replace(std::path::MAIN_SEPARATOR, "/"))
            }
        })
    }
}
