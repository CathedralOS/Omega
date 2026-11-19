/*
We stage, validate, install, and replay macOS `.app` application bundles
(`publish_macos_application_package`) and issue self-verifying
`NativePackagePublicationReceipt` records. We stage the complete bundle tree
(`Contents/Info.plist` and `Contents/MacOS/<name>`) in a temporary sibling
directory, verify that the signed Mach-O `CodeDirectory` identifier matches the
retained `ApplicationIdentifier` and plist `CFBundleIdentifier`, atomically
replace any prior bundle, and replay the installed tree shape and bytes.
*/

use std::path::{Path, PathBuf};

use crate::ExecutableContainerDigest;
use crate::digest_encoder::DomainDigestEncoder;
use crate::publication;
use crate::{NativePackageEvidenceDigest, PackageComponentDigest};
use platform_custody::file_publication::make_executable;
use std::process::id;

fn package_component_digest(bytes: &[u8]) -> PackageComponentDigest {
    PackageComponentDigest::from_digest(DomainDigestEncoder::content(
        b"omega.published-package-component.sha256.v1\0",
        bytes,
    ))
}

fn native_package_evidence_digest(
    package_root: &Path,
    application_name: &str,
    application_identifier: &target::ApplicationIdentifier,
    executable_container_digest: ExecutableContainerDigest,
    components: &[PackagePublicationComponent],
) -> NativePackageEvidenceDigest {
    let mut digest = DomainDigestEncoder::new(b"omega.native-package-evidence.sha256.v1\0");
    let root = package_root.as_os_str().as_encoded_bytes();
    digest.u64(root.len() as u64);
    digest.raw(root);
    digest.u64(application_name.len() as u64);
    digest.raw(application_name.as_bytes());
    let identifier = application_identifier.as_str().as_bytes();
    digest.u64(identifier.len() as u64);
    digest.raw(identifier);
    digest.raw(executable_container_digest.as_bytes());
    digest.u64(components.len() as u64);
    for component in components {
        let relative = component.relative_path.as_os_str().as_encoded_bytes();
        digest.u64(relative.len() as u64);
        digest.raw(relative);
        digest.u64(component.byte_count as u64);
        digest.raw(component.digest.as_bytes());
    }
    NativePackageEvidenceDigest::from_digest(digest.finish())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagePublicationComponent {
    relative_path: PathBuf,
    byte_count: usize,
    digest: PackageComponentDigest,
}

impl PackagePublicationComponent {
    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub const fn byte_count(&self) -> usize {
        self.byte_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativePackagePublicationReceipt {
    package_root: PathBuf,
    application_name: String,
    application_identifier: target::ApplicationIdentifier,
    executable_container_digest: ExecutableContainerDigest,
    executable_byte_count: usize,
    components: Vec<PackagePublicationComponent>,
    package_evidence_digest: NativePackageEvidenceDigest,
}

impl NativePackagePublicationReceipt {
    pub fn new(
        package_root: PathBuf,
        application_name: String,
        application_identifier: target::ApplicationIdentifier,
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

    pub const fn application_identifier(&self) -> &target::ApplicationIdentifier {
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

    pub(crate) fn executable_component(&self) -> Option<&PackagePublicationComponent> {
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

    pub fn has_consistent_package_evidence(&self) -> bool {
        let contract_members = [
            Path::new("Contents").join("Info.plist"),
            Path::new("Contents")
                .join("MacOS")
                .join(&self.application_name),
        ];
        let members_match = self.components.len() == contract_members.len()
            && self
                .components
                .iter()
                .zip(&contract_members)
                .all(|(component, expected)| component.relative_path == *expected);
        let root_matches = self.package_root.file_name()
            == Some(std::ffi::OsStr::new(&format!(
                "{}.app",
                self.application_name
            )));
        let executable_matches = self
            .executable_component()
            .is_some_and(|component| component.byte_count == self.executable_byte_count);
        members_match
            && root_matches
            && executable_matches
            && self.package_evidence_digest
                == native_package_evidence_digest(
                    &self.package_root,
                    &self.application_name,
                    &self.application_identifier,
                    self.executable_container_digest,
                    &self.components,
                )
    }

    pub fn binds_executable(&self, executable: &crate::ExecutablePublicationReceipt) -> bool {
        self.has_consistent_package_evidence()
            && self.inner_executable_path().as_deref() == Some(executable.output_path())
            && self.executable_container_digest == executable.container_digest()
            && self.executable_byte_count == executable.container_byte_count()
    }

    pub fn inner_executable_path(&self) -> Option<PathBuf> {
        self.executable_component()
            .map(|component| self.package_root.join(component.relative_path()))
    }
}

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

fn plist_identifier(plist: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(plist).ok()?;
    let value = text
        .split_once("<key>CFBundleIdentifier</key>\n\t<string>")?
        .1;
    value.split_once("</string>").map(|(value, _)| value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PackageEntryKind {
    Directory,
    RegularFile,
    Other,
}

fn package_tree_entries(root: &Path) -> Result<Vec<(PathBuf, PackageEntryKind)>, String> {
    let mut entries_found = Vec::new();
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
            let kind = if file_type.is_dir() {
                pending.push(entry.path());
                PackageEntryKind::Directory
            } else if file_type.is_file() {
                PackageEntryKind::RegularFile
            } else {
                PackageEntryKind::Other
            };
            let relative = entry
                .path()
                .strip_prefix(root)
                .expect("walked entry is inside the package root")
                .to_path_buf();
            entries_found.push((relative, kind));
        }
    }
    entries_found.sort();
    Ok(entries_found)
}

fn expected_package_tree(members: &[(PathBuf, Vec<u8>)]) -> Vec<(PathBuf, PackageEntryKind)> {
    let mut entries = Vec::new();
    for (relative, _) in members {
        entries.push((relative.clone(), PackageEntryKind::RegularFile));
        for ancestor in relative.ancestors().skip(1) {
            if ancestor.as_os_str().is_empty() {
                continue;
            }
            entries.push((ancestor.to_path_buf(), PackageEntryKind::Directory));
        }
    }
    entries.sort();
    entries.dedup();
    entries
}

pub fn publish_macos_application_package(
    build_dir: &Path,
    application_name: &str,
    application_identifier: &target::ApplicationIdentifier,
    executable_bytes: &[u8],
    executable_container_digest: ExecutableContainerDigest,
) -> Result<NativePackagePublicationReceipt, String> {
    publish_package_with_observers(
        build_dir,
        application_name,
        application_identifier,
        executable_bytes,
        executable_container_digest,
        PublicationObservers::NONE,
    )
}

struct PublicationObservers<'a> {
    after_staging: &'a dyn Fn(&Path),
    after_installation: &'a dyn Fn(&Path),
}

impl PublicationObservers<'_> {
    const NONE: PublicationObservers<'static> = PublicationObservers {
        after_staging: &|_| {},
        after_installation: &|_| {},
    };
}

fn publish_package_with_observers(
    build_dir: &Path,
    application_name: &str,
    application_identifier: &target::ApplicationIdentifier,
    executable_bytes: &[u8],
    executable_container_digest: ExecutableContainerDigest,
    observers: PublicationObservers<'_>,
) -> Result<NativePackagePublicationReceipt, String> {
    if publication::executable_container_digest(executable_bytes) != executable_container_digest {
        return Err(
            "macOS package executable bytes do not match the container commitment".to_owned(),
        );
    }
    if application_name.is_empty()
        || !application_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return Err("macOS package publication requires a local ASCII application name".to_owned());
    }
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
    let members: Vec<(PathBuf, Vec<u8>)> = vec![
        (Path::new("Contents").join("Info.plist"), plist),
        (executable_leaf.clone(), executable_bytes.to_vec()),
    ];
    let expected_shape = expected_package_tree(&members);
    let app_root = build_dir.join(format!("{application_name}.app"));
    let staged = build_dir.join(format!(".{application_name}.app.{}.tmp", id()));
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
        (observers.after_staging)(&staged);
        if package_tree_entries(&staged)? != expected_shape {
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
    if let Err(error) = make_executable(&app_root.join(&executable_leaf)) {
        let _ = std::fs::remove_dir_all(&app_root);
        return Err(error);
    }
    (observers.after_installation)(&app_root);
    let installed_result = (|| -> Result<(), String> {
        if package_tree_entries(&app_root)? != expected_shape {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::publication::executable_container_digest;
    use std::fs;
    use std::fs::read;
    use std::process::id;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct PublicationDirectory(PathBuf);

    impl PublicationDirectory {
        fn new() -> Self {
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("omega-package-publication-{}-{sequence}", id()));
            fs::create_dir(&path).expect("create package publication test directory");
            Self(path)
        }
    }

    impl Drop for PublicationDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn signed_executable(identifier: &str) -> Vec<u8> {
        let mut image = image::FinalImage::with_capacity(
            target::NativeTarget::macos_arm64(),
            image::FinalImageMemory {
                text: 0xd65f_03c0u32.to_le_bytes().to_vec(),
                ..Default::default()
            },
            image::FinalImageSymbolHandle::invalid(),
            1,
            0,
            0,
        );
        image.symbol_table.entry_symbol =
            image.symbol_table.symbols.insert(image::FinalImageSymbol {
                name: "entry".into(),
                section: image::FinalImageSection::Text,
                size: 4,
                kind: object_file::SymbolKind::Function,
                ..Default::default()
            });
        image_macho::emit_macho_executable_signed(image, identifier)
            .expect("emit signed fixture")
            .bytes
    }

    #[test]
    fn package_publication_is_deterministic_and_binds_the_signed_identifier() {
        let directory = PublicationDirectory::new();
        let identifier = target::ApplicationIdentifier::new(b"org.omega.window-test").unwrap();
        let executable = signed_executable(identifier.as_str());
        let digest = executable_container_digest(&executable);
        let publish = || {
            publish_macos_application_package(
                &directory.0,
                "window_test",
                &identifier,
                &executable,
                digest,
            )
            .unwrap()
        };
        let first = publish();
        assert_eq!(
            first.inner_executable_path().unwrap(),
            directory
                .0
                .join("window_test.app/Contents/MacOS/window_test")
        );
        assert_eq!(
            read(first.inner_executable_path().unwrap()).unwrap(),
            executable
        );
        let plist_path = first.package_root().join("Contents/Info.plist");
        let first_plist = read(&plist_path).unwrap();
        assert_eq!(plist_identifier(&first_plist), Some(identifier.as_str()));
        assert_eq!(first.components().len(), 2);
        assert!(
            !directory.0.join("window_test").exists(),
            "no redundant flat output"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(first.inner_executable_path().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
        assert_eq!(publish(), first);
        assert_eq!(read(plist_path).unwrap(), first_plist);
        assert!(first.has_consistent_package_evidence());
        let mut changed = first.clone();
        changed.application_identifier = target::ApplicationIdentifier::new(b"org.other").unwrap();
        assert!(
            !changed.has_consistent_package_evidence(),
            "a changed identifier must fail the receipt replay"
        );
        let mut changed = first.clone();
        changed.components.reverse();
        assert!(
            !changed.has_consistent_package_evidence(),
            "reordered members must fail the receipt replay"
        );
        let mut changed = first.clone();
        changed.package_root = directory.0.join("different.app");
        assert!(
            !changed.has_consistent_package_evidence(),
            "a changed package root must fail the receipt replay"
        );
        let mut changed = first;
        changed.executable_byte_count += 1;
        assert!(
            !changed.has_consistent_package_evidence(),
            "an executable byte count that disagrees with its member must fail the replay"
        );
    }

    #[test]
    fn invalid_identity_paths_and_staging_leave_the_previous_package_untouched() {
        let directory = PublicationDirectory::new();
        let identifier = target::ApplicationIdentifier::new(b"org.omega.window-test").unwrap();
        let executable = signed_executable(identifier.as_str());
        let digest = executable_container_digest(&executable);
        let previous = directory.0.join("window_test.app");
        fs::create_dir(&previous).unwrap();
        fs::write(previous.join("previous"), b"retained").unwrap();
        let wrong_identifier = target::ApplicationIdentifier::new(b"org.other").unwrap();
        assert!(
            publish_macos_application_package(
                &directory.0,
                "window_test",
                &identifier,
                &executable,
                executable_container_digest(b"different bytes"),
            )
            .is_err()
        );
        assert!(
            publish_macos_application_package(
                &directory.0,
                "window_test",
                &wrong_identifier,
                &executable,
                digest,
            )
            .is_err()
        );
        for name in [
            "",
            "../window_test",
            "/window_test",
            "bad&name",
            "bad\\name",
        ] {
            assert!(
                publish_macos_application_package(
                    &directory.0,
                    name,
                    &identifier,
                    &executable,
                    digest,
                )
                .is_err(),
                "{name}"
            );
        }
        assert_eq!(read(previous.join("previous")).unwrap(), b"retained");
        assert_eq!(
            fs::read_dir(&directory.0).unwrap().count(),
            1,
            "failed staging leaves no temporary package"
        );
    }

    fn publication_rejects_disturbed_tree(stage: bool, disturb: &dyn Fn(&Path)) {
        let directory = PublicationDirectory::new();
        let identifier = target::ApplicationIdentifier::new(b"org.omega.window-test").unwrap();
        let executable = signed_executable(identifier.as_str());
        let digest = executable_container_digest(&executable);
        let observers = if stage {
            PublicationObservers {
                after_staging: disturb,
                after_installation: &|_| {},
            }
        } else {
            PublicationObservers {
                after_staging: &|_| {},
                after_installation: disturb,
            }
        };
        let result = publish_package_with_observers(
            &directory.0,
            "window_test",
            &identifier,
            &executable,
            digest,
            observers,
        );
        assert!(result.is_err(), "a disturbed package tree must reject");
        assert_eq!(
            fs::read_dir(&directory.0).unwrap().count(),
            0,
            "a refused package leaves no staged or installed tree"
        );
    }

    #[test]
    fn package_publication_rejects_an_extra_empty_directory_in_the_staged_tree() {
        for extra in ["Contents/Resources", "Extra", "Contents/MacOS/Extra"] {
            publication_rejects_disturbed_tree(true, &|root| {
                fs::create_dir_all(root.join(extra)).unwrap();
            });
        }
    }

    #[test]
    fn package_publication_rejects_an_extra_empty_directory_in_the_installed_tree() {
        for extra in ["Contents/Resources", "Extra", "Contents/MacOS/Extra"] {
            publication_rejects_disturbed_tree(false, &|root| {
                fs::create_dir_all(root.join(extra)).unwrap();
            });
        }
    }

    #[test]
    fn package_publication_rejects_an_extra_file_in_either_tree() {
        for stage in [true, false] {
            publication_rejects_disturbed_tree(stage, &|root| {
                fs::write(root.join("Contents/PkgInfo"), b"APPL????").unwrap();
            });
        }
    }

    #[cfg(unix)]
    #[test]
    fn package_publication_rejects_an_extra_symlink_in_either_tree() {
        for stage in [true, false] {
            publication_rejects_disturbed_tree(stage, &|root| {
                std::os::unix::fs::symlink("MacOS", root.join("Contents/Resources")).unwrap();
            });
        }
    }
}
