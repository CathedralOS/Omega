//! Seal an anonymous Build product under the identity of its captured source,
//! resolved dependencies, and applicable toolchain inputs. Before the Build
//! returns, a distinct provisional identity may be used only for checking.

use crate::declarations::DependencyPurpose;
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};
use source_acquisition::capture::{CapturedSourceNode, CapturedSourceTree};
use std::sync::Arc;

#[derive(Clone, record_storage::Record)]
pub struct AnonymousSourceCapture {
    pub(crate) build_owner: Option<PackageKeyIdentity>,
    pub(crate) acquisition: Option<crate::CapturedLocalSource>,
    pub(crate) root: std::path::PathBuf, // Canonical live root of the anonymous capture.
    pub(crate) capture: Arc<CapturedSourceTree>, // Complete captured source tree.
    pub(crate) filesystem: source_acquisition::capture::SourceFileSystem, // Captured virtual filesystem used during assembly.
    pub(crate) semantic_dependencies: Vec<[u8; 32]>, // Applicable core and imported toolchain source digests.
    pub(crate) sources: Arc<source_compiler::representations::source_inputs::SourceMap>, // Retained source map prior to owner binding.
    pub(crate) installed_roots: crate::source::source_imports::InstalledSourceRoots, // Closed installed source roots of this capture.
}

/// The real identity of a completed anonymous Build product, bound to the source
/// acquisition that issued it. Only that product's receiving Build may adopt it.
#[derive(Clone)]
pub struct CompletedAnonymousSource {
    pub(crate) owner: PackageKeyIdentity,
    acquisition: crate::CapturedLocalSource,
}
impl CompletedAnonymousSource {
    pub fn owner(&self) -> PackageKeyIdentity {
        self.owner
    }

    pub(crate) fn require_capture(
        &self,
        capture: Option<&crate::CapturedLocalSource>,
    ) -> Result<(), Vec<diagnostics::Diagnostic>> {
        if !capture.is_some_and(|source| Arc::ptr_eq(source.tree(), self.acquisition.tree())) {
            return Err(vec![diagnostics::Diagnostic::error(
                "anonymous Build product belongs to another captured source",
            )]);
        }
        Ok(())
    }
}

impl AnonymousSourceCapture {
    pub fn build_owner(&self) -> Option<PackageKeyIdentity> {
        self.build_owner
    }

    /// The unbound local acquisition whose source map this Build actually typed.
    pub fn acquired_for(
        &self,
        program: &source_compiler::representations::source_state::TypedTrees,
    ) -> Option<&crate::CapturedLocalSource> {
        let sources = program.symbols.source_map()?;
        if !Arc::ptr_eq(&sources, self.sources()) {
            return None;
        }
        self.acquisition.as_ref()
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    pub fn filesystem(&self) -> &source_acquisition::capture::SourceFileSystem {
        &self.filesystem
    }

    pub fn build_input(&self) -> Result<build_filesystem::trees::CapturedBuildSourceInput, String> {
        crate::source::source_snapshot::retained_source_input(&self.capture)
    }

    pub fn sources(&self) -> &Arc<source_compiler::representations::source_inputs::SourceMap> {
        &self.sources
    }

    /*
        The compiler checks the whole Build file before running it
        (`wiki/spec/build/command_line.md`), but naming a boundary the root
        declares needs an exact owner, and the root receives its real owner
        only after Build returns (`wiki/spec/build/declarations.md`). This
        owner lets that check name the root's declarations. A domain tag of its
        own keeps it distinct from every real owner. The check that uses it
        keeps only its verdict and the constants it read, and the root is
        checked again under its real owner once Build returns.
    */
    pub fn provisional_check_owner(&self) -> Result<PackageKeyIdentity, String> {
        let mut digest = Sha256::new();
        field(&mut digest, b"OMEGA-CHECK-ONLY-PROVISIONAL-OWNER-V1\0");
        snapshot(&mut digest, &self.capture)?;
        PackageKeyIdentity::from_digest(digest.finalize().into())
            .ok_or_else(|| "provisional check owner has the reserved zero digest".to_owned())
    }

    /// The Build stage with this capture's unowned files under the check-only owner,
    /// appended without mutating `files[..]` in place so `retains_source_storage` holds.
    pub fn check_only_stage(
        &self,
        mut stage: source_compiler::BuildStageSource,
    ) -> Result<source_compiler::BuildStageSource, String> {
        let owner = self.provisional_check_owner()?;
        let mut files = (*stage.parsed.files).clone();
        let remapped = files.rebind_anonymous_owner_retaining_storage(&self.root, owner)?;
        let remap = |id: source_text::SourceId| {
            remapped
                .iter()
                .find_map(|&(old_id, new_id)| (old_id == id).then_some(new_id))
                .unwrap_or(id)
        };
        let files = Arc::new(files);
        let roots: Vec<_> = stage.parsed.roots.into_iter().map(remap).collect();
        let mut imports = stage.parsed.imports;
        for binding in &mut imports {
            binding.remap_sources(remap);
        }
        stage.parsed = source_compiler::ParsedStageSources::parse(files, roots, imports)
            .map_err(|errors| diagnostics::join_messages(&errors))?;
        stage.build_sources = stage.build_sources.into_iter().map(remap).collect();
        stage.entry = source_text::SourceSpan::new(remap(stage.entry.source_id), stage.entry.span);
        for pending in &mut stage.pending_imports {
            pending.source = remap(pending.source);
        }
        Ok(stage)
    }

    pub(crate) fn completed_owner<'a>(
        &self,
        dependencies: impl IntoIterator<Item = ResolvedEdge<'a>>,
    ) -> Result<PackageKeyIdentity, Vec<diagnostics::Diagnostic>> {
        derive(
            &self.capture,
            self.semantic_dependencies.iter().copied(),
            dependencies,
        )
        .map_err(|reason| vec![diagnostics::Diagnostic::error(reason)])
    }

