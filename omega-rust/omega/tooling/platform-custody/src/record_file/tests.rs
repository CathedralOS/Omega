use super::*;
use cap_std::{ambient_authority, fs::Dir};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn retained_read_detects_in_place_content_change() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    let directory_path = std::env::temp_dir().join(format!(
        "omega-root-record-content-change-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&directory_path).expect("create policy directory");
    let file_path = directory_path.join("policy.record");
    fs::write(&file_path, b"first").expect("write initial record");
    let directory = Dir::open_ambient_dir(&directory_path, ambient_authority())
        .expect("open policy directory capability");
    let root = RecordFileRoot::from_directory(directory, directory_path.clone())
        .expect("bind policy directory capability");
    let limits = RecordFileLimits { maximum_bytes: 5 };
    let mut read = root
        .read(Path::new("policy.record"), limits)
        .expect("read initial record");

    fs::write(&file_path, b"other").expect("replace bytes in the same file");

    assert!(matches!(
        read.verify_current(limits),
        Err(RecordFileError::ContentsChanged { .. })
    ));
    let _ = fs::remove_dir_all(directory_path);
}

#[test]
fn rooted_replacement_publishes_exact_synchronized_bytes_without_a_stage_residue() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    let directory_path = std::env::temp_dir().join(format!(
        "omega-root-record-replacement-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&directory_path).expect("create record directory");
    fs::write(directory_path.join("config"), b"mutable helper bytes")
        .expect("write existing control file");
    let directory = Dir::open_ambient_dir(&directory_path, ambient_authority())
        .expect("open record directory capability");
    let root = RecordFileRoot::from_directory(directory, directory_path.clone())
        .expect("bind record directory capability");

    root.replace_existing(
        Path::new("config"),
        b"canonical bytes",
        RecordFileLimits { maximum_bytes: 64 },
    )
    .expect("replace exact control file");

    assert_eq!(
        fs::read(directory_path.join("config")).expect("read replacement"),
        b"canonical bytes"
    );
    assert!(
        fs::read_dir(&directory_path)
            .expect("list record directory")
            .all(|entry| !entry
                .expect("record entry")
                .file_name()
                .to_string_lossy()
                .starts_with(".omega-record-stage-"))
    );
    let _ = fs::remove_dir_all(directory_path);
}

#[cfg(unix)]
#[test]
fn rooted_replacement_rejects_a_symlink_destination_without_touching_its_target() {
    use std::os::unix::fs::symlink;

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should follow the Unix epoch")
        .as_nanos();
    let directory_path = std::env::temp_dir().join(format!(
        "omega-root-record-replacement-link-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir_all(&directory_path).expect("create record directory");
    let target = directory_path.join("target");
    fs::write(&target, b"outside replacement").expect("write symlink target");
    symlink("target", directory_path.join("config")).expect("create destination symlink");
    let directory = Dir::open_ambient_dir(&directory_path, ambient_authority())
        .expect("open record directory capability");
    let root = RecordFileRoot::from_directory(directory, directory_path.clone())
        .expect("bind record directory capability");

    assert!(matches!(
        root.replace_existing(
            Path::new("config"),
            b"canonical bytes",
            RecordFileLimits { maximum_bytes: 64 },
        ),
        Err(RecordFileError::NotRegularFile { .. })
    ));
    assert_eq!(
        fs::read(target).expect("read symlink target"),
        b"outside replacement"
    );
    let _ = fs::remove_dir_all(directory_path);
}
