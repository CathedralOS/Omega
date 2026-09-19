//! Tests for capturing, committing, materializing and verifying staged
//! output trees.

use crate::portable_paths::validate_portable_component;
use crate::staged_output_tree::RetainedStagedOutputEntryKind;
use crate::{capture, empty, select_included_sources};
use checked_interpreter::FilesystemSponsor;
use sha2::Digest;
use sha2::Sha256;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    session: PathBuf,
    root: PathBuf,
    sponsor: FilesystemSponsor,
}

impl Fixture {
    fn new(label: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let session = std::env::temp_dir().join(format!(
            "omega-staged-output-{label}-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&session);
        std::fs::create_dir(&session).unwrap();
        let session = std::fs::canonicalize(session).unwrap();
        let sponsor = FilesystemSponsor::new(&session).unwrap();
        let root = session.join("output");
        let fixture = Self {
            session,
            root,
            sponsor,
        };
        fixture.create_directory(Path::new(""));
        fixture
    }

    fn bind(&self, relative: &Path) -> checked_interpreter::FilesystemSponsorPath {
        self.sponsor.bind_path(self.root.join(relative)).unwrap()
    }

    fn create_directory(&self, relative: &Path) {
        let path = self.root.join(relative);
        let prepared = self
            .sponsor
            .prepare_create_directory(&self.bind(relative))
            .unwrap();
        std::fs::create_dir(&path).unwrap();
        prepared.commit().unwrap();
    }

    fn create_file(&self, relative: &Path, bytes: &[u8]) {
        let prepared = self
            .sponsor
            .prepare_create_object(&self.bind(relative), bytes.len() as u64)
            .unwrap();
        std::fs::write(self.root.join(relative), bytes).unwrap();
        prepared.commit().unwrap();
    }

    fn empty_destination(&self, label: &str) -> PathBuf {
        let destination = self.session.join(label);
        std::fs::create_dir(&destination).unwrap();
        destination
    }

    #[cfg(unix)]
    fn create_symlink(&self, relative: &Path, target: &str) {
        use std::os::unix::fs::symlink;

        let prepared = self
            .sponsor
            .prepare_create_symlink(&self.bind(relative), target.as_bytes())
            .unwrap();
        symlink(target, self.root.join(relative)).unwrap();
        prepared.commit().unwrap();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.session);
    }
}

fn populated_fixture(label: &str, bytes: &[u8]) -> Fixture {
    let fixture = Fixture::new(label);
    fixture.create_directory(Path::new("nested"));
    fixture.create_file(Path::new("nested/artifact.bin"), bytes);
    fixture.create_directory(Path::new("empty"));
    fixture
}

#[test]
fn commitment_is_relocation_stable_and_binds_paths_kinds_and_bytes() {
    let first = populated_fixture("first", b"payload");
    let relocated = populated_fixture("relocated", b"payload");
    let changed = populated_fixture("changed", b"changed");
    let first_commitment = capture(&first.root, &first.sponsor).unwrap();
    let relocated_commitment = capture(&relocated.root, &relocated.sponsor).unwrap();
    let changed_commitment = capture(&changed.root, &changed.sponsor).unwrap();
    assert_eq!(first_commitment, relocated_commitment);
    assert_eq!(first_commitment.entry_count(), 3);
    assert_eq!(first_commitment.file_bytes(), 7);
    assert_ne!(first_commitment.digest(), changed_commitment.digest());
}

#[test]
fn included_sources_require_captured_regular_non_executable_omega_files() {
    let fixture = Fixture::new("included-source");
    fixture.create_file(Path::new("generated.omg"), b"data Generated {}\n");
    fixture.create_file(Path::new("artifact.bin"), b"bytes");
    fixture.create_directory(Path::new("directory.omg"));
    let retained = capture(&fixture.root, &fixture.sponsor).unwrap();

    assert!(
        select_included_sources(&retained, &[]).is_err(),
        "an Omega-looking output does not become source implicitly"
    );

    let selected = select_included_sources(&retained, &[b"generated.omg".to_vec()]).unwrap();
    let [selected] = selected.as_slice() else {
        panic!("one explicit handoff retains one source")
    };
    assert_eq!(selected.relative_path(), b"generated.omg");
    assert_eq!(selected.bytes(), b"data Generated {}\n");
    assert_eq!(
        selected.digest(),
        <[u8; 32]>::from(Sha256::digest(b"data Generated {}\n"))
    );

    for rejected in ["missing.omg", "artifact.bin", "directory.omg"] {
        assert!(
            select_included_sources(&retained, &[rejected.as_bytes().to_vec()]).is_err(),
            "{rejected} must not enter generated-source custody"
        );
    }
}

#[test]
fn included_sources_reject_reserved_discovery_filenames() {
    for relative in ["build.omg", "nested/main.omg"] {
        let fixture = Fixture::new("reserved-source-name");
        if relative.contains('/') {
            fixture.create_directory(Path::new("nested"));
        }
        fixture.create_file(Path::new(relative), b"data Generated {}\n");
        let retained = capture(&fixture.root, &fixture.sponsor).unwrap();
        let diagnostics = select_included_sources(&retained, &[relative.as_bytes().to_vec()])
            .expect_err("generated source must not impersonate discovery roots");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("reserved source-discovery filename")),
            "{relative}: {diagnostics:#?}"
        );
    }
}

