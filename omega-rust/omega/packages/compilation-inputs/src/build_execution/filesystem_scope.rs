//! Scoped Source/Output authority, staging custody, and sponsored host access.

pub mod preparation;
pub mod runtime;

use super::observations::{
    BuildActivation, BuildCapturedSourceInventory, BuildNamedInputInventory,
};
use crate::BuildCanonicalSourceMetadataIdentity;
use build_output::{
    BuildStagedOutputEntryKind, BuildStagedOutputTree, CapturedBuildSourceInput, capture, empty,
};
use build_time_evaluation::{
    BuildMachineCapturedFilesystemRoot, BuildMachineFilesystemAccess,
    BuildMachineFilesystemGrantRoot, BuildMachineFilesystemGrantRootIdentity,
    BuildMachineFilesystemGrants, BuildMachineFilesystemSponsor,
};
use diagnostics::Diagnostic;
use filesystem_authority::FilesystemSponsorEntry;
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
/// Captured roots are exact non-following memory views; physical source grants
/// remain a separate, explicitly authorized host protocol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMachineFilesystemScope {
    source_root: PathBuf,
    canonical_source_metadata: Option<filesystem_authority::CanonicalFilesystemMetadataIndex>,
    canonical_source_metadata_required: bool,
    build_dir: PathBuf,
    /// The write root's overlap key as admission resolved it. Establishment
    /// re-resolves the same spelling and compares against this baseline: an
    /// ancestor swapped to a host alias after admission changes the
    /// resolution, so a key computed at establishment would admit the
    /// redirect self-consistently.
    admitted_build_dir_key: PathBuf,
    sponsor: Option<BuildMachineFilesystemSponsor>,
    root_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>,
    root_role: Option<crate::BuildDeclarationKind>,
    build_execution_profile: Option<target::TargetProfile>,
    captured_source_input: Option<CapturedBuildSourceInput>,
    captured_source_view: Option<source::SourceSnapshot>,
    required_outputs: BTreeSet<Vec<u8>>,
    named_inputs: Vec<NamedBuildInput>,
}

/// One slot retains both its immutable bytes and its activation-local grant.
/// Source and input roots share the same read protocol, never the same namespace.
#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedBuildInput {
    name: Vec<u8>,
    input: CapturedBuildSourceInput,
    snapshot: source::SourceSnapshot,
    root: BuildMachineFilesystemGrantRootIdentity,
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

/// A fence-side root resolved as far as the host currently allows: the full
/// canonical spelling when the path exists, otherwise the admission key's
/// longest-existing-prefix resolution for a not-yet-materialized tail.
fn resolved_write_fence(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| overlap_key(path))
}

impl BuildMachineFilesystemScope {
    /// Establish the physical write fence before granting captured or host access.
    pub(crate) fn establish_filesystem_access(
        &self,
    ) -> Result<BuildMachineFilesystemAccess, Vec<Diagnostic>> {
        self.ensure_write_roots()?;
        self.ensure_canonical_source_metadata()?;
        Ok(self.filesystem_access())
    }

    pub(crate) fn for_root(
        root_path: &Path,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
    ) -> Self {
        let source_root = root_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let admitted_build_dir_key = overlap_key(&build_dir);
        Self {
            source_root,
            canonical_source_metadata: None,
            canonical_source_metadata_required: false,
            admitted_build_dir_key,
            build_dir,
            sponsor,
            root_package_identity: None,
            root_role: None,
            build_execution_profile: None,
            captured_source_input: None,
            captured_source_view: None,
            required_outputs: BTreeSet::new(),
            named_inputs: Vec::new(),
        }
    }

    #[doc(hidden)]
    pub(crate) fn for_package_root(
        source_root: PathBuf,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
        metadata: Option<filesystem_authority::CanonicalFilesystemMetadataIndex>,
    ) -> Self {
        let admitted_build_dir_key = overlap_key(&build_dir);
        Self {
            source_root,
            canonical_source_metadata: metadata,
            canonical_source_metadata_required: true,
            admitted_build_dir_key,
            build_dir,
            sponsor,
            root_package_identity: None,
            root_role: None,
            build_execution_profile: None,
            captured_source_input: None,
            captured_source_view: None,
            required_outputs: BTreeSet::new(),
            named_inputs: Vec::new(),
        }
    }

