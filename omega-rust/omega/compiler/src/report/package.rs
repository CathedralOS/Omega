//! macOS `.app` package assembly and its self-verifying receipt.
//!
//! Complete selected macOS GUI output is a bundle, never a flat executable
//! beside it (wiki/spec/build/macos_application.md). One validated authored
//! `builder.application` name supplies the `.app` basename and the inner
//! executable leaf; the retained `builder.identifier` supplies
//! `CFBundleIdentifier` and must match the CodeDirectory identity already
//! bound into the signed executable bytes.
//!
//! The v1 tree for `window-app` is exact:
//!
//! ```text
//!   window-app.app/
//!     Contents/
//!       Info.plist
//!       MacOS/window-app
//! ```
//!
//! plus the requested Psi artifact/companion pair beside the executable when
//! the normalized Build asked for it. The plist uses package type `APPL` with
//! fixed encoding, key order, and escaping — no timestamps or ambient facts —
//! so repeated publication is byte-identical.
//!
//! Assembly stages the whole tree under `.{name}.app.{pid}.tmp` beside the
//! destination, replays every staged byte, validates the exact shape, then
//! replaces any previous package with one rename and replays the installed
//! tree again. Missing, extra, substituted, or partial contents reject, and
//! any failure removes the staged tree and the just-installed package so no
//! partial output survives. The stored receipt independently re-compares the
//! plist identifier, the retained realization identity, and the identifier
//! encoded in the installed executable's CodeDirectory; matching producer
//! assertions alone are insufficient.

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use super::ExecutableContainerDigest;

publication_digest!(PackageComponentDigest);
publication_digest!(NativePackageEvidenceDigest);

/// Digest one installed package member's exact bytes under its own domain.
/// Crate-visible for report-custody tests minting honest member digests.
pub(crate) fn package_component_digest(bytes: &[u8]) -> PackageComponentDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.published-package-component.sha256.v1\0");
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    PackageComponentDigest::from_digest(digest.finalize().into())
}

/// The package-level commitment: package root, authored name, authored
/// identifier, the inner executable's v1 container identity, and every
/// installed member's package-relative path, byte count, and component digest
/// in their fixed recorded order. Reordering or editing any member stops the
/// receipt from replaying.
fn native_package_evidence_digest(
    package_root: &Path,
    application_name: &str,
    application_identifier: &build_evaluation::ApplicationIdentifier,
    executable_container_digest: ExecutableContainerDigest,
    components: &[PackagePublicationComponent],
) -> NativePackageEvidenceDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-package-evidence.sha256.v1\0");
    let root = package_root.as_os_str().as_encoded_bytes();
    digest.update((root.len() as u64).to_le_bytes());
    digest.update(root);
    digest.update((application_name.len() as u64).to_le_bytes());
    digest.update(application_name.as_bytes());
    let identifier = application_identifier.as_str().as_bytes();
    digest.update((identifier.len() as u64).to_le_bytes());
    digest.update(identifier);
    digest.update(executable_container_digest.as_bytes());
    digest.update((components.len() as u64).to_le_bytes());
    for component in components {
        let relative = component.relative_path.as_os_str().as_encoded_bytes();
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative);
        digest.update((component.byte_count as u64).to_le_bytes());
        digest.update(component.digest.as_bytes());
    }
    NativePackageEvidenceDigest::from_digest(digest.finalize().into())
}

/// One installed package member, recorded package-relative so the receipt is
/// portable across destinations and invocation working directories. Fields
/// are crate-visible for report-custody tests; the receipt's replayed
/// evidence digest is what makes a forged component detectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePublicationComponent {
    pub(crate) relative_path: PathBuf,
    pub(crate) byte_count: usize,
    pub(crate) digest: PackageComponentDigest,
}

impl PackagePublicationComponent {
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub const fn byte_count(&self) -> usize {
        self.byte_count
    }
}

