/*
We own the host and captured filesystem authority granted to an admitted `build.omg` machine (`BuildMachineFilesystemScope`).

Every build activation operates under explicit grant roots:
1. `BUILD_SOURCE_ROOT_IDENTITY` (`1`) is the read-only `Source` root, backed either by an immutable compiler-captured `CapturedBuildSourceInput` (`SourceSnapshot`) or by an explicitly admitted `LiveBuildSource` directory capability.
2. `BUILD_OUTPUT_ROOT_IDENTITY` (`2`) is the append-and-seal `Output` staging directory (`build_dir`), governed by a `BuildMachineFilesystemSponsor`.
3. `NamedBuildInput` slots (`3..`) bind additional read-only `CapturedBuildSourceInput` snapshots under `$OmegaBuildInputSlots`.
*/

/*
Why we record `admitted_build_dir_key` at admission and re-verify at establishment:
The write root (`build_dir`) is spelled at admission but written through by path across the build occurrence. If an ancestor directory were swapped for a symlink after admission, `create_dir_all` or subsequent file opens would silently redirect writes outside the staging fence or onto the read-only `Source` root. We compute a spelling-independent `overlap_key` (resolving the longest existing prefix through `canonicalize` while keeping dangling symlinks visible via `symlink_metadata`) at admission, verify that `source_root` does not sit inside `admitted_build_dir_key`, and re-verify both before and after materializing `build_dir` that its resolved key has not drifted.

Why `staged_output_tree` canonicalizes unreachable-filesystem runs to `empty()`:
When the selected build machine cannot reach the filesystem at all, the `Output` namespace is provably empty without needing a physical staging directory. Returning `Some(empty())` ensures sponsored review and ordinary production record the exact same staged-output observation.
*/

/*
Provenance:
- Capability-bound live source directory (`LiveBuildSource`): `src/build_execution/filesystem_scope/live_source.rs`.
- Request-to-scope preparation and private sponsor provisioning: `src/build_execution/filesystem_scope/preparation.rs`.
- Terminal Psi runtime filesystem host (`BuildFilesystemRuntime`): `src/build_execution/filesystem_scope/runtime.rs`.
*/

mod live_source;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildMachineFilesystemScope {
    source_root: PathBuf,                              // Physical source root path.
    live_source: Option<live_source::LiveBuildSource>, // Capability-bound live source directory when uncaptured.
    canonical_source_metadata: Option<filesystem_authority::CanonicalFilesystemMetadataIndex>, // Validated canonical Source metadata index.
    canonical_source_metadata_required: bool, // Whether package/anonymous custody requires canonical metadata.
    build_dir: PathBuf,                       // Physical Output staging root path.
    admitted_build_dir_key: PathBuf,          // Overlap key of `build_dir` recorded at admission.
    sponsor: Option<BuildMachineFilesystemSponsor>, // Staging directory quota and custody sponsor.
    root_package_identity: Option<semantic_vocabulary::PackageKeyIdentity>, // Bound root package identity.
    root_role: Option<crate::BuildDeclarationKind>, // Bound root declaration role (`Library` or `Product`).
    build_execution_profile: Option<target::TargetProfile>, // Admitted execution profile for build-scope evaluation.
    captured_source_input: Option<CapturedBuildSourceInput>, // Immutable captured Source inventory.
    captured_source_view: Option<source::SourceSnapshot>, // Read-only memory snapshot view of `captured_source_input`.
    required_outputs: BTreeSet<Vec<u8>>, // Declared required sealed output paths in canonical order.
    named_inputs: Vec<NamedBuildInput>,  // Assigned named input roots (`3..`).
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NamedBuildInput {
    name: Vec<u8>,                                 // Validated UTF-8 slot name.
    input: CapturedBuildSourceInput,               // Immutable captured input inventory.
    snapshot: source::SourceSnapshot,              // Memory snapshot view for interpreter reads.
    root: BuildMachineFilesystemGrantRootIdentity, // Activation-local grant root identity (`>= 3`).
}

const REQUIRED_BUILD_OUTPUT_LIMIT: usize = 4_096;

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

fn resolved_write_fence(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| overlap_key(path))
}

impl BuildMachineFilesystemScope {
    pub(crate) fn establish_filesystem_access(
        &mut self,
    ) -> Result<BuildMachineFilesystemAccess, Vec<Diagnostic>> {
        self.ensure_write_roots()?;
        self.ensure_canonical_source_metadata()?;
        if self.captured_source_view.is_none() && self.live_source.is_none() {
            self.live_source = Some(
                live_source::LiveBuildSource::bind(&self.source_root).map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "could not bind admitted live Build Source: {error}"
                    ))]
                })?,
            );
        }
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
            live_source: None,
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
    pub(crate) fn for_admitted_root(
        source_root: PathBuf,
        build_dir: PathBuf,
        sponsor: Option<BuildMachineFilesystemSponsor>,
        metadata: Option<filesystem_authority::CanonicalFilesystemMetadataIndex>,
    ) -> Self {
        let admitted_build_dir_key = overlap_key(&build_dir);
        Self {
            source_root,
            live_source: None,
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

    pub(crate) fn with_package_activation(
        mut self,
        root_package_identity: semantic_vocabulary::PackageKeyIdentity,
        root_role: crate::BuildDeclarationKind,
    ) -> Self {
        self.root_package_identity = Some(root_package_identity);
        self.root_role = Some(root_role);
        self
    }

    pub(crate) fn with_execution_profile(
        mut self,
        build_execution_profile: Option<target::TargetProfile>,
    ) -> Self {
        self.build_execution_profile = build_execution_profile;
        self
    }

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
        self.captured_source_view =
            Some(crate::source::source_snapshot::captured_source_view(&input));
        self.captured_source_input = Some(input);
        self.live_source = None;
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
                snapshot: crate::source::source_snapshot::captured_source_view(input),
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

    pub fn activation(&self) -> BuildActivation {
        BuildActivation {
            root_package_identity: self.root_package_identity,
            root_role: self.root_role,
            build_execution_profile: self.build_execution_profile,
        }
    }

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
        let admitted_build_dir_key = &self.admitted_build_dir_key;
        if overlap_key(&self.source_root).starts_with(admitted_build_dir_key) {
            return Err(vec![Diagnostic::error(format!(
                "build write root `{}` must not cover the source root `{}`",
                self.build_dir.display(),
                self.source_root.display()
            ))]);
        }
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
