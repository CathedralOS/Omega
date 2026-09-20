//! Scoped Source/Output authority, staging custody, and sponsored host access.

pub mod preparation;

use crate::{
    BuildCanonicalSourceMetadataIdentity,
    evidence::observations::{BuildActivation, BuildCapturedSourceInventory},
};
use build_output::{
    BuildStagedOutputEntryKind, BuildStagedOutputTree, CapturedBuildSourceInput, capture,
    discard_materialized_snapshot, empty,
};
use build_time_evaluation::{
    BuildMachineFilesystemAccess, BuildMachineFilesystemGrantRoot,
    BuildMachineFilesystemGrantRootIdentity, BuildMachineFilesystemGrants,
    BuildMachineFilesystemSponsor,
};
use checked_interpreter::FilesystemSponsorEntry;
use diagnostics::Diagnostic;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const BUILD_SOURCE_ROOT_IDENTITY: BuildMachineFilesystemGrantRootIdentity =
    match BuildMachineFilesystemGrantRootIdentity::new(1) {
        Some(identity) => identity,
        None => panic!("build source root identity must be nonzero"),
    };
pub const BUILD_OUTPUT_ROOT_IDENTITY: BuildMachineFilesystemGrantRootIdentity =
    match BuildMachineFilesystemGrantRootIdentity::new(2) {
        Some(identity) => identity,
        None => panic!("build output root identity must be nonzero"),
    };

/// Host filesystem authority granted to an admitted `build.omg` machine.
///
/// Source roots are read-only. The build directory is the only write root and
/// also permits read-back through the checked interpreter's `RealScoped`
/// contract.
///
/// When a captured source input is bound, source reads execute against a
/// fresh private materialization of that inventory — never the live source
/// root — while the inventory's canonical metadata index narrows every
/// granted operation to captured membership and kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMachineFilesystemScope {
    source_root: PathBuf,
    canonical_source_metadata: Option<checked_interpreter::CanonicalFilesystemMetadataIndex>,
    canonical_source_metadata_required: bool,
    build_dir: PathBuf,
    sponsor: Option<BuildMachineFilesystemSponsor>,
    // Ordinary snapshot compilation owns fresh scratch, not the publication
    // directory. Cloned admission state shares its lifetime through capture.
    output_scratch: Option<std::sync::Arc<preparation::BuildOutputScratch>>,
    root_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    root_role: Option<package_compilation::BuildDeclarationKind>,
    build_execution_profile: Option<target::TargetProfile>,
    captured_source_input: Option<CapturedBuildSourceInput>,
    snapshot_dir: Option<PathBuf>,
    required_outputs: BTreeSet<Vec<u8>>,
    dependency_inputs: BTreeMap<
        package_compilation::BuildDependencyOccurrence,
        BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
    >,
}

/// Maximum declared required sealed outputs for one build occurrence.
const REQUIRED_BUILD_OUTPUT_LIMIT: usize = 4_096;

/// Release handle for one occurrence's private captured-source backing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedSnapshotRelease {
    snapshot_dir: Option<PathBuf>,
}

impl CapturedSnapshotRelease {
    /// Discard the private materialization if this occurrence bound one.
    /// An already-absent backing is not an error, so the release may run on
    /// the settlement path and again on the occurrence's final exit.
    pub(crate) fn release(&self) {
        if let Some(snapshot_dir) = &self.snapshot_dir {
            let _ = discard_materialized_snapshot(snapshot_dir);
        }
    }
}

