//! Executable publication custody: the SHA-256 digest newtypes and domains,
//! exact-byte file publication, and the self-verifying
//! [`ExecutablePublicationReceipt`] that ties published bytes to the artifact
//! they came from.

use sha2::{Digest, Sha256};
use std::path::PathBuf;

macro_rules! publication_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

publication_digest!(NativePublicationCertificateDigest);
publication_digest!(NativePublicationEvidenceDigest);
publication_digest!(ExecutableContainerDigest);
publication_digest!(ExecutableInstallationEvidenceDigest);

pub fn executable_installation_evidence_digest(
    publication_evidence_digest: NativePublicationEvidenceDigest,
    callback_placement_identity_report_fingerprint: u64,
    output_path: &std::path::Path,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> ExecutableInstallationEvidenceDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.installed-executable-publication-evidence.sha256.v1\0");
    digest.update(publication_evidence_digest.as_bytes());
    digest.update(callback_placement_identity_report_fingerprint.to_le_bytes());
    // The fixed v1 flat-destination tag preserves existing installation digests.
    digest.update([0]);
    let path = output_path.as_os_str().as_encoded_bytes();
    digest.update((path.len() as u64).to_le_bytes());
    digest.update(path);
    digest.update((container_byte_count as u64).to_le_bytes());
    digest.update(container_digest.as_bytes());
    ExecutableInstallationEvidenceDigest::from_digest(digest.finalize().into())
}

/// Commit the producer's retained native publication evidence. This local
/// custody commitment is not a standalone proof of native behavior.
#[allow(clippy::too_many_arguments)]
pub(crate) fn native_publication_certificate_digest(
    native_artifact_identity: &[u8; 32],
    semantic_bytes: &[u8],
    proof_bytes: &[u8],
    target: target::NativeTarget,
    final_image_symbol_digest: image::FinalImageSymbolDigest,
    boundary_contract_report_fingerprint: Option<u64>,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
) -> NativePublicationCertificateDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-certificate.sha256.v1\0");
    digest.update(native_artifact_identity);
    digest.update((semantic_bytes.len() as u64).to_le_bytes());
    digest.update(semantic_bytes);
    digest.update((proof_bytes.len() as u64).to_le_bytes());
    digest.update(proof_bytes);
    digest.update([match target.architecture {
        target::Architecture::Aarch64 => 1,
        target::Architecture::X86_64 => 2,
    }]);
    digest.update([match target.object_format {
        target::ObjectFormat::Elf => 1,
        target::ObjectFormat::MachO => 2,
        target::ObjectFormat::Coff => 3,
    }]);
    digest.update((target.pointer_size as u64).to_le_bytes());
    digest.update((target.pointer_alignment as u64).to_le_bytes());
    digest.update(final_image_symbol_digest.as_bytes());
    digest.update([u8::from(boundary_contract_report_fingerprint.is_some())]);
    digest.update(
        boundary_contract_report_fingerprint
            .unwrap_or_default()
            .to_le_bytes(),
    );
    digest.update(text_validation_digest.as_bytes());
    digest.update(function_validation_digest.as_bytes());
    digest.update(function_validation_report_fingerprint.to_le_bytes());
    digest.update(inventory_digest.as_bytes());
    digest.update(inventory_report_fingerprint.to_le_bytes());
    NativePublicationCertificateDigest::from_digest(digest.finalize().into())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn native_publication_evidence_digest(
    native_artifact_identity: &[u8; 32],
    certificate_digest: NativePublicationCertificateDigest,
    callback_placement_identity_report_fingerprint: u64,
    inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    inventory_report_fingerprint: u64,
    text_validation_digest: image::CompilerTextDerivationDigest,
    function_validation_digest: image::CompilerFunctionValidationDigest,
    function_validation_report_fingerprint: u64,
    container_byte_count: usize,
    container_digest: ExecutableContainerDigest,
) -> NativePublicationEvidenceDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.native-publication-evidence.sha256.v1\0");
    digest.update(native_artifact_identity);
    digest.update(certificate_digest.as_bytes());
    digest.update(function_validation_digest.as_bytes());
    digest.update(inventory_digest.as_bytes());
    for value in [
        callback_placement_identity_report_fingerprint,
        inventory_report_fingerprint,
        function_validation_report_fingerprint,
        container_byte_count as u64,
    ] {
        digest.update(value.to_le_bytes());
    }
    digest.update(text_validation_digest.as_bytes());
    digest.update(container_digest.as_bytes());
    NativePublicationEvidenceDigest::from_digest(digest.finalize().into())
}

