//! Original complete input capture for the focused-file compilation route.
//!
//! The local acquisition owner supplies the existing package source policy,
//! drift checks and immutable bytes. Source and toolchain roots share one
//! captured filesystem before project discovery or parsing; overlapping roots
//! reuse one capture. Paths select backing and never identify an anonymous owner.
//! This capture is an input to that owner, not its completed identity: exact
//! resolved dependency occurrences and language/default-Build semantic inputs
//! still need their original producers. Compiler/profile/backend fingerprints
//! cannot substitute for them or grant receiving consent.

use diagnostics::Diagnostic;
use package_source::local::operations::{capture_local_source_input, project_local_source_scope};
use package_source::{LocalSourceCapturePolicy, LocalSourceLimits};
use source::SourceFileSystem;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct UnmanagedSourceCapture {
    selected_root: PathBuf,
    source_root: PathBuf,
    toolchain_root: PathBuf,
    filesystem: Arc<SourceFileSystem>,
    requested_build_directory: Option<PathBuf>,
    original_build_vocabulary: Arc<str>,
    source_root_revision: [u8; 32],
}

impl UnmanagedSourceCapture {
    pub(super) fn prepare(
        root_path: &Path,
        toolchain_root: &Path,
        build_directory: Option<&Path>,
        original_build_vocabulary: Arc<str>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let directory_input = root_path.is_dir();
        let source_root = if directory_input {
            root_path
        } else {
            root_path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."))
        };
        let source_root = source_root
            .canonicalize()
            .map_err(|error| failure(source_root, error))?;
        // Resolve the input's parent before capture, retaining the selected
        // leaf inside that original namespace. Later host aliases cannot
        // redirect project discovery to another physical root.
        let selected_root = if directory_input {
            source_root.clone()
        } else {
            source_root.join(
                root_path
                    .file_name()
                    .ok_or_else(|| failure(root_path, "source input has no file name"))?,
            )
        };
        let toolchain_root = toolchain_root
            .canonicalize()
            .map_err(|error| failure(toolchain_root, error))?;
        let mut roots = Vec::with_capacity(2);
        if source_root.starts_with(&toolchain_root) {
            roots.push(toolchain_root.clone());
        } else {
            roots.push(source_root.clone());
            if !toolchain_root.starts_with(&source_root) {
                roots.push(toolchain_root.clone());
            }
        }
        // Resolve the original output coordinate at the existing write-fence
        // owner before acquisition. Keep both its spelling and resolved target:
        // an owned symlink cannot leave its destination in the source inventory.
        let effective_output = build_directory
            .map(Path::to_path_buf)
            .unwrap_or_else(|| source_root.join("build"));
        let output_spelling = crate::build_execution::lexical_absolute(&effective_output);
        let output_target = crate::build_execution::overlap_key(&effective_output);
        if source_root.starts_with(&output_target) {
            return Err(failure(
                &source_root,
                "build write root must not cover the source root",
            ));
        }
        let source_has_own_mount = roots.iter().any(|root| root == &source_root);
        let mut filesystem = SourceFileSystem::new();
        for root in roots {
            if root.starts_with(&output_target) {
                return Err(failure(
                    &root,
                    "build write root must not cover a captured input root",
                ));
            }
            let owned_outputs = relative_owned_outputs(&root, &output_spelling, &output_target);
            let policy = LocalSourceCapturePolicy::new(&owned_outputs)
                .map_err(|error| failure(&root, error))?;
            let captured = capture_local_source_input(&root, LocalSourceLimits::default(), policy)
                .map_err(|error| failure(&root, error))?;
            filesystem
                .add_capture(
                    captured.canonical_live_root().to_path_buf(),
                    Arc::clone(captured.tree()),
                )
                .map_err(|error| failure(&root, error))?;
        }
        if !source_has_own_mount {
            // The larger toolchain capture observes drift once. Its nested
            // project still owns root-level lock/output reservations; narrow
            // those retained bytes under the same local-source policy before
            // either frontend reads or source revision projection consume them.
            let captured = filesystem
                .captured_tree_at(&source_root)
                .map_err(|error| failure(&source_root, error))?
                .ok_or_else(|| failure(&source_root, "original consuming root has no capture"))?;
            let owned_outputs =
                relative_owned_outputs(&source_root, &output_spelling, &output_target);
            let policy = LocalSourceCapturePolicy::new(&owned_outputs)
                .map_err(|error| failure(&source_root, error))?;
            let scoped = project_local_source_scope(&captured, policy)
                .map_err(|error| failure(&source_root, error))?;
            filesystem
                .add_capture(source_root.clone(), scoped)
                .map_err(|error| failure(&source_root, error))?;
        }
        // Bind the original consuming root inventory while acquisition still
        // owns it. Projection reads the captured namespace, including contained
        // roots that share one mount; it never reopens a physical checkout.
        let tree = filesystem
            .captured_tree_at(&source_root)
            .map_err(|error| failure(&source_root, error))?
            .ok_or_else(|| {
                failure(
                    &source_root,
                    "original source root has no captured inventory",
                )
            })?;
        let input = crate::source_snapshot::retained_source_input(&tree)
            .map_err(|error| failure(&source_root, error))?;
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
            b"OMEGA-CAPTURED-SOURCE-ROOT-REVISION-V1\0",
        );
        crate::source_snapshot::hash_canonical_source_metadata(
            &mut digest,
            input.canonical_source_metadata(),
        );
        let source_root_revision = digest.finish();
        Ok(Self {
            selected_root,
            source_root,
            toolchain_root,
            filesystem: Arc::new(filesystem),
            requested_build_directory: build_directory.map(Path::to_path_buf),
            original_build_vocabulary,
            source_root_revision,
        })
    }

    /// A checkpoint has already consumed an immutable input inventory. Changing
    /// its output exclusion requires a fresh invocation, never recapture or a
    /// different Build context behind retained frontend work. Exact request
    /// equality deliberately does not inspect the host after acquisition.
    pub fn admit_build_directory(
        &self,
        build_directory: Option<&Path>,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.requested_build_directory.as_deref() != build_directory {
            return Err(failure(
                &self.source_root,
                "checked child output context does not match its prepared source checkpoint",
            ));
        }
        Ok(())
    }

    /// Exact original compiled Build source consumed by prelude injection.
    /// This byte input cannot stand for the complete language/default behavior
    /// record or supply the absent ordinary dependency owner/resolution rows.
    pub fn original_build_vocabulary(&self) -> &Arc<str> {
        &self.original_build_vocabulary
    }

    pub fn selected_root(&self) -> &Path {
        &self.selected_root
    }

    /// Exact captured inventory only. Ordinary dependency/default semantics
    /// still supply the distinct completed anonymous owner; no authority is
    /// granted by this source revision.
    pub const fn source_root_revision(&self) -> [u8; 32] {
        self.source_root_revision
    }

    pub fn source_root(&self) -> &Path {
        &self.source_root
    }
    pub fn toolchain_root(&self) -> &Path {
        &self.toolchain_root
    }
    pub fn filesystem(&self) -> &Arc<SourceFileSystem> {
        &self.filesystem
    }
}

fn relative_owned_outputs(root: &Path, spelling: &Path, target: &Path) -> Vec<PathBuf> {
    let mut owned_outputs = Vec::with_capacity(2);
    for output in [spelling, target] {
        if let Ok(relative) = output.strip_prefix(root)
            && !relative.as_os_str().is_empty()
            && !owned_outputs.iter().any(|retained| retained == relative)
        {
            owned_outputs.push(relative.to_path_buf());
        }
    }
    owned_outputs
}

fn failure(root: &Path, error: impl std::fmt::Display) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "could not capture compilation source input {}: {error}",
        root.display()
    ))]
}
