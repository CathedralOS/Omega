//! Scoped Source/Output authority, staging custody, and sponsored host access.

pub mod preparation;

use crate::{
    BuildCanonicalSourceMetadataIdentity,
    evidence::observations::{
        BuildActivation, BuildCapturedSourceInventory, BuildNamedInputInventory,
    },
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
    root_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    root_role: Option<package_compilation::BuildDeclarationKind>,
    build_execution_profile: Option<target::TargetProfile>,
    captured_source_input: Option<CapturedBuildSourceInput>,
    snapshot_dir: Option<PathBuf>,
    required_outputs: BTreeSet<Vec<u8>>,
    named_inputs: Vec<NamedBuildInput>,
}

/// One slot retains both its immutable bytes and its activation-local grant.
/// Source and input roots share the same read protocol, never the same namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedBuildInput {
    name: Vec<u8>,
    input: CapturedBuildSourceInput,
    root: BuildMachineFilesystemGrantRootIdentity,
    snapshot_dir: PathBuf,
}

/// Maximum declared required sealed outputs for one build occurrence.
const REQUIRED_BUILD_OUTPUT_LIMIT: usize = 4_096;

/// One spelling-independent key for root-overlap decisions. Two roots that
/// collide on the host must share a key whichever spelling bound them: `..`
/// and `.` components fold lexically, a relative spelling absolutizes
/// against the current directory, and the longest existing prefix resolves
/// through the host's canonical spelling so a symlinked ancestor names the
/// same directory. A `..` that escapes the filesystem root keeps its
/// original spelling — it cannot name a granted root anyway.
fn overlap_key(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        match std::env::current_dir() {
            Ok(current_dir) => current_dir.join(path),
            Err(_) => path.to_path_buf(),
        }
    };
    let mut folded = PathBuf::new();
    for component in absolute.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                if !folded.pop() {
                    return absolute;
                }
            }
            other => folded.push(other.as_os_str()),
        }
    }
    // A not-yet-created tail cannot resolve; the longest existing prefix
    // still does, so a symlink inside an ancestor cannot alias past the
    // fence. `symlink_metadata` keeps a dangling symlink "existing" so it
    // still resolves through its spelling rather than falling past it.
    let mut base = folded.as_path();
    let mut tail: Vec<std::ffi::OsString> = Vec::new();
    while std::fs::symlink_metadata(base).is_err() {
        match base
            .file_name()
            .map(|name| (name.to_os_string(), base.parent()))
        {
            Some((name, Some(parent))) => {
                tail.push(name);
                base = parent;
            }
            _ => break,
        }
    }
    let mut key = std::fs::canonicalize(base).unwrap_or_else(|_| base.to_path_buf());
    for name in tail.iter().rev() {
        key.push(name);
    }
    key
}

/// Whether two roots name overlapping host directories under any spelling.
fn roots_overlap(left: &Path, right: &Path) -> bool {
    let left = overlap_key(left);
    let right = overlap_key(right);
    left == right || left.starts_with(&right) || right.starts_with(&left)
}

/// Release handle for one occurrence's private captured-source backing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedSnapshotRelease {
    snapshot_dirs: Vec<PathBuf>,
}