    pub(crate) fn complete_product<'a>(
        self,
        dependencies: impl IntoIterator<Item = ResolvedEdge<'a>>,
    ) -> Result<CompletedAnonymousSource, Vec<diagnostics::Diagnostic>> {
        let owner = self.completed_owner(dependencies)?;
        let acquisition = self.acquisition.ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "anonymous Build product lost its original source acquisition",
            )]
        })?;
        Ok(CompletedAnonymousSource { owner, acquisition })
    }
}

impl source_compiler::compile_time_invocations::InvocationSourceConsent for AnonymousSourceCapture {
    fn allows_selection(
        &self,
        sources: &Arc<source_compiler::representations::source_inputs::SourceMap>,
        requester: source_text::SourceSpan,
        declaration: source_text::SourceSpan,
        purpose: source_provenance::DependencyScope,
    ) -> bool {
        // This is the retained live Build input, not an anonymous package key.
        // A later Product cohort or another capture cannot borrow its consent.
        if purpose != source_provenance::DependencyScope::Build
            || !Arc::ptr_eq(sources, &self.sources)
        {
            return false;
        }
        [requester, declaration].into_iter().all(|span| {
            sources.get(span.source_id).is_some_and(|file| {
                sources.source_is_selected(span.source_id)
                    && file.origin == source_admission::SourceOrigin::User
                    && file.package_identity.is_none()
                    && file.package_root == self.root
                    && file.dependency_scope == purpose
                    && span.span.start <= span.span.end
                    && span.span.end <= file.source.len()
            })
        })
    }
}

fn field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
}

/// A direct registration in a completed anonymous Build. The resolution binds
/// the immutable acquisition made by the package resolver, separately from
/// the dependency's continuing package identity.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ResolvedEdge<'a> {
    pub purpose: DependencyPurpose,
    pub alias: &'a str,
    pub owner: PackageKeyIdentity,
    pub resolution: [u8; 32],
}

fn derive<'a>(
    capture: &CapturedSourceTree,
    core: impl IntoIterator<Item = [u8; 32]>,
    dependencies: impl IntoIterator<Item = ResolvedEdge<'a>>,
) -> Result<PackageKeyIdentity, String> {
    let mut digest = Sha256::new();
    field(&mut digest, b"OMEGA-ANONYMOUS-CONTENT-OWNER-V1\0");
    field(&mut digest, b"OMEGA-LANGUAGE-DEFAULT-BUILD-SEMANTICS-V1\0");
    snapshot(&mut digest, capture)?;
    let mut edges = dependencies.into_iter().collect::<Vec<_>>();
    edges.sort_unstable();
    if edges
        .windows(2)
        .any(|pair| pair[0].purpose == pair[1].purpose && pair[0].alias == pair[1].alias)
    {
        return Err("anonymous Build has two resolutions for one dependency registration".into());
    }
    digest.update((edges.len() as u64).to_le_bytes());
    for edge in edges {
        digest.update([edge.purpose.canonical_tag()]);
        field(&mut digest, edge.alias.as_bytes());
        field(&mut digest, &edge.owner.digest());
        field(&mut digest, &edge.resolution);
    }
    let mut core = core.into_iter().collect::<Vec<_>>();
    core.sort();
    core.dedup();
    digest.update((core.len() as u64).to_le_bytes());
    for identity in core {
        field(&mut digest, &identity);
    }
    PackageKeyIdentity::from_digest(digest.finalize().into())
        .ok_or_else(|| "anonymous owner has the reserved zero digest".to_owned())
}