pub(crate) fn executable_container_digest(bytes: &[u8]) -> ExecutableContainerDigest {
    let mut digest = Sha256::new();
    digest.update(b"omega.published-executable-container.sha256.v1\0");
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    ExecutableContainerDigest::from_digest(digest.finalize().into())
}

pub(crate) fn publish_exact_executable_bytes(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<(), String> {
    publish_exact_bytes(path, bytes, true)
}

/// Publish ordinary non-executable product bytes — Psi artifacts and `.proof`
/// companions — with the same stage/replay/rename/replay discipline as the
/// executable path.
pub(crate) fn publish_exact_file_bytes(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    publish_exact_bytes(path, bytes, false)
}

/// One product file written and replayed beside its destination, not yet
/// visible there.
///
/// Publication of a PAIR cannot be a sequence of single-file publications:
/// "Publication must not associate a stale sidecar with newly written bytes"
/// (spec, `proofs/publication.md`), and writing the artifact and then its
/// companion leaves exactly that arrangement when the second write fails.
/// Staging every member first, then installing them, means a write failure
/// leaves the previous consistent pair untouched. The spec permits the
/// residue: "Failure may leave diagnostic staging data".
pub(crate) struct StagedProduct {
    staged: std::path::PathBuf,
    destination: std::path::PathBuf,
    /// Retained for the post-install replay. Owned rather than borrowed so a
    /// staged set can outlive the scopes that produced each member's bytes.
    bytes: Vec<u8>,
}

/// Write `bytes` beside `path` and replay them, without making them visible
/// at `path`.
pub(crate) fn stage_exact_bytes(
    path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<StagedProduct, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "native publication path has no parent directory".to_owned())?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("omega-output");
    let staged = parent.join(format!(".{file_name}.{}.tmp", std::process::id()));
    let _ = std::fs::remove_file(&staged);
    std::fs::write(&staged, bytes)
        .map_err(|error| format!("failed to stage {}: {error}", staged.display()))?;
    let staged_bytes = std::fs::read(&staged)
        .map_err(|error| format!("failed to replay {}: {error}", staged.display()))?;
    if staged_bytes != bytes {
        let _ = std::fs::remove_file(&staged);
        return Err("staged native output bytes failed exact replay".to_owned());
    }
    if executable && let Err(error) = make_executable(&staged) {
        let _ = std::fs::remove_file(&staged);
        return Err(error);
    }
    Ok(StagedProduct {
        staged,
        destination: path.to_path_buf(),
        bytes: bytes.to_vec(),
    })
}

/// Make one staged product visible at its destination and replay it there.
pub(crate) fn install_staged_product(product: &StagedProduct) -> Result<(), String> {
    install_staged(&product.staged, &product.destination)?;
    let installed = std::fs::read(&product.destination).map_err(|error| {
        format!(
            "failed to replay {}: {error}",
            product.destination.display()
        )
    })?;
    if installed != product.bytes {
        let _ = std::fs::remove_file(&product.destination);
        return Err("published native output bytes failed exact replay".to_owned());
    }
    Ok(())
}

/// Install every member of one requested set, or none of them.
pub(crate) fn install_staged_products(products: &[StagedProduct]) -> Result<(), String> {
    for product in products {
        install_staged_product(product)?;
    }
    Ok(())
}

fn publish_exact_bytes(
    path: &std::path::Path,
    bytes: &[u8],
    executable: bool,
) -> Result<(), String> {
    install_staged_product(&stage_exact_bytes(path, bytes, executable)?)
}

/// Install the staged file at its destination. Unix `rename` replaces an
/// existing file atomically; platforms whose rename refuses an occupied
/// destination remove it first. Either way the staged bytes were already
/// replayed, and on Unix the prior output survives a failed rename.
#[cfg(unix)]
fn install_staged(staged: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    std::fs::rename(staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

#[cfg(not(unix))]
fn install_staged(staged: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|error| format!("failed to replace {}: {error}", path.display()))?;
    }
    std::fs::rename(staged, path)
        .map_err(|error| format!("failed to publish {}: {error}", path.display()))
}