impl CapturedSnapshotRelease {
    /// Discard the private materialization if this occurrence bound one.
    /// An already-absent backing is not an error, so the release may run on
    /// the settlement path and again on the occurrence's final exit.
    pub(crate) fn release(&self) {
        for snapshot_dir in &self.snapshot_dirs {
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
            root_package_identity: None,
            root_role: None,
            build_execution_profile: None,
            captured_source_input: None,
            snapshot_dir: None,
            required_outputs: BTreeSet::new(),
            named_inputs: Vec::new(),
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
            root_package_identity: None,
            root_role: None,
            build_execution_profile: None,
            captured_source_input: None,
            snapshot_dir: None,
            required_outputs: BTreeSet::new(),
            named_inputs: Vec::new(),
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
        if roots_overlap(&snapshot_dir, &self.build_dir) {
            return Err(vec![Diagnostic::error(format!(
                "captured source snapshot directory `{}` must not overlap the build write root `{}`",
                snapshot_dir.display(),
                self.build_dir.display()
            ))]);
        }
        if roots_overlap(&snapshot_dir, &self.source_root) {
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

    pub(crate) fn with_named_inputs(
        mut self,
        inputs: &BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
    ) -> Self {
        if inputs.is_empty() {
            return self;
        }
        let source_backing = self
            .snapshot_dir
            .as_ref()
            .expect("named inputs accompany captured source");
        self.named_inputs = inputs
            .iter()
            .enumerate()
            .map(|(index, (name, input))| NamedBuildInput {
                name: name.clone(),
                input: input.clone(),
                root: BuildMachineFilesystemGrantRootIdentity::new(index as u32 + 3)
                    .expect("bounded slot ordinal follows the source and output roots"),
                snapshot_dir: source_backing.with_extension(format!("input-{index}")),
            })
            .collect();
        self
    }

    pub(crate) fn named_input_facet(&self) -> build_time_evaluation::BuildTimeValue {
        use build_time_evaluation::BuildTimeValue;
        BuildTimeValue::Struct {
            type_name: "$OmegaBuildInputSlots".to_owned(),
            fields: self
                .named_inputs
                .iter()
                .map(|input| {
                    (
                        String::from_utf8(input.name.clone()).expect("validated UTF-8 slot"),
                        BuildTimeValue::Struct {
                            type_name: "$OmegaBuildSourceRoot".to_owned(),
                            fields: vec![(
                                "root".to_owned(),
                                BuildTimeValue::Int(i64::from(input.root.get())),
                            )],
                        },
                    )
                })
                .collect(),
        }
    }

    pub(crate) fn named_input_inventory(
        &self,
        root: BuildMachineFilesystemGrantRootIdentity,
    ) -> Option<BuildCapturedSourceInventory> {
        self.named_inputs
            .iter()
            .find(|input| input.root == root)
            .map(|input| captured_inventory(&input.input))
    }

    pub(crate) fn named_input_inventories(&self) -> Vec<BuildNamedInputInventory> {
        self.named_inputs
            .iter()
            .map(|input| BuildNamedInputInventory {
                name: input.name.clone(),
                root_identity: input.root.get(),
                inventory: captured_inventory(&input.input),
            })
            .collect()
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
        self.captured_source_input.as_ref().map(captured_inventory)
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
        let mut read_roots = vec![source_root];
        read_roots.extend(self.named_inputs.iter().map(|input| {
            BuildMachineFilesystemGrantRoot::new(input.root, input.snapshot_dir.clone())
                .with_canonical_metadata(input.input.canonical_source_metadata().clone())
        }));
        let grants = BuildMachineFilesystemGrants {
            read_roots,
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

    /// The benign build exemption follows exact issued custody, not root names
    /// or sponsorship alone. Generic sponsors may point at existing host data.
    /// Here every read uses captured membership, and the only write root belongs
    /// to a fresh private session whose shared owner also handles cleanup.
    pub(crate) fn is_private_snapshot_execution(
        &self,
        access: &BuildMachineFilesystemAccess,
    ) -> bool {
        let BuildMachineFilesystemAccess::RealScopedSponsored { sponsor, .. } = access else {
            return false;
        };
        self.captured_source_input.is_some()
            && self.snapshot_dir.is_some()
            && sponsor.owns_private_staging_root(&self.build_dir)
            && access == &self.filesystem_access()
    }

    pub(crate) fn ensure_write_roots(&self) -> Result<(), Vec<Diagnostic>> {
        // The write root must not name the source root's directory or one
        // of its ancestors — through any spelling — or Source's read-only
        // contract would grant writes over every captured member. The
        // default layout nests the write root inside the read root, which
        // stays admitted: writes reach only the build directory itself.
        let admitted_build_dir_key = overlap_key(&self.build_dir);
        if overlap_key(&self.source_root).starts_with(&admitted_build_dir_key) {
            return Err(vec![Diagnostic::error(format!(
                "build write root `{}` must not cover the source root `{}`",
                self.build_dir.display(),
                self.source_root.display()
            ))]);
        }
        // Each named input's read root derives from the captured source
        // backing as `<snapshot>.input-N` — a sibling spelling the source
        // fence never sees, so the write root is rechecked here after the
        // inventory is bound.
        for input in &self.named_inputs {
            if roots_overlap(&input.snapshot_dir, &self.build_dir) {
                return Err(vec![Diagnostic::error(format!(
                    "build write root `{}` must not overlap named input snapshot directory `{}`",
                    self.build_dir.display(),
                    input.snapshot_dir.display()
                ))]);
            }
        }
        if let Some(sponsor) = &self.sponsor {
            let path = sponsor
                .bind_path(&self.build_dir)
                .map_err(|error| self.sponsor_diagnostic(error))?;
            match sponsor
                .entry(&path)
                .map_err(|error| self.sponsor_diagnostic(error))?
            {
                Some(FilesystemSponsorEntry::Directory) => {
                    return self.ensure_established_write_root(&admitted_build_dir_key);
                }
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
            if let Err(diagnostics) = self.ensure_established_write_root(&admitted_build_dir_key) {
                let _ = std::fs::remove_dir(&self.build_dir);
                return Err(diagnostics);
            }
            return Ok(());
        }
        std::fs::create_dir_all(&self.build_dir).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to create build machine filesystem write root `{}`: {error}",
                self.build_dir.display()
            ))]
        })?;
        self.ensure_established_write_root(&admitted_build_dir_key)
    }

    /// Re-check the write root now that it exists. The overlap fences above
    /// ran on the key admission computed, but a host alias planted between
    /// that check and the first write is invisible to them — `create_dir_all`
    /// follows a symlinked root, so establishment must confirm the spelling
    /// still resolves to the admitted directory rather than a substituted
    /// host path.
    fn ensure_established_write_root(
        &self,
        admitted_build_dir_key: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
        match std::fs::symlink_metadata(&self.build_dir) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(vec![Diagnostic::error(format!(
                    "build write root `{}` is a symbolic link, not the directory admission checked",
                    self.build_dir.display()
                ))]);
            }
            _ => {
                return Err(vec![Diagnostic::error(format!(
                    "build write root `{}` is not a real directory",
                    self.build_dir.display()
                ))]);
            }
        }
        if overlap_key(&self.build_dir) != *admitted_build_dir_key {
            return Err(vec![Diagnostic::error(format!(
                "build write root `{}` resolves to a different directory than admission checked",
                self.build_dir.display()
            ))]);
        }
        Ok(())
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
        let snapshots = self
            .captured_source_input
            .as_ref()
            .zip(self.snapshot_dir.as_deref())
            .into_iter()
            .chain(
                self.named_inputs
                    .iter()
                    .map(|input| (&input.input, input.snapshot_dir.as_path())),
            );
        for (input, directory) in snapshots {
            if let Err(diagnostics) = Self::materialize_snapshot(input, directory) {
                self.captured_snapshot_release().release();
                return Err(diagnostics);
            }
        }
        Ok(())
    }

    fn materialize_snapshot(
        input: &CapturedBuildSourceInput,
        snapshot_dir: &Path,
    ) -> Result<(), Vec<Diagnostic>> {
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
            snapshot_dirs: self
                .snapshot_dir
                .iter()
                .cloned()
                .chain(
                    self.named_inputs
                        .iter()
                        .map(|input| input.snapshot_dir.clone()),
                )
                .collect(),
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

fn captured_inventory(input: &CapturedBuildSourceInput) -> BuildCapturedSourceInventory {
    BuildCapturedSourceInventory {
        entry_count: input.entry_count(),
        file_bytes: input.file_bytes(),
        source_metadata_identity: BuildCanonicalSourceMetadataIdentity::new(
            input.canonical_source_metadata().policy_version(),
            *input
                .canonical_source_metadata()
                .source_content_commitment(),
        ),
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
    fn benign_snapshot_execution_requires_exact_private_staging_custody() {
        let fixture = temporary_staging_root("benign-custody");
        fs::create_dir(&fixture).unwrap();
        let sponsor = FilesystemSponsor::create_private(fixture.join("private")).unwrap();
        let private_root = sponsor.session_root().unwrap();
        let source = fixture.join("source");
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            private_root.join("output"),
            Some(sponsor.clone()),
            Some(captured_input().canonical_source_metadata().clone()),
        )
        .with_captured_source_input(captured_input(), fixture.join("captured"))
        .unwrap()
        .with_named_inputs(&std::collections::BTreeMap::from([(
            b"template".to_vec(),
            captured_input(),
        )]));
        let access = scope.filesystem_access();
        assert!(scope.is_private_snapshot_execution(&access));
        let BuildMachineFilesystemAccess::RealScopedSponsored { grants, .. } = &access else {
            panic!("captured private scope must be sponsored")
        };
        let mut broader = grants.clone();
        broader
            .read_roots
            .push(build_time_evaluation::BuildMachineFilesystemGrantRoot::new(
                build_time_evaluation::BuildMachineFilesystemGrantRootIdentity::new(9).unwrap(),
                fixture.join("live"),
            ));
        assert!(!scope.is_private_snapshot_execution(
            &BuildMachineFilesystemAccess::RealScopedSponsored {
                grants: broader,
                sponsor: sponsor.clone(),
            }
        ));
        let mut writable_input = grants.clone();
        writable_input
            .write_roots
            .push(grants.read_roots[1].clone());
        assert!(!scope.is_private_snapshot_execution(
            &BuildMachineFilesystemAccess::RealScopedSponsored {
                grants: writable_input,
                sponsor: sponsor.clone(),
            }
        ));
        let mut substituted = grants.clone();
        substituted.read_roots[0] = build_time_evaluation::BuildMachineFilesystemGrantRoot::new(
            super::BUILD_SOURCE_ROOT_IDENTITY,
            fixture.join("different-backing"),
        )
        .with_canonical_metadata(captured_input().canonical_source_metadata().clone());
        assert!(!scope.is_private_snapshot_execution(
            &BuildMachineFilesystemAccess::RealScopedSponsored {
                grants: substituted,
                sponsor: sponsor.clone(),
            }
        ));
        assert!(!scope.is_private_snapshot_execution(&BuildMachineFilesystemAccess::RealUnscoped));
        assert!(
            !scope.is_private_snapshot_execution(&BuildMachineFilesystemAccess::RealScoped(
                grants.clone()
            ))
        );
        let mut live_source = scope.clone();
        live_source.captured_source_input = None;
        live_source.snapshot_dir = None;
        assert!(!live_source.is_private_snapshot_execution(&live_source.filesystem_access()));
        let mut supplied_directory = scope.clone();
        supplied_directory.sponsor = Some(FilesystemSponsor::new(&private_root).unwrap());
        assert!(
            !supplied_directory
                .is_private_snapshot_execution(&supplied_directory.filesystem_access())
        );
        sponsor.dispose_private_staging().unwrap();
        assert!(!scope.is_private_snapshot_execution(&access));
        fs::remove_dir_all(fixture).unwrap();
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
    fn captured_source_scope_rejects_host_alias_spellings_of_the_roots() {
        let fixture = temporary_staging_root("snapshot-alias");
        let source = fixture.join("source");
        let build = fixture.join("build");
        fs::create_dir_all(&source).expect("create source dir");
        fs::create_dir_all(&build).expect("create build dir");

        // A `..` spelling of the write root names the same directory.
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            build.clone(),
            None,
            None,
        );
        let diagnostics = scope
            .with_captured_source_input(
                captured_input(),
                fixture.join("other").join("..").join("build"),
            )
            .expect_err("a parenthesized spelling of the write root still aliases it");
        assert!(diagnostics[0].to_string().contains("build write root"));

        // A `..` spelling of the source root names the same directory.
        let scope = BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            build.clone(),
            None,
            None,
        );
        let diagnostics = scope
            .with_captured_source_input(captured_input(), source.join("nested").join(".."))
            .expect_err("a parenthesized spelling of the source root still aliases it");
        assert!(diagnostics[0].to_string().contains("source root"));

        // A relative spelling of an absolute read root names the same
        // directory once absolutized.
        let scope = BuildMachineFilesystemScope::for_package_root(
            PathBuf::from("aliased-source"),
            PathBuf::from("aliased-output"),
            None,
            None,
        );
        let snapshot = std::env::current_dir()
            .expect("resolve test cwd")
            .join("aliased-source");
        let diagnostics = scope
            .with_captured_source_input(captured_input(), snapshot)
            .expect_err("a relative read root must not hide an absolute alias");
        assert!(diagnostics[0].to_string().contains("source root"));

        // A symlinked ancestor names the same directory.
        #[cfg(unix)]
        {
            let link = fixture.join("link");
            std::os::unix::fs::symlink(&build, &link).expect("symlink the build root");
            let scope = BuildMachineFilesystemScope::for_package_root(
                source.clone(),
                build.clone(),
                None,
                None,
            );
            let diagnostics = scope
                .with_captured_source_input(captured_input(), link.join("snapshot"))
                .expect_err("a symlinked spelling of the write root still aliases it");
            assert!(diagnostics[0].to_string().contains("build write root"));
            fs::remove_file(&link).expect("remove link");
        }

        fs::remove_dir_all(&fixture).expect("remove fixture");
    }

    #[test]
    fn build_write_root_rejects_spelling_that_covers_the_source_root() {
        let fixture = temporary_staging_root("covering-alias");
        let source = fixture.join("source");
        fs::create_dir_all(&source).expect("create source dir");

        // The write root covering the read root — through either a plain or
        // a `..` spelling — would make every read-only source writable.
        for build_dir in [fixture.clone(), fixture.join("nested").join("..")] {
            let diagnostics = BuildMachineFilesystemScope::for_package_root(
                source.clone(),
                build_dir.clone(),
                None,
                None,
            )
            .ensure_write_roots()
            .expect_err("a write root covering the source root collides with it");
            assert!(
                diagnostics[0].to_string().contains("source root"),
                "unexpected diagnostic: {diagnostics:?}"
            );
            assert!(!fixture.join("nested").exists());
        }

        // The default layout — the write root nested inside the read root —
        // remains admitted.
        BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            source.join("build"),
            None,
            None,
        )
        .ensure_write_roots()
        .expect("a write root nested inside the source root stays admitted");

        fs::remove_dir_all(&fixture).expect("remove fixture");
    }

    #[test]
    fn build_write_root_rejects_alias_of_a_named_input_snapshot() {
        let fixture = temporary_staging_root("named-input-alias");
        let source = fixture.join("source");
        let snapshot = fixture.join("captured");
        fs::create_dir_all(&source).expect("create source dir");

        // A named input's read root is derived as `<snapshot>.input-N`: a
        // sibling spelling of the captured backing that neither the source
        // root fence nor the captured-input fence sees.
        let named_input_root = snapshot.with_extension("input-0");
        for build_dir in [named_input_root.clone(), named_input_root.join("nested")] {
            let diagnostics = BuildMachineFilesystemScope::for_package_root(
                source.clone(),
                build_dir.clone(),
                None,
                None,
            )
            .with_captured_source_input(captured_input(), snapshot.clone())
            .expect("snapshot backing does not overlap the source root")
            .with_named_inputs(&std::collections::BTreeMap::from([(
                b"template".to_vec(),
                captured_input(),
            )]))
            .ensure_write_roots()
            .expect_err("a write root overlapping a named input's read root collides with it");
            assert!(
                diagnostics[0].to_string().contains("named input"),
                "unexpected diagnostic: {diagnostics:?}"
            );
            assert!(
                !named_input_root.exists() && !build_dir.exists(),
                "the rejected write root must not be created"
            );
        }

        // A write root unrelated to any named input's derived root remains
        // admitted.
        BuildMachineFilesystemScope::for_package_root(
            source.clone(),
            fixture.join("output"),
            None,
            None,
        )
        .with_captured_source_input(captured_input(), snapshot.clone())
        .expect("snapshot backing does not overlap the source root")
        .with_named_inputs(&std::collections::BTreeMap::from([(
            b"template".to_vec(),
            captured_input(),
        )]))
        .ensure_write_roots()
        .expect("a disjoint write root stays admitted");

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

    #[cfg(unix)]
    #[test]
    fn write_root_establishment_rejects_a_host_alias() {
        let session_root = temporary_staging_root("write-root-alias");
        fs::create_dir(&session_root).expect("create session root");
        let redirect_target = session_root.join("redirected-elsewhere");
        fs::create_dir(&redirect_target).expect("create redirect target");
        let build_dir = session_root.join("build");
        std::os::unix::fs::symlink(&redirect_target, &build_dir)
            .expect("plant the host alias at the write root");

        let diagnostics = BuildMachineFilesystemScope::for_root(
            &session_root.join("source/main.omg"),
            build_dir.clone(),
            None,
        )
        .ensure_write_roots()
        .expect_err("a symlinked write root redirects output writes outside the fenced root");
        assert!(diagnostics[0].to_string().contains("symbolic link"));
        assert_eq!(
            fs::read_link(&build_dir).expect("the host alias is host-owned, not ours to remove"),
            redirect_target
        );

        fs::remove_dir_all(session_root).expect("remove session root");
    }

    #[cfg(unix)]
    #[test]
    fn write_root_establishment_rejects_resolution_drift() {
        let session_root = temporary_staging_root("write-root-drift");
        fs::create_dir(&session_root).expect("create session root");
        let real_dir = session_root.join("real-build");
        fs::create_dir(&real_dir).expect("create the admitted directory");
        let spelling = session_root.join("build");

        // An alias resolving to the admitted directory is accepted at
        // admission by design (`overlap_key` canonicalizes existing
        // prefixes); a spelling that resolves differently once the write
        // root exists is not the directory admission checked.
        std::os::unix::fs::symlink(&real_dir, &spelling).expect("plant the host alias");
        let diagnostics = BuildMachineFilesystemScope::for_root(
            &session_root.join("source/main.omg"),
            spelling.clone(),
            None,
        )
        .ensure_write_roots()
        .expect_err("a write root spelled through a link is not the directory admission checked");
        assert!(diagnostics[0].to_string().contains("symbolic link"));

        fs::remove_dir_all(session_root).expect("remove session root");
    }

    #[cfg(unix)]
    #[test]
    fn captured_source_snapshot_rejects_a_host_alias_at_its_backing() {
        let session_root = temporary_staging_root("snapshot-backing-alias");
        fs::create_dir(&session_root).expect("create session root");
        let backing = session_root.join("captured-source");
        let redirect_target = session_root.join("redirected-elsewhere");
        fs::create_dir(&redirect_target).expect("create redirect target");
        std::os::unix::fs::symlink(&redirect_target, &backing)
            .expect("plant the host alias at the snapshot backing");

        let diagnostics = BuildMachineFilesystemScope::materialize_snapshot(
            &captured_input(),
            &backing,
        )
        .expect_err(
            "a symlinked snapshot backing redirects captured-source writes outside its custody",
        );
        assert!(
            diagnostics[0]
                .to_string()
                .contains("not a concrete directory")
        );
        assert_eq!(
            fs::read_link(&backing).expect("the host alias is host-owned, not ours to remove"),
            redirect_target
        );

        // An ordinary non-directory resident at the backing names the same
        // rejection: the snapshot materializes only into a fresh directory
        // it owns, never over host content.
        fs::remove_file(&backing).expect("remove the planted alias");
        fs::write(&backing, b"occupied").expect("plant a regular file at the snapshot backing");
        let diagnostics =
            BuildMachineFilesystemScope::materialize_snapshot(&captured_input(), &backing)
                .expect_err("a regular file at the snapshot backing is not its directory custody");
        assert!(
            diagnostics[0]
                .to_string()
                .contains("not a concrete directory")
        );

        fs::remove_dir_all(session_root).expect("remove session root");
    }
}