    /// Bind the root package occurrence and authored declaration role whose
    /// validated inputs produced this scope.
    pub(crate) fn with_package_activation(
        mut self,
        root_package_identity: semantic_vocabulary::PackageKeyIdentity,
        root_role: crate::BuildDeclarationKind,
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
    pub(crate) fn with_execution_profile(
        mut self,
        build_execution_profile: Option<target::TargetProfile>,
    ) -> Self {
        self.build_execution_profile = build_execution_profile;
        self
    }

    /// Bind this build occurrence's source reads to one compiler-captured
    /// immutable input inventory.
    ///
    /// Its canonical metadata must match the binding's validated index.
    pub(crate) fn with_captured_source_input(
        mut self,
        input: CapturedBuildSourceInput,
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
        Ok(self.bind_captured_source_input(input))
    }

    /// Keep full package provenance while narrowing the build's read grant.
    /// The complete capture must match resolver custody, and the selected
    /// entries must match its exact bytes, not just its file lengths.
    fn with_scoped_package_source_input(
        self,
        input: CapturedBuildSourceInput,
        complete: &CapturedBuildSourceInput,
    ) -> Result<Self, Vec<Diagnostic>> {
        if self.canonical_source_metadata.as_ref() != Some(complete.canonical_source_metadata())
            || !input.is_subset_of(complete)
        {
            return Err(vec![Diagnostic::error(
                "scoped build source input does not match the validated complete package capture",
            )]);
        }
        Ok(self.bind_captured_source_input(input))
    }

    fn bind_captured_source_input(mut self, input: CapturedBuildSourceInput) -> Self {
        self.captured_source_view = Some(crate::source_snapshot::captured_source_view(&input));
        self.captured_source_input = Some(input);
        self
    }

    pub(crate) fn with_named_inputs(
        mut self,
        inputs: &BTreeMap<Vec<u8>, CapturedBuildSourceInput>,
    ) -> Self {
        if inputs.is_empty() {
            return self;
        }
        self.named_inputs = inputs
            .iter()
            .enumerate()
            .map(|(index, (name, input))| NamedBuildInput {
                name: name.clone(),
                input: input.clone(),
                snapshot: crate::source_snapshot::captured_source_view(input),
                root: BuildMachineFilesystemGrantRootIdentity::new(index as u32 + 3)
                    .expect("bounded slot ordinal follows the source and output roots"),
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
    pub(crate) fn with_required_outputs(
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
            if !filesystem_authority::canonical_filesystem_metadata_path_is_canonical(&name, false)
            {
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
    /// members and admitted execution profile. The Build observes no realized
    /// target.
    pub fn activation(&self) -> BuildActivation {
        BuildActivation {
            root_package_identity: self.root_package_identity,
            root_role: self.root_role,
            build_execution_profile: self.build_execution_profile,
        }
    }

    /// The declared sealed outputs this occurrence must settle before its
    /// result may publish, in canonical order.
    pub fn required_outputs(&self) -> &BTreeSet<Vec<u8>> {
        &self.required_outputs
    }

    pub fn captured_source_inventory(&self) -> Option<BuildCapturedSourceInventory> {
        self.captured_source_input.as_ref().map(captured_inventory)
    }

    pub(crate) fn filesystem_access(&self) -> BuildMachineFilesystemAccess {
        let mut read_roots = Vec::new();
        let mut captured_roots = Vec::new();
        if let Some(snapshot) = &self.captured_source_view {
            captured_roots.push(BuildMachineCapturedFilesystemRoot {
                identity: BUILD_SOURCE_ROOT_IDENTITY,
                snapshot: snapshot.clone(),
            });
        } else {
            let mut source_root = BuildMachineFilesystemGrantRoot::new(
                BUILD_SOURCE_ROOT_IDENTITY,
                self.source_root.clone(),
            );
            if let Some(metadata) = &self.canonical_source_metadata {
                source_root = source_root.with_canonical_metadata(metadata.clone());
            }
            read_roots.push(source_root);
        }
        captured_roots.extend(self.named_inputs.iter().map(|input| {
            BuildMachineCapturedFilesystemRoot {
                identity: input.root,
                snapshot: input.snapshot.clone(),
            }
        }));
        let grants = BuildMachineFilesystemGrants {
            read_roots,
            write_roots: vec![BuildMachineFilesystemGrantRoot::new(
                BUILD_OUTPUT_ROOT_IDENTITY,
                self.build_dir.clone(),
            )],
        };
        if !captured_roots.is_empty() {
            return BuildMachineFilesystemAccess::CapturedScoped {
                inputs: captured_roots,
                grants,
                sponsor: self.sponsor.clone(),
            };
        }
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
        let BuildMachineFilesystemAccess::CapturedScoped {
            sponsor: Some(sponsor),
            grants,
            ..
        } = access
        else {
            return false;
        };
        self.captured_source_input.is_some()
            && grants.read_roots.is_empty()
            && sponsor.owns_private_staging_root(&self.build_dir)
            && access == &self.filesystem_access()
    }

    pub(crate) fn ensure_write_roots(&self) -> Result<(), Vec<Diagnostic>> {
        self.admit_write_roots()?;
        self.verify_materialized_write_root()
    }

    /// The write root is spelled at admission but written through by path
    /// for the whole occurrence, so an ancestor swapped for a symlink after
    /// admission would redirect the granted writes. Re-resolve the
    /// now-materialized root through the host's canonical spelling and
    /// re-run the same overlap assertions on resolved roots; an alias
    /// planted in the window surfaces here, before first use. Refusal
    /// deliberately leaves the resolved location untouched — a directory
    /// reached through a racing alias is not this occurrence's to delete.
    fn verify_materialized_write_root(&self) -> Result<(), Vec<Diagnostic>> {
        let write_root = std::fs::canonicalize(&self.build_dir).map_err(|error| {
            vec![Diagnostic::error(format!(
                "failed to re-resolve the admitted build write root `{}`: {error}",
                self.build_dir.display()
            ))]
        })?;
        if resolved_write_fence(&self.source_root).starts_with(&write_root) {
            return Err(vec![Diagnostic::error(format!(
                "admitted build write root `{}` resolved onto the source root `{}` after admission — a racing alias replaced an ancestor",
                self.build_dir.display(),
                self.source_root.display()
            ))]);
        }
        Ok(())
    }

    fn admit_write_roots(&self) -> Result<(), Vec<Diagnostic>> {
        // The write root must not name the source root's directory or one
        // of its ancestors — through any spelling — or Source's read-only
        // contract would grant writes over every captured member. The
        // default layout nests the write root inside the read root, which
        // stays admitted: writes reach only the build directory itself. The
        // baseline is admission's recorded key, not a fresh resolution — an
        // ancestor aliased after admission must still compare against it.
        let admitted_build_dir_key = &self.admitted_build_dir_key;
        if overlap_key(&self.source_root).starts_with(admitted_build_dir_key) {
            return Err(vec![Diagnostic::error(format!(
                "build write root `{}` must not cover the source root `{}`",
                self.build_dir.display(),
                self.source_root.display()
            ))]);
        }
        // Compare against admission's recorded key before any mutation: an
        // ancestor aliased between them must refuse here, not after
        // `create_dir_all` has already followed the alias. The post-creation
        // check below keeps covering the narrower window inside establishment.
        if overlap_key(&self.build_dir) != *admitted_build_dir_key {
            return Err(vec![Diagnostic::error(format!(
                "build write root `{}` resolves to a different directory than admission checked",
                self.build_dir.display()
            ))]);
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
                    return self.ensure_established_write_root(admitted_build_dir_key);
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
            if let Err(diagnostics) = self.ensure_established_write_root(admitted_build_dir_key) {
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
        self.ensure_established_write_root(admitted_build_dir_key)
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
        error: filesystem_authority::FilesystemSponsorError,
    ) -> Vec<Diagnostic> {
        vec![Diagnostic::error(format!(
            "build machine staging sponsor rejected `{}`: {error}",
            self.build_dir.display()
        ))]
    }

    pub(crate) fn staged_output_tree(
        &self,
        filesystem_reachable: bool,
    ) -> Result<Option<BuildStagedOutputTree>, Vec<Diagnostic>> {
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
