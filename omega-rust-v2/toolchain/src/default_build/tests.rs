use super::*;
use std::{fs, path::PathBuf};

#[test]
fn canonical_anonymous_record_matches_build_reference_and_commits_semantic_inputs() {
    let node = Node::File {
        bytes: Arc::from(b"public data Token {}".as_slice()),
        executable: false,
    };
    let tree = Snapshot::from([(PathBuf::from("library.omg"), node)]);
    let key = anonymous_owner(&tree, DEFAULT_SEMANTICS, &[[1; 32], [2; 32]]).unwrap();
    // Cross-implementation vector from Build's source/anonymous_owner.rs.
    assert_eq!(
        key,
        [
            53, 179, 79, 52, 161, 231, 70, 176, 53, 17, 97, 63, 133, 76, 192, 249, 53, 186, 154, 205, 196, 211, 104, 141, 69, 99, 254, 91, 79, 148, 35, 9
        ]
    );
    assert_eq!(key, anonymous_owner(&tree, DEFAULT_SEMANTICS, &[[2; 32], [1; 32], [2; 32]]).unwrap());
    assert_ne!(key, anonymous_owner(&tree, b"changed semantics", &[[1; 32], [2; 32]]).unwrap());
    let mut files = PRODUCT_CORE.to_vec();
    files.push(DEFAULT_ENTRY);
    let bundled = default_core().unwrap();
    files.reverse();
    assert_eq!(bundled, core_snapshot(files.iter().copied()).unwrap());
    files[1].1 = "// changed admitted declaration";
    let changed = core_snapshot(files.iter().copied()).unwrap();
    assert_ne!(
        anonymous_owner(&tree, DEFAULT_SEMANTICS, &[bundled]).unwrap(),
        anonymous_owner(&tree, DEFAULT_SEMANTICS, &[changed]).unwrap()
    );
    assert!(check_empty_default("machine build(activation: @mutable Build) {}").is_ok());
    for entry in [
        "machine build(builder: @mutable Build) { dependency(); }",
        "machine build(builder: @Build) {}",
        "machine build(builder: @mutable UserBuild) {}",
        "machine Owner::build(builder: @mutable Build) {}",
    ] {
        assert!(check_empty_default(entry).is_err(), "{entry}");
    }
}

#[test]
fn capture_survives_relocation_and_reads_only_retained_bytes() {
    let path = std::env::temp_dir().join(format!("omega-default-build-{}", std::process::id()));
    fs::create_dir(&path).unwrap();
    let a = path.join("original");
    let b = path.join("relocated");
    fs::create_dir(&a).unwrap();
    fs::create_dir(a.join("empty")).unwrap();
    fs::write(a.join("input.omg"), "machine entry() {}").unwrap();
    fs::write(a.join("data.bin"), [0, 255, 15]).unwrap();
    let original = DefaultBuild::capture(&a).unwrap();
    fs::rename(&a, &b).unwrap();
    for ignored in ["build", ".git", "empty/.git"] {
        fs::create_dir(b.join(ignored)).unwrap();
        fs::write(b.join(ignored).join("mutated"), [42]).unwrap();
    }
    fs::write(b.join("OmEgA.LoCk"), "lock").unwrap();
    assert_eq!(original.package_key(), DefaultBuild::capture(&b).unwrap().package_key());
    fs::write(b.join("data.bin"), [0, 255, 14]).unwrap();
    assert_ne!(original.package_key(), DefaultBuild::capture(&b).unwrap().package_key());
    fs::write(b.join("data.bin"), [0, 255, 15]).unwrap();
    fs::rename(b.join("data.bin"), b.join("renamed.bin")).unwrap();
    assert_ne!(original.package_key(), DefaultBuild::capture(&b).unwrap().package_key());
    fs::rename(b.join("renamed.bin"), b.join("data.bin")).unwrap();
    fs::create_dir(b.join("empty/build")).unwrap();
    assert_ne!(original.package_key(), DefaultBuild::capture(&b).unwrap().package_key());
    fs::write(b.join("input.omg"), "machine changed() {}").unwrap();
    assert_eq!(&*original.source_bytes(Path::new("input.omg")).unwrap(), b"machine entry() {}");
    fs::write(b.join("build.omg"), "machine build() {}").unwrap();
    assert!(
        DefaultBuild::capture(&b)
            .err()
            .unwrap()
            .to_string()
            .contains("authored build.omg")
    );
    fs::remove_dir_all(path).unwrap();
}

#[test]
fn captured_links_cannot_read_host_or_excluded_paths() {
    let mut tree = Snapshot::from([
        (PathBuf::from("dir"), Node::Directory),
        (
            PathBuf::from("dir/file"),
            Node::File {
                bytes: Arc::from(b"payload".as_slice()),
                executable: false,
            },
        ),
        (PathBuf::from("dir/alias"), Node::Link { target: "../dir/file".into() }),
    ]);
    assert_eq!(capture::resolve(&tree, Path::new("dir/alias")).unwrap(), Path::new("dir/file"));
    for target in [
        "../../outside",
        "../.git/config",
        "../build/private",
        "alias",
        "file/../file",
    ] {
        tree.insert("dir/alias".into(), Node::Link { target: target.into() });
        assert!(capture::resolve(&tree, Path::new("dir/alias")).is_err(), "{target}");
    }
}