impl BuildMachineFilesystemScope {
    pub fn for_root(
        root_path: &Path,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
    ) -> Self {
        let source_root = root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            source_root,
            canonical_source_metadata: None,
            canonical_source_metadata_required: false,
            build_dir,
            sponsor,
            output_scratch: None,
            root_package_identity: None,
            root_role: None,
            build_execution_profile: None,
            captured_source_input: None,
            snapshot_dir: None,
            required_outputs: BTreeSet::new(),
            dependency_inputs: BTreeMap::new(),
        }
    }

    #[doc(hidden)]
    pub fn for_package_root(
        source_root: PathBuf,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
        metadata: Option<checked_interpreter::CanonicalFilesystemMetadataIndex>,
    ) -> Self {
        Self {
            source_root,
            canonical_source_metadata: metadata,
            canonical_source_metadata_required: true,
            build_dir,
            sponsor,
            output_scratch: None,
            root_package_identity: None,
            root_role: None,
            build_execution_profile: None,
            captured_source_input: None,
            snapshot_dir: None,
            required_outputs: BTreeSet::new(),
            dependency_inputs: BTreeMap::new(),
        }
    }

    /// Bind the root package occurrence and authored declaration role whose
    /// validated inputs produced this scope.
    pub fn with_package_activation(
        mut self,
        root_package_identity: semantic_vocabulary::PackageKeyIdentity,
        root_role: package_compilation::BuildDeclarationKind,
    ) -> Self {
        self.root_package_identity = Some(root_package_identity);
        self.root_role = Some(root_role);
        self
    }

    /// Bind the admitted build execution profile the requesting compilation
    /// checks its build-scope sources for and runs its build machine under.
    /// It is a request fact (the compiler host when the request names none),
    /// never inferred here from the compiler process. `None` records an
    /// admitted host no catalogued profile describes rather than naming one.
    pub fn with_execution_profile(
        mut self,
        build_execution_profile: Option<target::TargetProfile>,
    ) -> Self {
        self.build_execution_profile = build_execution_profile;
        self
    }

    /// Bind this build occurrence's source reads to one compiler-captured
    /// immutable input inventory.
    ///
    /// The inventory's canonical metadata index must equal any index the
    /// sponsor already validated, and becomes the index bound into the
    /// build's Source grant. `snapshot_dir` names the fresh private
    /// materialization backing the Source root for this occurrence; it must
    /// not overlap the source root or the build write root.
    pub fn with_captured_source_input(
        mut self,
        input: CapturedBuildSourceInput,
        snapshot_dir: PathBuf,
    ) -> Result<Self, Vec<Diagnostic>> {
        if let Some(existing) = &self.canonical_source_metadata {
            if existing != input.canonical_source_metadata() {
                return Err(vec![Diagnostic::error(
                    "captured build source input's canonical Source metadata does not match the binding's validated index",
                )]);
            }
        } else {
            self.canonical_source_metadata = Some(input.canonical_source_metadata().clone());
        }
        self.bind_captured_source_input(input, snapshot_dir)
    }

    /// Keep full package provenance while narrowing the build's read grant.
    /// The complete capture must match resolver custody, and the selected
    /// entries must match its exact bytes, not just its file lengths.
    fn with_scoped_package_source_input(
        self,
        input: CapturedBuildSourceInput,
        complete: &CapturedBuildSourceInput,
        snapshot_dir: PathBuf,
    ) -> Result<Self, Vec<Diagnostic>> {
        if self.canonical_source_metadata.as_ref() != Some(complete.canonical_source_metadata())
            || !input.is_subset_of(complete)
        {
            return Err(vec![Diagnostic::error(
                "scoped build source input does not match the validated complete package capture",
            )]);
        }
        self.bind_captured_source_input(input, snapshot_dir)
    }

    fn bind_captured_source_input(
        mut self,
        input: CapturedBuildSourceInput,
        snapshot_dir: PathBuf,
    ) -> Result<Self, Vec<Diagnostic>> {
        if snapshot_dir == self.build_dir
            || snapshot_dir.starts_with(&self.build_dir)
            || self.build_dir.starts_with(&snapshot_dir)
        {
            return Err(vec![Diagnostic::error(format!(
                "captured source snapshot directory `{}` must not overlap the build write root `{}`",
                snapshot_dir.display(),
                self.build_dir.display()
            ))]);
        }
        if snapshot_dir == self.source_root
            || snapshot_dir.starts_with(&self.source_root)
            || self.source_root.starts_with(&snapshot_dir)
        {
            return Err(vec![Diagnostic::error(format!(
                "captured source snapshot directory `{}` must not overlap the source root `{}`",
                snapshot_dir.display(),
                self.source_root.display()
            ))]);
        }
        self.captured_source_input = Some(input);
        self.snapshot_dir = Some(snapshot_dir);
        Ok(self)
    }

    /// Bind caller-captured immutable inputs assigned to exact dependency
    /// occurrences. The request's binding already validated every key
    /// against the reconciled graph, so this custody move never rereads the
    /// host and cannot widen an occurrence's inputs.
    pub fn with_dependency_inputs(
        mut self,
        inputs: BTreeMap<
            package_compilation::BuildDependencyOccurrence,
            BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
        >,
    ) -> Self {
        self.dependency_inputs = inputs;
        self
    }

    /// Inputs assigned to one exact dependency occurrence, in slot order.
    pub fn dependency_inputs(
        &self,
        occurrence: &package_compilation::BuildDependencyOccurrence,
    ) -> Option<&BTreeMap<Vec<u8>, CapturedBuildSourceInput>> {
        self.dependency_inputs.get(occurrence)
    }

    /// Declare the required sealed outputs this build occurrence must
    /// complete before its result may publish. Each name is a canonical
    /// slash-separated relative path naming exactly one sealed regular file
    /// in the final staged-output tree.
    pub fn with_required_outputs(
        mut self,
        required_outputs: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut roster = BTreeSet::<Vec<u8>>::new();
        for name in required_outputs {
            if roster.len() >= REQUIRED_BUILD_OUTPUT_LIMIT {
                return Err(vec![Diagnostic::error(format!(
                    "required build outputs exceed the {REQUIRED_BUILD_OUTPUT_LIMIT}-entry ceiling"
                ))]);
            }
            if !checked_interpreter::canonical_filesystem_metadata_path_is_canonical(&name, false) {
                return Err(vec![Diagnostic::error(format!(
                    "required build output name is not a canonical relative path: {name:?}"
                ))]);
            }
            for existing in &roster {
                let mut prefixed = existing.clone();
                prefixed.push(b'/');
                let mut name_prefixed = name.clone();
                name_prefixed.push(b'/');
                if *existing == name
                    || name.starts_with(&prefixed)
                    || existing.starts_with(&name_prefixed)
                {
                    return Err(vec![Diagnostic::error(format!(
                        "required build output `{}` duplicates or collides with `{}`",
                        String::from_utf8_lossy(&name),
                        String::from_utf8_lossy(existing)
                    ))]);
                }
            }
            roster.insert(name);
        }
        self.required_outputs = roster;
        Ok(self)
    }

    /// The activation this scope describes: the bound package occurrence
    /// members and admitted execution profile plus the selected target the
    /// requesting compilation asked for.
    pub(crate) fn activation(
        &self,
        selected_target_profile: Option<target::TargetProfile>,
    ) -> BuildActivation {
        BuildActivation {
            root_package_identity: self.root_package_identity,
            root_role: self.root_role,
            selected_target_profile,
            build_execution_profile: self.build_execution_profile,
        }
    }

    /// The declared sealed outputs this occurrence must settle before its
    /// result may publish, in canonical order.
    pub(crate) fn required_outputs(&self) -> &BTreeSet<Vec<u8>> {
        &self.required_outputs
    }

    pub(crate) fn captured_source_inventory(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured_source_input
            .as_ref()
            .map(|input| BuildCapturedSourceInventory {
                entry_count: input.entry_count(),
                file_bytes: input.file_bytes(),
                source_metadata_identity: BuildCanonicalSourceMetadataIdentity::new(
                    input.canonical_source_metadata().policy_version(),
                    *input
                        .canonical_source_metadata()
                        .source_content_commitment(),
                ),
            })
    }

    pub(crate) fn filesystem_access(&self) -> BuildMachineFilesystemAccess {
        // A captured input switches the Source grant to this occurrence's
        // fresh private materialization. Its selected index narrows access;
        // the full package index remains separate package provenance.
        let read_root = self
            .snapshot_dir
            .clone()
            .unwrap_or_else(|| self.source_root.clone());
        let mut source_root =
            BuildMachineFilesystemGrantRoot::new(BUILD_SOURCE_ROOT_IDENTITY, read_root);
        if let Some(metadata) = self
            .captured_source_input
            .as_ref()
            .map(CapturedBuildSourceInput::canonical_source_metadata)
            .or(self.canonical_source_metadata.as_ref())
        {
            source_root = source_root.with_canonical_metadata(metadata.clone());
        }
        let grants = BuildMachineFilesystemGrants {
            read_roots: vec![source_root],
            write_roots: vec![BuildMachineFilesystemGrantRoot::new(
                BUILD_OUTPUT_ROOT_IDENTITY,
                self.build_dir.clone(),
            )],
        };
        match &self.sponsor {
            Some(sponsor) => BuildMachineFilesystemAccess::RealScopedSponsored {
                grants,
                sponsor: sponsor.clone(),
            },
            None => BuildMachineFilesystemAccess::RealScoped(grants),
        }
    }

    pub(crate) fn ensure_write_roots(&self) -> Result<(), Vec<Diagnostic>> {
        if let Some(sponsor) = &self.sponsor {
            let path = sponsor
                .bind_path(&self.build_dir)
                .map_err(|error| self.sponsor_diagnostic(error))?;
            match sponsor
                .entry(&path)
                .map_err(|error| self.sponsor_diagnostic(error))?
            {
                Some(FilesystemSponsorEntry::Directory) => return Ok(()),
                Some(_) => {
                    return Err(vec![Diagnostic::error(format!(
                        "sponsored build machine write root `{}` is not a directory",
                        self.build_dir.display()
                    ))]);
                }
                None => {}
            }
            let prepared = sponsor
                .prepare_create_directory(&path)
                .map_err(|error| self.sponsor_diagnostic(error))?;
            if let Err(error) = std::fs::create_dir(&self.build_dir) {
                prepared.abort();
                return Err(vec![Diagnostic::error(format!(
                    "failed to create sponsored build machine filesystem write root `{}`: {error}",
                    self.build_dir.display()
                ))]);
            }
            if let Err(error) = prepared.commit() {
                let _ = std::fs::remove_dir(&self.build_dir);
                return Err(self.sponsor_diagnostic(error));
            }
            return Ok(());
        }
        std::fs::create_dir_all(&self.build_dir).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to create build machine filesystem write root `{}`: {error}",
                self.build_dir.display()
            ))]
        })
    }

    pub(crate) fn ensure_canonical_source_metadata(&self) -> Result<(), Vec<Diagnostic>> {
        if self.canonical_source_metadata_required && self.canonical_source_metadata.is_none() {
            return Err(vec![Diagnostic::error(
                "package-aware build filesystem access requires compiler-validated canonical Source metadata",
            )]);
        }
        Ok(())
    }

    /// Materialize this occurrence's captured source input into its fresh
    /// private backing directory, replacing any residue from a previous
    /// occurrence. The backing is created read-only and independently
    /// re-inspected before the build is admitted to it.
    pub(crate) fn ensure_captured_snapshot(&self) -> Result<(), Vec<Diagnostic>> {
        let (Some(input), Some(snapshot_dir)) = (&self.captured_source_input, &self.snapshot_dir)
        else {
            return Ok(());
        };
        match std::fs::symlink_metadata(snapshot_dir) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                discard_materialized_snapshot(snapshot_dir).map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "failed to clear the previous captured source snapshot `{}`: {}",
                        snapshot_dir.display(),
                        error.message()
                    ))]
                })?;
            }
            Ok(_) => {
                return Err(vec![Diagnostic::error(format!(
                    "captured source snapshot `{}` exists and is not a concrete directory",
                    snapshot_dir.display()
                ))]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(vec![Diagnostic::error(format!(
                    "failed to inspect the captured source snapshot `{}`: {error}",
                    snapshot_dir.display()
                ))]);
            }
        }
        std::fs::create_dir(snapshot_dir).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to create the captured source snapshot `{}`: {error}",
                snapshot_dir.display()
            ))]
        })?;
        if let Err(error) = input.materialize_into(snapshot_dir) {
            let _ = discard_materialized_snapshot(snapshot_dir);
            return Err(vec![Diagnostic::error(format!(
                "failed to materialize the captured source snapshot `{}`: {}",
                snapshot_dir.display(),
                error.message()
            ))]);
        }
        Ok(())
    }

    /// Verify every declared required output is completed by exactly one
    /// sealed regular file in this occurrence's retained staged-output tree.
    /// Completion is linear: a required member cannot be omitted, and a
    /// directory or inert link at its path does not complete it.
    pub(crate) fn verify_required_outputs(
        &self,
        staged_output_tree: Option<&BuildStagedOutputTree>,
        machine_name: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        if self.required_outputs.is_empty() {
            return Ok(());
        }
        let Some(tree) = staged_output_tree else {
            return Err(vec![Diagnostic::error(format!(
                "build-time evaluation of `{machine_name}` cannot complete its required outputs without staged-output custody"
            ))]);
        };
        for name in &self.required_outputs {
            match tree.sealed_entry(name) {
                Some(entry) if matches!(entry.kind(), BuildStagedOutputEntryKind::File { .. }) => {}
                Some(_) => {
                    return Err(vec![Diagnostic::error(format!(
                        "required build output `{}` of `{machine_name}` is not a sealed regular file",
                        String::from_utf8_lossy(name)
                    ))]);
                }
                None => {
                    return Err(vec![Diagnostic::error(format!(
                        "required build output `{}` of `{machine_name}` was omitted from the sealed staged-output tree",
                        String::from_utf8_lossy(name)
                    ))]);
                }
            }
        }
        Ok(())
    }

    pub(crate) const fn canonical_source_metadata_identity(
        &self,
    ) -> Option<BuildCanonicalSourceMetadataIdentity> {
        match &self.canonical_source_metadata {
            Some(metadata) => Some(BuildCanonicalSourceMetadataIdentity {
                policy_version: metadata.policy_version(),
                source_content_commitment: *metadata.source_content_commitment(),
            }),
            None => None,
        }
    }

    fn sponsor_diagnostic(
        &self,
        error: checked_interpreter::FilesystemSponsorError,
    ) -> Vec<Diagnostic> {
        vec![Diagnostic::error(format!(
            "build machine staging sponsor rejected `{}`: {error}",
            self.build_dir.display()
        ))]
    }

    /// The release handle for this occurrence's private captured-source
    /// materialization. The backing is scratch for one run only: it must be
    /// discarded on every exit of the occurrence, including evaluator halts
    /// and rejected settlements, so a failed build leaves no residue behind.
    /// The handle carries only the backing path, so the caller can hold it
    /// across the run without cloning retained inventory bytes.
    pub(crate) fn captured_snapshot_release(&self) -> CapturedSnapshotRelease {
        CapturedSnapshotRelease {
            snapshot_dir: self.snapshot_dir.clone(),
        }
    }

    pub(crate) fn staged_output_tree(
        &self,
        filesystem_reachable: bool,
    ) -> Result<Option<BuildStagedOutputTree>, Vec<Diagnostic>> {
        // The private captured-source backing is scratch for the completed
        // run only; release it before staged-output custody is captured.
        self.captured_snapshot_release().release();
        // When the selected build machine cannot reach the filesystem, the
        // Output namespace is exactly empty without needing a physical
        // staging sponsor. Canonicalize that semantic fact so sponsored
        // review and ordinary production retain the same observation.
        if !filesystem_reachable {
            return Ok(Some(empty()));
        }
        let Some(sponsor) = &self.sponsor else {
            return Ok(None);
        };
        capture(&self.build_dir, sponsor).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::BuildMachineFilesystemScope;
    use build_output::{CapturedBuildSourceInput, CapturedSourceEntry};
    use build_time_evaluation::BuildMachineFilesystemAccess;
    use checked_interpreter::{
        CanonicalFilesystemMetadataIndex, CanonicalFilesystemMetadataRow,
        CanonicalFilesystemMetadataRowKind, FilesystemSponsor, FilesystemSponsorLimits,
    };
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    static STAGING_TEST_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_staging_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "omega-build-staging-{label}-{}-{}",
            std::process::id(),
            STAGING_TEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn sponsored_build_roots_share_one_session_entry_ceiling() {
        let session_root = temporary_staging_root("shared-account");
        fs::create_dir(&session_root).expect("create session root");
        let sponsor = FilesystemSponsor::with_limits(
            &session_root,
            FilesystemSponsorLimits {
                maximum_entries: 1,
                maximum_total_logical_bytes: 64,
                maximum_object_extent: 64,
            },
        )
        .expect("create filesystem sponsor");

        let first_build_dir = session_root.join("first-package");
        BuildMachineFilesystemScope::for_root(
            &session_root.join("first-source/main.omg"),
            first_build_dir.clone(),
            Some(sponsor.clone()),
        )
        .ensure_write_roots()
        .expect("first package consumes the one available entry");

        let second_build_dir = session_root.join("second-package");
        let diagnostics = BuildMachineFilesystemScope::for_root(
            &session_root.join("second-source/main.omg"),
            second_build_dir.clone(),
            Some(sponsor.clone()),
        )
        .ensure_write_roots()
        .expect_err("the second package must share the exhausted session ceiling");

        assert_eq!(sponsor.snapshot().unwrap().entries, 1);
        assert!(first_build_dir.is_dir());
        assert!(!second_build_dir.exists());
        assert!(diagnostics[0].to_string().contains("entry limit 1"));

        fs::remove_dir_all(session_root).expect("remove session root");
    }

    #[test]
    fn build_scope_attaches_canonical_metadata_only_to_source() {
        let metadata = checked_interpreter::CanonicalFilesystemMetadataIndex::version_1(
            [0x5a; 32],
            [checked_interpreter::CanonicalFilesystemMetadataRow::new(
                Vec::new(),
                checked_interpreter::CanonicalFilesystemMetadataRowKind::Directory,
            )],
        )
        .expect("construct canonical source metadata");
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("source"),
            PathBuf::from("build"),
            None,
            Some(metadata.clone()),
        );

        let BuildMachineFilesystemAccess::RealScoped(grants) = scope.filesystem_access() else {
            panic!("unsponsored build scope must use scoped real filesystem grants")
        };
        assert_eq!(grants.read_roots.len(), 1);
        assert_eq!(grants.write_roots.len(), 1);
        assert_eq!(grants.read_roots[0].canonical_metadata(), Some(&metadata));
        assert!(grants.write_roots[0].canonical_metadata().is_none());
    }

    #[test]
    fn package_build_scope_rejects_filesystem_authority_without_canonical_source_metadata() {
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("package-root"),
            PathBuf::from("build"),
            None,
            None,
        );

        let diagnostics = scope
            .ensure_canonical_source_metadata()
            .expect_err("package filesystem access must not fall back to physical metadata");
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].message.contains("canonical Source metadata"));
        assert_eq!(scope.source_root, PathBuf::from("package-root"));
    }

    fn captured_input() -> CapturedBuildSourceInput {
        let index = CanonicalFilesystemMetadataIndex::version_1(
            [0x5a; 32],
            [
                CanonicalFilesystemMetadataRow::new(
                    Vec::new(),
                    CanonicalFilesystemMetadataRowKind::Directory,
                ),
                CanonicalFilesystemMetadataRow::new(
                    b"template.tmpl".to_vec(),
                    CanonicalFilesystemMetadataRowKind::File {
                        executable: false,
                        logical_byte_length: 6,
                    },
                ),
            ],
        )
        .expect("construct canonical source metadata");
        CapturedBuildSourceInput::from_capture_rows(
            index,
            [(
                b"template.tmpl".to_vec(),
                CapturedSourceEntry::file(b"sealed".to_vec(), false),
            )],
        )
        .expect("assemble captured build source input")
    }

    #[test]
    fn captured_source_scope_materializes_a_fresh_private_snapshot() {
        let fixture = temporary_staging_root("snapshot");
        let source = fixture.join("source");
        let snapshot = fixture.join("snapshot-backing");
        fs::create_dir_all(&source).expect("create source dir");
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            fixture.join("build"),
            None,
            Some(captured_input().canonical_source_metadata().clone()),
        )
        .with_captured_source_input(captured_input(), snapshot.clone())
        .expect("bind the captured source input");

        scope
            .ensure_captured_snapshot()
            .expect("materialize the fresh private snapshot");
        assert_eq!(
            fs::read(snapshot.join("template.tmpl")).expect("read snapshot backing"),
            b"sealed"
        );
        let BuildMachineFilesystemAccess::RealScoped(grants) = scope.filesystem_access() else {
            panic!("unsponsored captured scope keeps scoped real grants")
        };
        assert_eq!(
            grants.read_roots[0].path(),
            snapshot.as_path(),
            "the Source grant reads the private snapshot, not the live root"
        );
        let inventory = scope
            .captured_source_inventory()
            .expect("captured run retains inventory evidence");
        assert_eq!(inventory.entry_count(), 2);
        assert_eq!(inventory.file_bytes(), 6);

        fs::remove_dir_all(&fixture).expect("remove fixture");
    }

    #[test]
    fn captured_snapshot_release_discards_the_private_backing_on_every_exit() {
        let fixture = temporary_staging_root("snapshot-release");
        let source = fixture.join("source");
        let snapshot = fixture.join("snapshot-backing");
        fs::create_dir_all(&source).expect("create source dir");
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            fixture.join("build"),
            None,
            Some(captured_input().canonical_source_metadata().clone()),
        )
        .with_captured_source_input(captured_input(), snapshot.clone())
        .expect("bind the captured source input");
        // The handle is taken before the run, as the executor does, and
        // outlives the scope's own settlement release.
        let release = scope.captured_snapshot_release();

        scope
            .ensure_captured_snapshot()
            .expect("materialize the fresh private snapshot");
        assert!(snapshot.join("template.tmpl").is_file());
        // A run that halts before settlement never reaches
        // `staged_output_tree`; the executor's final release still clears it.
        release.release();
        assert!(
            !snapshot.exists(),
            "the private backing must not outlive the occurrence"
        );
        // Releasing again after the settlement path already ran is inert.
        release.release();
        assert!(!snapshot.exists());
        assert!(
            source.exists(),
            "release never touches the live source root"
        );

        // A scope without a captured input releases nothing.
        BuildMachineFilesystemScope::for_root(
            &source.join("main.omg"),
            fixture.join("build"),
            None,
        )
        .captured_snapshot_release()
        .release();

        fs::remove_dir_all(&fixture).expect("remove fixture");
    }

    #[test]
    fn captured_source_scope_rejects_an_index_mismatch_and_overlap() {
        let fixture = temporary_staging_root("snapshot-mismatch");
        let source = fixture.join("source");
        fs::create_dir_all(&source).expect("create source dir");
        let other_index = CanonicalFilesystemMetadataIndex::version_1(
            [0x99; 32],
            [CanonicalFilesystemMetadataRow::new(
                Vec::new(),
                CanonicalFilesystemMetadataRowKind::Directory,
            )],
        )
        .expect("construct other index");
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            fixture.join("build"),
            None,
            Some(other_index),
        );
        let diagnostics = scope
            .with_captured_source_input(captured_input(), fixture.join("snapshot"))
            .expect_err("a captured input must match the validated index");
        assert!(diagnostics[0].to_string().contains("does not match"));

        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            fixture.join("build"),
            None,
            None,
        );
        let diagnostics = scope
            .with_captured_source_input(captured_input(), source.clone())
            .expect_err("the snapshot backing must not overlap the source root");
        assert!(diagnostics[0].to_string().contains("source root"));

        fs::remove_dir_all(&fixture).expect("remove fixture");
    }

    #[test]
    fn scoped_package_capture_keeps_provenance_but_grants_only_selected_members() {
        let complete = captured_input();
        let selected_metadata = CanonicalFilesystemMetadataIndex::version_1(
            [0x17; 32],
            [CanonicalFilesystemMetadataRow::new(
                Vec::new(),
                CanonicalFilesystemMetadataRowKind::Directory,
            )],
        )
        .unwrap();
        let selected =
            CapturedBuildSourceInput::from_capture_rows(selected_metadata.clone(), []).unwrap();
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("source"),
            PathBuf::from("output"),
            None,
            Some(complete.canonical_source_metadata().clone()),
        )
        .with_scoped_package_source_input(selected, &complete, PathBuf::from("snapshot"))
        .unwrap();
        assert_eq!(
            scope.canonical_source_metadata.as_ref(),
            Some(complete.canonical_source_metadata())
        );
        assert_eq!(
            scope
                .captured_source_inventory()
                .unwrap()
                .source_metadata_identity()
                .source_content_commitment(),
            [0x17; 32]
        );
        let BuildMachineFilesystemAccess::RealScoped(grants) = scope.filesystem_access() else {
            panic!("captured scope must retain scoped reads")
        };
        assert_eq!(
            grants.read_roots[0].canonical_metadata(),
            Some(&selected_metadata)
        );
    }

    #[test]
    fn required_output_roster_rejects_noncanonical_duplicate_and_prefix_names() {
        let scope = || {
            BuildMachineFilesystemScope::for_package_root(
                PathBuf::from("source"),
                PathBuf::from("build"),
                None,
                None,
            )
        };
        assert!(
            scope()
                .with_required_outputs([b"artifact.txt".to_vec()])
                .is_ok()
        );
        for bad in [
            Vec::new(),
            b"./dot".to_vec(),
            b"../parent".to_vec(),
            b"nested//gap".to_vec(),
            b"back\\slash".to_vec(),
            b"/absolute".to_vec(),
        ] {
            assert!(
                scope().with_required_outputs([bad]).is_err(),
                "noncanonical required names reject"
            );
        }
        assert!(
            scope()
                .with_required_outputs([b"a".to_vec(), b"a".to_vec()])
                .is_err(),
            "duplicates reject"
        );
        assert!(
            scope()
                .with_required_outputs([b"dir".to_vec(), b"dir/file".to_vec()])
                .is_err(),
            "file/directory prefix collisions reject"
        );
    }

    #[test]
    fn required_output_completion_is_linear_against_sealed_custody() {
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("source"),
            PathBuf::from("build"),
            None,
            None,
        )
        .with_required_outputs([b"artifact.txt".to_vec()])
        .expect("declare the roster");

        let diagnostics = scope
            .verify_required_outputs(None, "build")
            .expect_err("no staged custody means no completion");
        assert!(diagnostics[0].to_string().contains("staged-output custody"));

        let tree = build_output::from_entries(&[build_output::OutputTreeEntry::regular_file(
            b"artifact.txt",
            b"done",
            false,
        )])
        .expect("sealed output tree");
        scope
            .verify_required_outputs(Some(&tree), "build")
            .expect("the sealed file completes its required output");

        let diagnostics = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("source"),
            PathBuf::from("build"),
            None,
            None,
        )
        .with_required_outputs([b"omitted.txt".to_vec()])
        .expect("declare the roster")
        .verify_required_outputs(Some(&tree), "build")
        .expect_err("an omitted required member cannot publish");
        assert!(diagnostics[0].to_string().contains("omitted"));

        let dirs_only = build_output::from_entries(&[build_output::OutputTreeEntry::directory(
            b"artifact.txt",
        )])
        .expect("dir tree");
        let diagnostics = scope
            .verify_required_outputs(Some(&dirs_only), "build")
            .expect_err("a directory cannot complete a required file");
        assert!(
            diagnostics[0]
                .to_string()
                .contains("not a sealed regular file")
        );
    }
}