#[cfg(unix)]
pub(crate) fn make_executable(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .map_err(|error| format!("failed to make {} executable: {error}", path.display()))
}

#[cfg(not(unix))]
pub(crate) fn make_executable(_path: &std::path::Path) -> Result<(), String> {
    Ok(())
}

/// Remove one companion this publication is not refreshing. The publication
/// contract forbids associating a stale sidecar with newly written bytes, so
/// an unrequested artifact/`.proof` pair cannot survive beside the artifact
/// it no longer commits to. Absence is the expected common case; every other
/// removal failure aborts publication before any new bytes are installed.
pub(crate) fn remove_stale_companion(path: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to remove stale proof companion {}: {error}",
            path.display()
        )),
    }
}

/// The adjacent `.proof`/`.psi` companion path beside `path`, formed by
/// appending `suffix` to the complete artifact filename per the contract.
pub(crate) fn appended_file_name_path(path: &std::path::Path, suffix: &str) -> PathBuf {
    let mut name = path
        .file_name()
        .expect("a publication path always has a file name")
        .to_os_string();
    name.push(suffix);
    path.with_file_name(name)
}

/// Producer-side pair validation for one Psi artifact/companion: replay the
/// exact checks a receiver performs under a self-consistent policy so a
/// malformed or internally inconsistent pair can never be reported as
/// published.
pub(crate) fn validate_psi_pair(
    psi_bytes: &[u8],
    sidecar: &terminal_codec::PccProofSidecar,
    admission_profile: &proof_admission::AdmissionProfile,
) -> Result<(), String> {
    let policy =
        terminal_codec::PccReceiverPolicy::for_offered_claim(sidecar, admission_profile.clone());
    match terminal_codec::verify_psi_proof_sidecar(psi_bytes, &sidecar.to_bytes(), &policy) {
        terminal_codec::PccVerificationOutcome::Complete(_) => Ok(()),
        outcome => Err(format!(
            "psi proof sidecar failed producer validation: {outcome:?}"
        )),
    }
}

/// Producer-side pair validation for one native executable/companion: replay
/// the exact checks a receiver performs under a self-consistent policy so a
/// malformed or byte-inconsistent pair can never be reported as published.
/// The bounded placed-image evidence must decode and replay against the
/// published bytes. `Incomplete(UnsupportedEvidence)` is the honest ceiling —
/// the coverage leg validates while the behavioral legs have no standalone
/// checking yet — but `Reject` or a named resource limit means the emitted
/// evidence cannot carry even its bounded claim, and refuses publication.
pub(crate) fn validate_native_pair(
    executable_bytes: &[u8],
    sidecar: &terminal_codec::PccProofSidecar,
    admission_profile: &proof_admission::AdmissionProfile,
) -> Result<(), String> {
    let policy =
        terminal_codec::PccReceiverPolicy::for_offered_claim(sidecar, admission_profile.clone());
    match crate::pcc::verify_native_proof_sidecar(executable_bytes, &sidecar.to_bytes(), &policy) {
        terminal_codec::PccVerificationOutcome::Complete(_)
        | terminal_codec::PccVerificationOutcome::Incomplete(
            terminal_codec::PccIncompleteness::UnsupportedEvidence {
                product: terminal_codec::PccProductKind::Native,
            },
        ) => Ok(()),
        outcome => Err(format!(
            "native proof sidecar failed producer validation: {outcome:?}"
        )),
    }
}