/// Immutable custody for one compiler-published macOS application package.
///
/// The receipt covers the exact executable/plist/companion bytes and the
/// directory shape they were installed under. It is compiler artifact
/// evidence only — package consistency is not a distribution signature,
/// notarization, or authenticity claim. Fields are crate-visible for
/// report-custody tests; the replayed evidence digest and the executable
/// member joins are what make a forged field detectable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePackagePublicationReceipt {
    pub(crate) package_root: PathBuf,
    pub(crate) application_name: String,
    pub(crate) application_identifier: build_evaluation::ApplicationIdentifier,
    /// The inner executable's ordinary v1 container commitment — the same
    /// digest the flat publication receipt carries — so package and
    /// executable custody are cross-bound rather than parallel claims.
    pub(crate) executable_container_digest: ExecutableContainerDigest,
    /// Retained inner-executable byte count. It never enters the evidence
    /// digest: its replay binding is the `Contents/MacOS/<name>` member's own
    /// byte count plus the flat receipt's container byte count at the
    /// report-level join.
    pub(crate) executable_byte_count: usize,
    /// Ordered installed members: `Contents/Info.plist` first, then
    /// `Contents/MacOS/<name>`, then each requested proof companion.
    pub(crate) components: Vec<PackagePublicationComponent>,
    pub(crate) package_evidence_digest: NativePackageEvidenceDigest,
}

impl NativePackagePublicationReceipt {
    pub fn new(
        package_root: PathBuf,
        application_name: String,
        application_identifier: build_evaluation::ApplicationIdentifier,
        executable_container_digest: ExecutableContainerDigest,
        executable_byte_count: usize,
        components: Vec<PackagePublicationComponent>,
    ) -> Self {
        let package_evidence_digest = native_package_evidence_digest(
            &package_root,
            &application_name,
            &application_identifier,
            executable_container_digest,
            &components,
        );
        Self {
            package_root,
            application_name,
            application_identifier,
            executable_container_digest,
            executable_byte_count,
            components,
            package_evidence_digest,
        }
    }

    pub fn package_root(&self) -> &Path {
        &self.package_root
    }

    pub fn application_name(&self) -> &str {
        &self.application_name
    }

    pub const fn application_identifier(&self) -> &build_evaluation::ApplicationIdentifier {
        &self.application_identifier
    }

    pub const fn executable_container_digest(&self) -> ExecutableContainerDigest {
        self.executable_container_digest
    }

    pub const fn executable_byte_count(&self) -> usize {
        self.executable_byte_count
    }

    pub fn components(&self) -> &[PackagePublicationComponent] {
        &self.components
    }

    pub const fn package_evidence_digest(&self) -> NativePackageEvidenceDigest {
        self.package_evidence_digest
    }

    /// The inner executable's recorded component: `Contents/MacOS/<name>`
    /// must be present exactly once for the receipt to describe a package.
    pub fn executable_component(&self) -> Option<&PackagePublicationComponent> {
        let expected = Path::new("Contents")
            .join("MacOS")
            .join(&self.application_name);
        let mut matching = self
            .components
            .iter()
            .filter(|component| component.relative_path == expected);
        let component = matching.next()?;
        matching.next().is_none().then_some(component)
    }

    /// The inner executable path implied by this receipt's own root and
    /// component list; never trusted independently of the executable
    /// receipt's `output_path`.
    pub fn inner_executable_path(&self) -> Option<PathBuf> {
        self.executable_component()
            .map(|component| self.package_root.join(component.relative_path()))
    }

    /// Recompute the package commitment from this receipt's own fields. Any
    /// edited field — root, name, identifier, executable commitment, member
    /// set, order, or per-member data — stops the receipt from replaying.
    /// The executable component must also sit exactly at
    /// `Contents/MacOS/<name>` and carry the retained byte count.
    pub fn has_consistent_package_identity(&self) -> bool {
        self.package_evidence_digest
            == native_package_evidence_digest(
                &self.package_root,
                &self.application_name,
                &self.application_identifier,
                self.executable_container_digest,
                &self.components,
            )
            && self
                .executable_component()
                .is_some_and(|component| component.byte_count == self.executable_byte_count)
    }
}

