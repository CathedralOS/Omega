use super::{TempTree, identity, seal_source_tree, unseal_source_tree};
use crate::package_compilation::capture_package_source_input;
use crate::package_compilation::package_compilation::PackageSourceBinding;
use std::fs;

#[test]
fn captured_package_source_input_retains_every_byte_and_matches_its_index() {
    let tree = TempTree::new();
    let root = tree.package("snapshot-root");
    fs::write(root.join("main.omg"), "data Main { value: u8; }\n").expect("write main");
    let templates = root.join("templates");
    fs::create_dir(&templates).expect("create templates");
    fs::write(templates.join("banner.tmpl"), b"HELLO {{name}}\n").expect("write template");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let tool = templates.join("run.sh");
        fs::write(&tool, b"run\n").expect("write tool");
        fs::set_permissions(&tool, fs::Permissions::from_mode(0o555)).expect("seal tool");
        std::os::unix::fs::symlink("templates", root.join("link")).expect("create link");
    }
    seal_source_tree(&root);

    let captured = capture_package_source_input(&root).expect("capture package source input");
    let binding = PackageSourceBinding::new(identity(1), "root", root.clone())
        .with_canonical_source_metadata()
        .expect("binding capture");
    assert_eq!(
        captured.canonical_source_metadata(),
        binding.canonical_source_metadata().expect("binding index"),
        "the retained inventory and the validated binding share one capture authority"
    );
    assert_eq!(
        captured.entry_count(),
        captured.canonical_source_metadata().rows().len() as u64
    );
    let template = captured
        .file(b"templates/banner.tmpl")
        .expect("captured template bytes");
    assert_eq!(template.bytes(), b"HELLO {{name}}\n");
    assert!(!template.executable());
    #[cfg(unix)]
    {
        assert!(
            captured
                .file(b"templates/run.sh")
                .expect("captured tool")
                .executable()
        );
        assert!(
            captured.file(b"link").is_none(),
            "a captured link stays inert evidence"
        );
        let kinds = captured
            .entries()
            .map(|entry| (entry.relative_path().to_vec(), entry.kind()))
            .collect::<Vec<_>>();
        assert!(kinds.iter().any(|(path, kind)| {
            path == b"link"
                && matches!(
                    kind,
                    crate::build_output::CapturedSourceEntryKind::Symlink { target }
                        if *target == b"templates"
                )
        }));
    }
    unseal_source_tree(&root);
}

#[test]
fn captured_package_source_input_is_immune_to_post_capture_mutation() {
    let tree = TempTree::new();
    let root = tree.package("mutating-root");
    let template = root.join("template.tmpl");
    fs::write(&template, b"captured").expect("write template");
    seal_source_tree(&root);

    let captured = capture_package_source_input(&root).expect("capture before mutation");
    unseal_source_tree(&root);
    fs::write(&template, b"mutated-live-bytes").expect("mutate the live host file");

    let retained = captured.file(b"template.tmpl").expect("captured file");
    assert_eq!(
        retained.bytes(),
        b"captured",
        "captured inventory bytes are immutable after the host path changes"
    );
    unseal_source_tree(&root);
}