/// Immutable custody for one compiler-published executable container.
///
/// This records the exact final-footprint certificate, publication seal, and
/// checked atomic installation that produced `output_path`. It is compiler
/// artifact evidence only; it does not authorize loading or runtime
/// installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutablePublicationReceipt {
    pub(crate) output_path: PathBuf,
    pub(crate) native_artifact_identity: [u8; 32],
    pub(crate) certificate_digest: NativePublicationCertificateDigest,
    /// Compact report coordinate. Exact callback placements are structurally
    /// replayed before this publication receipt can be produced.
    pub(crate) callback_placement_identity_report_fingerprint: u64,
    /// Compact report coordinate; exact final footprint authority is retained
    /// by the placed-region inventory and certificate digest. Crate-visible
    /// like its siblings for report-custody tests.
    pub(crate) boundary_contract_report_fingerprint: Option<u64>,
    pub(crate) inventory_digest: image::PlacedExecutableRegionInventoryDigest,
    /// Compact report compatibility only. Publication and replay authority is
    /// `inventory_digest`.
    pub(crate) inventory_report_fingerprint: u64,
    pub(crate) compiler_text_validation_digest: image::CompilerTextDerivationDigest,
    pub(crate) compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
    /// Compact report compatibility only. The validation digest preserves this
    /// summary's custody, while exact text and inventory commitments retain the
    /// underlying publication and replay authority.
    pub(crate) compiler_function_validation_report_fingerprint: u64,
    pub(crate) publication_evidence_digest: NativePublicationEvidenceDigest,
    pub(crate) container_byte_count: usize,
    pub(crate) container_digest: ExecutableContainerDigest,
    pub(crate) installation_evidence_digest: ExecutableInstallationEvidenceDigest,
}

impl ExecutablePublicationReceipt {
    pub fn new(
        output_path: PathBuf,
        native_artifact_identity: [u8; 32],
        certificate_digest: NativePublicationCertificateDigest,
        callback_placement_identity_report_fingerprint: u64,
        boundary_contract_report_fingerprint: Option<u64>,
        inventory_digest: image::PlacedExecutableRegionInventoryDigest,
        inventory_report_fingerprint: u64,
        compiler_text_validation_digest: image::CompilerTextDerivationDigest,
        compiler_function_validation_digest: image::CompilerFunctionValidationDigest,
        compiler_function_validation_report_fingerprint: u64,
        publication_evidence_digest: NativePublicationEvidenceDigest,
        container_byte_count: usize,
        container_digest: ExecutableContainerDigest,
        installation_evidence_digest: ExecutableInstallationEvidenceDigest,
    ) -> Self {
        Self {
            output_path,
            native_artifact_identity,
            certificate_digest,
            callback_placement_identity_report_fingerprint,
            boundary_contract_report_fingerprint,
            inventory_digest,
            inventory_report_fingerprint,
            compiler_text_validation_digest,
            compiler_function_validation_digest,
            compiler_function_validation_report_fingerprint,
            publication_evidence_digest,
            container_byte_count,
            container_digest,
            installation_evidence_digest,
        }
    }

    pub fn output_path(&self) -> &std::path::Path {
        &self.output_path
    }

    pub const fn certificate_digest(&self) -> NativePublicationCertificateDigest {
        self.certificate_digest
    }

    pub const fn native_artifact_identity(&self) -> &[u8; 32] {
        &self.native_artifact_identity
    }

    pub const fn callback_placement_identity_report_fingerprint(&self) -> u64 {
        self.callback_placement_identity_report_fingerprint
    }

    pub const fn boundary_contract_report_fingerprint(&self) -> Option<u64> {
        self.boundary_contract_report_fingerprint
    }

    pub const fn inventory_digest(&self) -> image::PlacedExecutableRegionInventoryDigest {
        self.inventory_digest
    }

    pub const fn inventory_report_fingerprint(&self) -> u64 {
        self.inventory_report_fingerprint
    }

    pub const fn compiler_text_validation_digest(&self) -> image::CompilerTextDerivationDigest {
        self.compiler_text_validation_digest
    }

    pub const fn compiler_function_validation_report_fingerprint(&self) -> u64 {
        self.compiler_function_validation_report_fingerprint
    }

    pub const fn compiler_function_validation_digest(
        &self,
    ) -> image::CompilerFunctionValidationDigest {
        self.compiler_function_validation_digest
    }

    pub const fn publication_evidence_digest(&self) -> NativePublicationEvidenceDigest {
        self.publication_evidence_digest
    }

    pub const fn container_byte_count(&self) -> usize {
        self.container_byte_count
    }

    pub const fn container_digest(&self) -> ExecutableContainerDigest {
        self.container_digest
    }

    pub const fn installation_evidence_digest(&self) -> ExecutableInstallationEvidenceDigest {
        self.installation_evidence_digest
    }