/// The deterministic v1 `Contents/Info.plist` for one authored application.
/// Key order is fixed (Executable, Identifier, Name, PackageType); both
/// authored strings are pre-validated to charsets without XML metacharacters,
/// so the spelling needs no escaping pass.
fn info_plist_bytes(application_name: &str, identifier: &str) -> Vec<u8> {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>CFBundleExecutable</key>\n\
         \t<string>{application_name}</string>\n\
         \t<key>CFBundleIdentifier</key>\n\
         \t<string>{identifier}</string>\n\
         \t<key>CFBundleName</key>\n\
         \t<string>{application_name}</string>\n\
         \t<key>CFBundlePackageType</key>\n\
         \t<string>APPL</string>\n\
         </dict>\n\
         </plist>\n"
    )
    .into_bytes()
}

/// The `CFBundleIdentifier` value spelled by one installed plist, or `None`
/// when the exact key/value spelling is absent. This is a deliberate
/// substring read of the fixed layout, not a general plist parser: a plist
/// that reorders or escapes its keys is a different byte string and fails
/// the exact-content replay before this read ever matters.
fn plist_identifier(plist: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(plist).ok()?;
    let value = text
        .split_once("<key>CFBundleIdentifier</key>\n\t<string>")?
        .1;
    value.split_once("</string>").map(|(value, _)| value)
}