pub(crate) fn core_snapshot_identity(capture: &CapturedSourceTree) -> Result<[u8; 32], String> {
    let mut digest = Sha256::new();
    field(&mut digest, b"OMEGA-ANONYMOUS-CORE-SNAPSHOT-V1\0");
    snapshot(&mut digest, capture)?;
    Ok(digest.finalize().into())
}

fn snapshot(digest: &mut Sha256, capture: &CapturedSourceTree) -> Result<(), String> {
    let mut entries = capture
        .entries()
        .map(|(path, entry)| {
            crate::source::source_snapshot::capture_relative_path_bytes(path)
                .map(|path| (path, entry))
        })
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by(|left, right| left.0.cmp(&right.0));
    digest.update((entries.len() as u64).to_le_bytes());
    for (path, entry) in entries {
        field(digest, &path);
        match entry {
            CapturedSourceNode::Directory => digest.update([0]),
            CapturedSourceNode::File { bytes, executable } => {
                digest.update([1, u8::from(*executable)]);
                field(digest, bytes);
            }
            CapturedSourceNode::Symlink { target } => {
                digest.update([2]);
                field(
                    digest,
                    &crate::source::canonical_fs::os_str_bytes(target.as_os_str())?,
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Arc};

    fn derive(
        capture: &CapturedSourceTree,
        core: impl IntoIterator<Item = [u8; 32]>,
    ) -> Result<PackageKeyIdentity, String> {
        super::derive(capture, core, [])
    }

    #[test]
    fn anonymous_owners_bind_canonical_purposes_aliases_and_immutable_resolutions() {
        use source_acquisition::{GitCommitId, GitTreeId, ImmutableSourceResolution};
        let capture = snapshot("build.omg", b"machine build() {}");
        let tree = GitTreeId::parse_hex(&"2".repeat(40)).unwrap();
        let revision = |c: char| {
            ImmutableSourceResolution::git(
                GitCommitId::parse_hex(&c.to_string().repeat(40)).unwrap(),
                tree.clone(),
            )
            .unwrap()
        };
        let edge = ResolvedEdge {
            purpose: DependencyPurpose::Product,
            alias: "util",
            owner: PackageKeyIdentity::from_digest([1; 32]).unwrap(),
            resolution: revision('1').identity_digest(),
        };
        let add = |edges: Vec<ResolvedEdge<'_>>| super::derive(&capture, [], edges).unwrap();
        let other = ResolvedEdge {
            alias: "kit",
            ..edge.clone()
        };
        let owner = add(vec![edge.clone(), other.clone()]);
        assert_eq!(owner, add(vec![other.clone(), edge.clone()]));
        for changed in [
            ResolvedEdge {
                purpose: DependencyPurpose::Build,
                ..edge.clone()
            },
            ResolvedEdge {
                alias: "helpers",
                ..edge.clone()
            },
            ResolvedEdge {
                owner: PackageKeyIdentity::from_digest([2; 32]).unwrap(),
                ..edge.clone()
            },
            ResolvedEdge {
                resolution: revision('3').identity_digest(),
                ..edge.clone()
            },
        ] {
            assert_ne!(owner, add(vec![changed, other.clone()]));
        }
        assert!(super::derive(&capture, [], [edge.clone(), edge]).is_err());
    }

    fn snapshot(path: &str, bytes: &[u8]) -> CapturedSourceTree {
        CapturedSourceTree::new([(
            PathBuf::from(path),
            CapturedSourceNode::File {
                bytes: Arc::from(bytes),
                executable: false,
            },
        )])
        .expect("closed source snapshot")
    }

    #[test]
    fn a_relocated_capture_has_the_same_owner_but_cannot_supply_the_other_activation() {
        use platform_custody::temporary_directory::TemporaryDirectory;
        let directory = TemporaryDirectory::create("completed-build-source", false).unwrap();
        let capture = |name: &str| {
            let root = directory.path().join(name);
            std::fs::create_dir(&root).unwrap();
            std::fs::write(
                root.join("entry.omg"),
                "machine build(builder: @mutable Build) {}",
            )
            .unwrap();
            crate::capture_anonymous_compilation_source(&root).unwrap()
        };
        let first = capture("first");
        let second = capture("second");
        let close = |acquisition: &crate::CapturedLocalSource| {
            AnonymousSourceCapture {
                build_owner: None,
                root: acquisition.canonical_live_root().into(),
                capture: Arc::clone(acquisition.tree()),
                acquisition: Some(acquisition.clone()),
                filesystem: Default::default(),
                semantic_dependencies: vec![[13; 32]],
                sources: Arc::default(),
                installed_roots: Default::default(),
            }
            .complete_product([])
            .unwrap()
        };
        let a = close(&first);
        let b = close(&second);
        assert_eq!(a.owner(), b.owner());
        a.require_capture(Some(&first.clone())).unwrap();
        assert!(a.require_capture(Some(&second)).is_err());
        let reacquired =
            crate::capture_anonymous_compilation_source(first.canonical_live_root()).unwrap();
        assert_eq!(a.owner(), close(&reacquired).owner());
        assert!(a.require_capture(Some(&reacquired)).is_err());
        assert!(a.require_capture(None).is_err());
    }

    #[test]
    fn build_source_consent_keeps_the_actual_capture_purpose_and_root() {
        use source_compiler::compile_time_invocations::InvocationSourceConsent;
        let root = std::env::temp_dir()
            .canonicalize()
            .unwrap()
            .join("anonymous");
        let text = "machine local() {}";
        let tree = Arc::new(snapshot("build.omg", text.as_bytes()));
        let mut filesystem = source_acquisition::capture::SourceFileSystem::default();
        filesystem
            .add_capture(root.clone(), Arc::clone(&tree))
            .unwrap();
        let mut sources = source_compiler::representations::source_inputs::SourceMap::default();
        let id = sources
            .add_checked_instance(
                root.join("build.omg"),
                text,
                root.clone(),
                None,
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                source_provenance::DependencyScope::Build,
            )
            .source_id;
        let sources = Arc::new(sources);
        let capture = AnonymousSourceCapture {
            build_owner: None,
            acquisition: None,
            root,
            capture: tree,
            filesystem,
            semantic_dependencies: Vec::new(),
            sources: Arc::clone(&sources),
            installed_roots: Default::default(),
        };
        let span = source_text::SourceSpan::new(id, source_text::Span::new(0, text.len()));
        assert!(capture.allows_selection(
            &sources,
            span,
            span,
            source_provenance::DependencyScope::Build
        ));
        assert!(sources.get(id).unwrap().package_identity.is_none());
        assert!(!capture.allows_selection(
            &sources,
            span,
            span,
            source_provenance::DependencyScope::Product
        ));
        let foreign = Arc::new(sources.as_ref().clone());
        assert!(!capture.allows_selection(
            &foreign,
            span,
            span,
            source_provenance::DependencyScope::Build
        ));
        let outside = source_text::SourceSpan::new(id, source_text::Span::new(0, text.len() + 1));
        assert!(!capture.allows_selection(
            &sources,
            span,
            outside,
            source_provenance::DependencyScope::Build
        ));
        let missing = source_text::SourceSpan::new(source_text::SourceId(usize::MAX), span.span);
        assert!(!capture.allows_selection(
            &sources,
            span,
            missing,
            source_provenance::DependencyScope::Build
        ));
        let mut another_root = capture;
        another_root.root = PathBuf::from("/another-root");
        assert!(!another_root.allows_selection(
            &sources,
            span,
            span,
            source_provenance::DependencyScope::Build
        ));
    }

    #[test]
    fn canonical_record_retains_source_and_core_without_instance_order() {
        let source = snapshot("library.omg", b"public data Token {}");
        let owner = derive(&source, [[1; 32], [2; 32]]).unwrap();
        assert_eq!(
            owner.digest(),
            [
                53, 179, 79, 52, 161, 231, 70, 176, 53, 17, 97, 63, 133, 76, 192, 249, 53, 186,
                154, 205, 196, 211, 104, 141, 69, 99, 254, 91, 79, 148, 35, 9
            ]
        );
        assert_eq!(owner, derive(&source, [[2; 32], [1; 32], [2; 32]]).unwrap());
        assert_ne!(owner, derive(&source, [[1; 32], [3; 32]]).unwrap());
        assert_ne!(
            owner,
            derive(
                &snapshot("other.omg", b"public data Token {}"),
                [[1; 32], [2; 32]]
            )
            .unwrap()
        );
        assert_ne!(
            owner,
            derive(
                &snapshot("library.omg", b"public data Token { value: u8; }"),
                [[1; 32], [2; 32]]
            )
            .unwrap()
        );
        let expanded = CapturedSourceTree::new([
            (
                PathBuf::from("library.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"public data Token {}".as_slice()),
                    executable: false,
                },
            ),
            (
                PathBuf::from("unused.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"const SEMANTIC_INPUT: u8 = 1;".as_slice()),
                    executable: false,
                },
            ),
        ])
        .unwrap();
        assert_ne!(
            owner,
            derive(&expanded, [[1; 32], [2; 32]]).unwrap(),
            "the entire capture participates, including unselected files"
        );
    }

    #[test]
    fn closed_core_commitment_includes_undemanded_semantic_inputs() {
        let source = snapshot("library.omg", b"public data Token {}");
        let core = snapshot("core.omg", b"public data Core {}");
        let expanded = CapturedSourceTree::new([
            (
                PathBuf::from("core.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"public data Core {}".as_slice()),
                    executable: false,
                },
            ),
            (
                PathBuf::from("other.omg"),
                CapturedSourceNode::File {
                    bytes: Arc::from(b"public data Other {}".as_slice()),
                    executable: false,
                },
            ),
        ])
        .unwrap();
        let before = core_snapshot_identity(&core).unwrap();
        let after = core_snapshot_identity(&expanded).unwrap();
        assert_ne!(before, after);
        assert_ne!(
            derive(&source, [before]).unwrap(),
            derive(&source, [after]).unwrap()
        );
    }

    #[test]
    fn source_binding_keeps_supplied_and_core_owners_and_checked_instances() {
        let identity = derive(&snapshot("library.omg", b"public data Token {}"), []).unwrap();
        let supplied = PackageKeyIdentity::from_digest([23; 32]).unwrap();
        let root = PathBuf::from("/anonymous");
        let mut sources = source_compiler::representations::source_inputs::SourceMap::default();
        for scope in [
            source_provenance::DependencyScope::Product,
            source_provenance::DependencyScope::Build,
        ] {
            sources.add_checked_instance(
                root.join("library.omg"),
                "public data Token {}".to_owned(),
                root.clone(),
                None,
                source_admission::SourceOrigin::User,
                source_admission::SourceResolutionStratum::Base,
                scope,
            );
        }
        sources.add_with_metadata(
            PathBuf::from("/supplier/types.omg"),
            "public data Foreign {}".to_owned(),
            PathBuf::from("/supplier"),
            Some(supplied),
            source_admission::SourceOrigin::User,
        );
        sources.add_with_metadata(
            PathBuf::from("/toolchain/core.omg"),
            "public data Core {}".to_owned(),
            PathBuf::from("/toolchain"),
            None,
            source_admission::SourceOrigin::Toolchain,
        );
        sources.bind_anonymous_owner(&root, identity).unwrap();
        assert_eq!(
            sources
                .get(source_text::SourceId(0))
                .unwrap()
                .package_identity,
            Some(identity)
        );
        assert_eq!(
            sources
                .get(source_text::SourceId(1))
                .unwrap()
                .package_identity,
            Some(identity)
        );
        assert_eq!(
            sources
                .get(source_text::SourceId(2))
                .unwrap()
                .package_identity,
            Some(supplied)
        );
        assert_eq!(
            sources
                .get(source_text::SourceId(3))
                .unwrap()
                .package_identity,
            None
        );
        assert_ne!(
            sources
                .get(source_text::SourceId(0))
                .unwrap()
                .dependency_scope,
            sources
                .get(source_text::SourceId(1))
                .unwrap()
                .dependency_scope
        );
        let before = sources.clone();
        sources.bind_anonymous_owner(&root, supplied).unwrap();
        assert_eq!(sources, before, "a supplied owner is never rebound");
    }

    #[test]
    fn missing_foreign_provenance_rejects_without_partial_binding() {
        let owner = derive(&snapshot("library.omg", b"public data Token {}"), []).unwrap();
        let root = PathBuf::from("/anonymous");
        let mut sources = source_compiler::representations::source_inputs::SourceMap::default();
        sources.add_with_metadata(
            root.join("library.omg"),
            String::new(),
            root.clone(),
            None,
            source_admission::SourceOrigin::User,
        );
        sources.add_with_metadata(
            PathBuf::from("/foreign/copied.omg"),
            String::new(),
            PathBuf::from("/foreign"),
            None,
            source_admission::SourceOrigin::User,
        );
        let before = sources.clone();
        let error = sources.bind_anonymous_owner(&root, owner).unwrap_err();
        assert!(error.contains("no supplied owner outside the anonymous capture"));
        assert_eq!(sources, before);
    }
}