    pub fn has_consistent_installation_identity(&self) -> bool {
        self.publication_evidence_digest
            == native_publication_evidence_digest(
                &self.native_artifact_identity,
                self.certificate_digest,
                self.callback_placement_identity_report_fingerprint,
                self.inventory_digest,
                self.inventory_report_fingerprint,
                self.compiler_text_validation_digest,
                self.compiler_function_validation_digest,
                self.compiler_function_validation_report_fingerprint,
                self.container_byte_count,
                self.container_digest,
            )
            && self.installation_evidence_digest
                == executable_installation_evidence_digest(
                    self.publication_evidence_digest,
                    self.callback_placement_identity_report_fingerprint,
                    &self.output_path,
                    self.container_byte_count,
                    self.container_digest,
                )
    }

    /// Replay the published file against this receipt: length, content digest
    /// and executable mode must all still match — the publication contract's
    /// byte- and mode-drift detection.
    pub fn validate_published_file(&self) -> Result<(), String> {
        let bytes = std::fs::read(&self.output_path).map_err(|error| {
            format!(
                "cannot replay published executable {}: {error}",
                self.output_path.display()
            )
        })?;
        if bytes.len() != self.container_byte_count {
            return Err(format!(
                "published executable {} has {} bytes; the receipt recorded {}",
                self.output_path.display(),
                bytes.len(),
                self.container_byte_count
            ));
        }
        if executable_container_digest(&bytes) != self.container_digest {
            return Err(format!(
                "published executable {} no longer matches its receipt's content digest",
                self.output_path.display()
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = std::fs::metadata(&self.output_path)
                .map_err(|error| {
                    format!(
                        "cannot stat published executable {}: {error}",
                        self.output_path.display()
                    )
                })?
                .permissions()
                .mode();
            if mode & 0o777 != 0o755 {
                return Err(format!(
                    "published executable {} drifted to mode {:o}; the receipt expects 755",
                    self.output_path.display(),
                    mode & 0o7777
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Filesystem legs of the publication operation itself: stage,
    //! staged-replay, rename, installed-replay, executable-bit, staged-name
    //! hygiene, companion naming, and stale-companion removal. The receipt
    //! chain above this is covered by `compile_report/custody_tests.rs`.
    use super::{
        ExecutablePublicationReceipt, appended_file_name_path, executable_container_digest,
        install_staged_products, publish_exact_executable_bytes, publish_exact_file_bytes,
        remove_stale_companion, stage_exact_bytes,
    };
    use std::path::{Path, PathBuf};

    /// A fresh per-test destination directory under the system temp root.

    /// "Publication must not associate a stale sidecar with newly written
    /// bytes" (spec, `proofs/publication.md`). The interruption this pins is a
    /// failure BETWEEN pair members, which a sequence of single-file
    /// publications cannot survive: the artifact lands, its companion does
    /// not, and the previous run's `.proof` is left describing bytes nobody
    /// proved.
    #[test]
    fn a_failed_pair_member_leaves_the_previous_pair_intact() {
        let directory = TestDir::new("pair-interruption");
        let artifact = directory.path("product.psi");
        let companion = appended_file_name_path(&artifact, ".proof");

        publish_exact_file_bytes(&artifact, b"first-artifact").expect("publish first artifact");
        publish_exact_file_bytes(&companion, b"first-proof").expect("publish first companion");

        // The second publication stages both members and one of them fails.
        // Staging the companion into a path whose parent does not exist is
        // that failure; the artifact was staged successfully first.
        let staged_artifact =
            stage_exact_bytes(&artifact, b"second-artifact", false).expect("stage artifact");
        let unwritable = directory.path("absent-directory").join("product.psi.proof");
        assert!(
            stage_exact_bytes(&unwritable, b"second-proof", false).is_err(),
            "staging into a missing directory must fail",
        );

        // Nothing was installed, so the first pair still agrees.
        assert_eq!(
            std::fs::read(&artifact).expect("read artifact"),
            b"first-artifact",
            "a failed pair member must not leave new artifact bytes behind",
        );
        assert_eq!(
            std::fs::read(&companion).expect("read companion"),
            b"first-proof",
        );

        // Installing the complete set replaces the pair together.
        let staged_companion =
            stage_exact_bytes(&companion, b"second-proof", false).expect("stage companion");
        install_staged_products(&[staged_artifact, staged_companion]).expect("install pair");
        assert_eq!(
            std::fs::read(&artifact).expect("read artifact"),
            b"second-artifact",
        );
        assert_eq!(
            std::fs::read(&companion).expect("read companion"),
            b"second-proof",
        );
    }

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "omega-publication-test-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("create test directory");
            Self(root)
        }

        fn path(&self, name: impl AsRef<Path>) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn directory_entries(dir: &Path) -> Vec<String> {
        std::fs::read_dir(dir)
            .expect("read test directory")
            .map(|entry| entry.expect("read directory entry").file_name())
            .map(|name| name.into_string().expect("utf-8 file name"))
            .collect()
    }

    #[test]
    fn published_executable_replays_exact_bytes_and_leaves_no_staged_file() {
        let dir = TestDir::new("executable-exact");
        let output = dir.path("product");
        let bytes = b"\x7fELF-pinned-bytes".to_vec();

        publish_exact_executable_bytes(&output, &bytes).expect("publish executable");

        assert_eq!(std::fs::read(&output).expect("replay output"), bytes);
        assert_eq!(
            directory_entries(&dir.0),
            vec!["product".to_owned()],
            "publication leaves exactly the installed name; the staged .tmp is renamed, not copied"
        );
    }

    #[cfg(unix)]
    #[test]
    fn published_executable_carries_the_executable_bit() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TestDir::new("executable-mode");
        let output = dir.path("product");

        publish_exact_executable_bytes(&output, b"bytes").expect("publish executable");

        let mode = std::fs::metadata(&output)
            .expect("read output metadata")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o777,
            0o755,
            "installed executable is world-readable, owner-writable"
        );
    }

    #[cfg(unix)]
    #[test]
    fn published_plain_file_stays_non_executable() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TestDir::new("plain-mode");
        let output = dir.path("product.psi");

        publish_exact_file_bytes(&output, b"psi-bytes").expect("publish file");

        assert_eq!(std::fs::read(&output).expect("replay output"), b"psi-bytes");
        let mode = std::fs::metadata(&output)
            .expect("read output metadata")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o111,
            0,
            "a data companion never acquires the executable bit"
        );
    }

    #[test]
    fn republication_replaces_an_installed_file_with_exact_new_bytes() {
        let dir = TestDir::new("republish");
        let output = dir.path("product");
        std::fs::write(&output, b"old-bytes").expect("install prior output");

        publish_exact_executable_bytes(&output, b"new-bytes").expect("republish");

        assert_eq!(std::fs::read(&output).expect("replay output"), b"new-bytes");
    }

    #[test]
    fn publication_clears_a_leftover_staged_file_before_writing() {
        let dir = TestDir::new("stale-staged");
        let output = dir.path("product");
        let staged = dir.path(format!(".product.{}.tmp", std::process::id()));
        std::fs::write(&staged, b"junk-from-an-aborted-attempt").expect("plant stale staged file");

        publish_exact_executable_bytes(&output, b"pinned").expect("publish over stale staged file");

        assert_eq!(std::fs::read(&output).expect("replay output"), b"pinned");
        assert!(!staged.exists(), "staged name is consumed by the rename");
    }

    #[test]
    fn publication_into_a_missing_directory_refuses() {
        let dir = TestDir::new("missing-parent");
        let output = dir.path("absent-parent/product");

        let error =
            publish_exact_executable_bytes(&output, b"bytes").expect_err("parent is absent");

        assert!(
            error.contains("failed to stage"),
            "staging failure names its step, got: {error}"
        );
        assert!(!output.exists());
    }

    #[test]
    fn companion_path_appends_to_the_complete_file_name() {
        let dir = TestDir::new("companion-path");
        let output = dir.path("nested/product");

        assert_eq!(
            appended_file_name_path(&output, ".psi"),
            dir.path("nested/product.psi")
        );
        assert_eq!(
            appended_file_name_path(&appended_file_name_path(&output, ".psi"), ".proof"),
            dir.path("nested/product.psi.proof"),
            "companion suffixes append to the full artifact name, never replace it"
        );
    }

    #[test]
    fn stale_companion_removal_is_absent_tolerant() {
        let dir = TestDir::new("companion-absent");

        remove_stale_companion(&dir.path("product.proof"))
            .expect("an absent companion is the common case");
    }

    #[test]
    fn stale_companion_removal_deletes_the_file() {
        let dir = TestDir::new("companion-present");
        let companion = dir.path("product.proof");
        std::fs::write(&companion, b"stale").expect("install stale companion");

        remove_stale_companion(&companion).expect("remove stale companion");

        assert!(!companion.exists());
    }

    #[test]
    fn stale_companion_removal_refuses_a_non_file() {
        let dir = TestDir::new("companion-directory");
        let companion = dir.path("product.proof");
        std::fs::create_dir_all(&companion).expect("install directory at companion path");

        let error = remove_stale_companion(&companion).expect_err("directories refuse removal");

        assert!(
            error.contains("failed to remove stale proof companion"),
            "removal failure names its step, got: {error}"
        );
    }

    /// A receipt whose container commitment matches `bytes` at `output`; the
    /// evidence chains are unrelated to the filesystem replay under test.
    fn receipt_for(output: &Path, bytes: &[u8]) -> ExecutablePublicationReceipt {
        use super::{
            ExecutableInstallationEvidenceDigest, NativePublicationCertificateDigest,
            NativePublicationEvidenceDigest,
        };

        ExecutablePublicationReceipt::new(
            output.to_path_buf(),
            [0; 32],
            NativePublicationCertificateDigest::from_digest([0; 32]),
            0,
            None,
            image::PlacedExecutableRegionInventoryDigest::from_digest([0; 32]),
            0,
            image::CompilerTextDerivationDigest::from_digest([0; 32]),
            image::CompilerFunctionValidationDigest::from_digest([0; 32]),
            0,
            NativePublicationEvidenceDigest::from_digest([0; 32]),
            bytes.len(),
            executable_container_digest(bytes),
            ExecutableInstallationEvidenceDigest::from_digest([0; 32]),
        )
    }

    #[test]
    fn receipt_replay_accepts_the_published_executable_unchanged() {
        let dir = TestDir::new("receipt-clean");
        let output = dir.path("product");
        let bytes = b"\x7fELF-pinned-bytes".to_vec();
        publish_exact_executable_bytes(&output, &bytes).expect("publish executable");

        receipt_for(&output, &bytes)
            .validate_published_file()
            .expect("unchanged published file replays");
    }

    #[test]
    fn receipt_replay_detects_byte_drift() {
        let dir = TestDir::new("receipt-byte-drift");
        let output = dir.path("product");
        publish_exact_executable_bytes(&output, b"original").expect("publish executable");
        std::fs::write(&output, b"mutated!").expect("drift the installed bytes");

        let error = receipt_for(&output, b"original")
            .validate_published_file()
            .expect_err("drifted bytes must not replay");

        assert!(
            error.contains("no longer matches its receipt's content digest"),
            "byte drift names the digest mismatch, got: {error}"
        );
    }

    #[test]
    fn receipt_replay_detects_length_drift() {
        let dir = TestDir::new("receipt-length-drift");
        let output = dir.path("product");
        publish_exact_executable_bytes(&output, b"exact").expect("publish executable");
        std::fs::write(&output, b"exact-but-longer").expect("drift the installed length");

        let error = receipt_for(&output, b"exact")
            .validate_published_file()
            .expect_err("length drift must not replay");

        assert!(
            error.contains("the receipt recorded"),
            "length drift names the recorded count, got: {error}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn receipt_replay_detects_mode_drift() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TestDir::new("receipt-mode-drift");
        let output = dir.path("product");
        publish_exact_executable_bytes(&output, b"mode").expect("publish executable");
        let mut permissions = std::fs::metadata(&output)
            .expect("read output metadata")
            .permissions();
        permissions.set_mode(0o644);
        std::fs::set_permissions(&output, permissions).expect("drift the executable bit");

        let error = receipt_for(&output, b"mode")
            .validate_published_file()
            .expect_err("mode drift must not replay");

        assert!(
            error.contains("drifted to mode"),
            "mode drift names the observed mode, got: {error}"
        );
    }
}