/// Enumerate the installed regular files under `root`, package-relative and
/// sorted, so shape comparison is order-free in the filesystem but fixed in
/// the receipt.
fn installed_package_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = std::fs::read_dir(&directory)
            .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
        for entry in entries {
            let entry = entry
                .map_err(|error| format!("failed to read {}: {error}", directory.display()))?;
            let file_type = entry.file_type().map_err(|error| {
                format!("failed to inspect {}: {error}", entry.path().display())
            })?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else {
                files.push(
                    entry
                        .path()
                        .strip_prefix(root)
                        .expect("walked entry is inside the package root")
                        .to_path_buf(),
                );
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Stage, validate, install, and replay one complete `.app` package.
///
/// `companions` are extra `Contents/MacOS/<name><suffix>` members (the
/// requested Psi artifact/`.proof` pair); each suffix already includes its
/// leading dot. Every member is staged and replayed before the destination
/// is touched; a post-install replay failure removes the just-installed
/// package so nothing certified-looking survives.
pub(crate) fn publish_macos_application_package(
    build_dir: &Path,
    application_name: &str,
    application_identifier: &build_evaluation::ApplicationIdentifier,
    executable_bytes: &[u8],
    executable_container_digest: ExecutableContainerDigest,
    companions: &[(String, Vec<u8>)],
) -> Result<NativePackagePublicationReceipt, String> {
    // The signed executable must already carry the authored identity before
    // any package byte becomes visible; publication cannot re-sign it.
    let encoded_identifier =
        image_macho::code_signature_identifier(executable_bytes).ok_or_else(|| {
            "macOS package publication requires a signed executable CodeDirectory".to_owned()
        })?;
    if encoded_identifier != application_identifier.as_str() {
        return Err(
            "macOS package publication found an executable CodeDirectory identifier that differs from the retained build identifier"
                .to_owned(),
        );
    }
    let plist = info_plist_bytes(application_name, application_identifier.as_str());
    let executable_leaf = Path::new("Contents").join("MacOS").join(application_name);
    let mut members: Vec<(PathBuf, Vec<u8>)> = vec![
        (Path::new("Contents").join("Info.plist"), plist),
        (executable_leaf.clone(), executable_bytes.to_vec()),
    ];
    for (suffix, bytes) in companions {
        members.push((
            Path::new("Contents")
                .join("MacOS")
                .join(format!("{application_name}{suffix}")),
            bytes.clone(),
        ));
    }
    let mut expected_shape: Vec<PathBuf> = members
        .iter()
        .map(|(relative, _)| relative.clone())
        .collect();
    expected_shape.sort();
    let app_root = build_dir.join(format!("{application_name}.app"));
    let staged = build_dir.join(format!(
        ".{application_name}.app.{}.tmp",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&staged);
    let stage_result = (|| -> Result<(), String> {
        for (relative, bytes) in &members {
            let destination = staged.join(relative);
            let parent = destination
                .parent()
                .ok_or_else(|| "package member has no parent directory".to_owned())?;
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("failed to stage {}: {error}", parent.display()))?;
            std::fs::write(&destination, bytes)
                .map_err(|error| format!("failed to stage {}: {error}", destination.display()))?;
            let replayed = std::fs::read(&destination)
                .map_err(|error| format!("failed to replay {}: {error}", destination.display()))?;
            if replayed != *bytes {
                return Err("staged package member failed exact replay".to_owned());
            }
        }
        if installed_package_files(&staged)? != expected_shape {
            return Err("staged macOS package shape is not the exact contract tree".to_owned());
        }
        Ok(())
    })();
    if let Err(error) = stage_result {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(error);
    }
    if app_root.exists() {
        std::fs::remove_dir_all(&app_root)
            .map_err(|error| format!("failed to replace {}: {error}", app_root.display()))?;
    }
    if let Err(error) = std::fs::rename(&staged, &app_root) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(format!("failed to publish {}: {error}", app_root.display()));
    }
    if let Err(error) =
        crate::report::executable_publication::make_executable(&app_root.join(&executable_leaf))
    {
        let _ = std::fs::remove_dir_all(&app_root);
        return Err(error);
    }
    // Installed replay: exact member bytes, exact shape, and the independent
    // three-way identity join — plist identifier, retained realization
    // identity, and the CodeDirectory identity inside the installed
    // executable. Any disagreement removes the package.
    let installed_result = (|| -> Result<(), String> {
        if installed_package_files(&app_root)? != expected_shape {
            return Err("installed macOS package shape is not the exact contract tree".to_owned());
        }
        for (relative, bytes) in &members {
            let installed = std::fs::read(app_root.join(relative)).map_err(|error| {
                format!(
                    "failed to replay {}: {error}",
                    app_root.join(relative).display()
                )
            })?;
            if installed != *bytes {
                return Err("installed package member failed exact replay".to_owned());
            }
        }
        let installed_plist = std::fs::read(app_root.join("Contents").join("Info.plist"))
            .map_err(|error| format!("failed to replay installed Info.plist: {error}"))?;
        let plist_identifier = plist_identifier(&installed_plist)
            .ok_or_else(|| "installed Info.plist carries no CFBundleIdentifier".to_owned())?;
        let installed_executable = std::fs::read(app_root.join(&executable_leaf))
            .map_err(|error| format!("failed to replay installed executable: {error}"))?;
        let installed_identifier = image_macho::code_signature_identifier(&installed_executable)
            .ok_or_else(|| {
                "installed executable carries no readable CodeDirectory identifier".to_owned()
            })?;
        if plist_identifier != application_identifier.as_str()
            || installed_identifier != application_identifier.as_str()
        {
            return Err(
                "installed package identifier disagrees across plist, retained identity, and executable signature"
                    .to_owned(),
            );
        }
        Ok(())
    })();
    if let Err(error) = installed_result {
        let _ = std::fs::remove_dir_all(&app_root);
        return Err(error);
    }
    let components = members
        .iter()
        .map(|(relative, bytes)| PackagePublicationComponent {
            relative_path: relative.clone(),
            byte_count: bytes.len(),
            digest: package_component_digest(bytes),
        })
        .collect();
    Ok(NativePackagePublicationReceipt::new(
        app_root,
        application_name.to_owned(),
        application_identifier.clone(),
        executable_container_digest,
        executable_bytes.len(),
        components,
    ))
}
