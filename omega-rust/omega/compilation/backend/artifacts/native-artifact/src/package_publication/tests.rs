use super::*;
use crate::publication::executable_container_digest;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct PublicationDirectory(PathBuf);

impl PublicationDirectory {
    fn new() -> Self {
        let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "omega-package-publication-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&path).expect("create package publication test directory");
        Self(path)
    }
}

impl Drop for PublicationDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
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
    image.symbol_table.entry_symbol = image.symbol_table.symbols.insert(image::FinalImageSymbol {
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
        std::fs::read(first.inner_executable_path().unwrap()).unwrap(),
        executable
    );
    let plist_path = first.package_root().join("Contents/Info.plist");
    let first_plist = std::fs::read(&plist_path).unwrap();
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
            std::fs::metadata(first.inner_executable_path().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
    assert_eq!(publish(), first);
    assert_eq!(std::fs::read(plist_path).unwrap(), first_plist);
    let mut changed = first.clone();
    changed.application_identifier = target::ApplicationIdentifier::new(b"org.other").unwrap();
    let mut changed = first.clone();
    changed.components.reverse();
    let mut changed = first;
    changed.package_root = directory.0.join("different.app");
}

#[test]
fn invalid_identity_paths_and_staging_leave_the_previous_package_untouched() {
    let directory = PublicationDirectory::new();
    let identifier = target::ApplicationIdentifier::new(b"org.omega.window-test").unwrap();
    let executable = signed_executable(identifier.as_str());
    let digest = executable_container_digest(&executable);
    let previous = directory.0.join("window_test.app");
    std::fs::create_dir(&previous).unwrap();
    std::fs::write(previous.join("previous"), b"retained").unwrap();
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
    assert_eq!(
        std::fs::read(previous.join("previous")).unwrap(),
        b"retained"
    );
    assert_eq!(
        std::fs::read_dir(&directory.0).unwrap().count(),
        1,
        "failed staging leaves no temporary package"
    );
}