#[cfg(unix)]
#[test]
fn commitment_normalizes_hard_links_and_binds_executable_mode() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("hard-links");
    fixture.create_file(Path::new("first"), b"payload");
    let first = fixture.bind(Path::new("first"));
    let second = fixture.bind(Path::new("second"));
    let prepared = fixture.sponsor.prepare_hard_link(&first, &second).unwrap();
    std::fs::hard_link(fixture.root.join("first"), fixture.root.join("second")).unwrap();
    prepared.commit().unwrap();

    let ordinary = capture(&fixture.root, &fixture.sponsor).unwrap();
    assert_eq!(ordinary.entry_count(), 2);
    assert_eq!(ordinary.file_bytes(), 7);

    let duplicated_fixture = Fixture::new("duplicate-files");
    duplicated_fixture.create_file(Path::new("first"), b"payload");
    duplicated_fixture.create_file(Path::new("second"), b"payload");
    let duplicated = capture(&duplicated_fixture.root, &duplicated_fixture.sponsor).unwrap();
    assert_eq!(
        ordinary.commitment(),
        duplicated.commitment(),
        "canonical staged-output identity must not reveal hard-link topology"
    );

    let mut permissions = std::fs::metadata(fixture.root.join("first"))
        .unwrap()
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(fixture.root.join("first"), permissions).unwrap();
    let executable = capture(&fixture.root, &fixture.sponsor).unwrap();
    assert_ne!(ordinary.digest(), executable.digest());
    assert_eq!(executable.file_bytes(), 7);

    let destination = fixture.empty_destination("hard-link-materialization");
    assert_eq!(
        executable.materialize_into(&destination).unwrap(),
        executable.commitment()
    );
    assert_eq!(
        std::fs::read(destination.join("first")).unwrap(),
        b"payload"
    );
    assert_eq!(
        std::fs::read(destination.join("second")).unwrap(),
        b"payload"
    );
}

#[cfg(unix)]
#[test]
fn retained_tree_materializes_files_empty_directories_and_relative_symlinks() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("materialize");
    fixture.create_directory(Path::new("bin"));
    fixture.create_directory(Path::new("empty"));
    fixture.create_file(Path::new("ordinary.txt"), b"ordinary\0bytes");
    fixture.create_file(Path::new("bin/tool"), b"#!/omega\n");
    let mut permissions = std::fs::metadata(fixture.root.join("bin/tool"))
        .unwrap()
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(fixture.root.join("bin/tool"), permissions).unwrap();
    fixture.create_symlink(Path::new("bin/ordinary-link"), "../ordinary.txt");

    let retained = capture(&fixture.root, &fixture.sponsor).unwrap();
    let destination = fixture.empty_destination("materialized");
    let materialized_commitment = retained.materialize_into(&destination).unwrap();

    assert_eq!(materialized_commitment, retained.commitment());
    assert_eq!(
        std::fs::read(destination.join("ordinary.txt")).unwrap(),
        b"ordinary\0bytes"
    );
    assert_eq!(
        std::fs::read(destination.join("bin/tool")).unwrap(),
        b"#!/omega\n"
    );
    assert!(destination.join("empty").is_dir());
    assert_eq!(
        std::fs::read_link(destination.join("bin/ordinary-link")).unwrap(),
        PathBuf::from("../ordinary.txt")
    );
    assert_eq!(
        std::fs::metadata(destination.join("ordinary.txt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o644
    );
    assert_eq!(
        std::fs::metadata(destination.join("bin/tool"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o755
    );
}

#[test]
fn retained_empty_tree_materializes_with_its_explicit_commitment() {
    let fixture = Fixture::new("empty-materialize");
    let retained = empty();
    let destination = fixture.empty_destination("materialized-empty");

    assert_eq!(
        retained.materialize_into(&destination).unwrap(),
        retained.commitment()
    );
    assert_eq!(retained.entry_count(), 0);
    assert_eq!(retained.file_bytes(), 0);
    assert!(std::fs::read_dir(destination).unwrap().next().is_none());
}

#[test]
fn materialization_rejects_nonempty_destination_without_overwriting_it() {
    let fixture = populated_fixture("nonempty-destination", b"payload");
    let retained = capture(&fixture.root, &fixture.sponsor).unwrap();
    let destination = fixture.empty_destination("nonempty");
    std::fs::write(destination.join("sentinel"), b"owned by caller").unwrap();

    let error = retained.materialize_into(&destination).unwrap_err();
    assert!(error.message().contains("must be empty"));
    assert_eq!(
        std::fs::read(destination.join("sentinel")).unwrap(),
        b"owned by caller"
    );
}

#[cfg(unix)]
#[test]
fn materialization_rejects_symlink_destination() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new("symlink-destination");
    let retained = empty();
    let concrete = fixture.empty_destination("concrete");
    let destination = fixture.session.join("destination-link");
    symlink(&concrete, &destination).unwrap();

    let error = retained.materialize_into(&destination).unwrap_err();
    assert!(error.message().contains("existing concrete directory"));
}

#[test]
fn materialization_rejects_tampered_content_and_invalid_retained_shape() {
    let fixture = populated_fixture("tamper", b"payload");
    let retained = capture(&fixture.root, &fixture.sponsor).unwrap();

    let mut invalid_shape = retained.clone();
    invalid_shape.entries[0].relative_path = b"../escape".to_vec();
    let shape_destination = fixture.empty_destination("invalid-shape");
    assert!(invalid_shape.materialize_into(&shape_destination).is_err());
    assert!(
        std::fs::read_dir(&shape_destination)
            .unwrap()
            .next()
            .is_none(),
        "shape validation must reject before materialization"
    );

    let mut tampered_content = retained;
    let file = tampered_content
        .entries
        .iter_mut()
        .find_map(|entry| match &mut entry.kind {
            RetainedStagedOutputEntryKind::File { bytes, .. } => Some(bytes),
            _ => None,
        })
        .expect("fixture has one retained file");
    *file = Arc::from(b"tampered".as_slice());
    let content_destination = fixture.empty_destination("tampered-content");
    assert!(
        tampered_content
            .materialize_into(&content_destination)
            .is_err()
    );
    assert!(
        std::fs::read_dir(&content_destination)
            .unwrap()
            .next()
            .is_none(),
        "commitment validation must reject before materialization"
    );
}

#[test]
fn rejects_portability_collisions() {
    assert!(validate_portable_component(b"NUL.txt", Path::new("NUL.txt")).is_err());
    assert!(validate_portable_component(b"COM1", Path::new("COM1")).is_err());
    assert!(validate_portable_component(b"trailing.", Path::new("trailing.")).is_err());
}

#[cfg(unix)]
#[test]
fn rejects_external_symlink_targets_and_unsponsored_entries() {
    use std::os::unix::fs::symlink;

    let symlink_fixture = Fixture::new("symlink");
    let prepared = symlink_fixture
        .sponsor
        .prepare_create_symlink(&symlink_fixture.bind(Path::new("link")), b"/outside")
        .unwrap();
    symlink("/outside", symlink_fixture.root.join("link")).unwrap();
    prepared.commit().unwrap();
    assert!(capture(&symlink_fixture.root, &symlink_fixture.sponsor).is_err());

    let unsponsored_fixture = Fixture::new("unsponsored");
    std::fs::write(unsponsored_fixture.root.join("extra"), b"bytes").unwrap();
    assert!(capture(&unsponsored_fixture.root, &unsponsored_fixture.sponsor).is_err());
}
