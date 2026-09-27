use super::{PackageCommandKind, rejects};

#[test]
fn every_package_command_rejects_duplicate_offline() {
    for arguments in [
        vec!["source", "--offline", "--offline"],
        vec!["--offline", "--resume", "--offline"],
        vec!["--offline", "--discard-review", "--offline"],
    ] {
        for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
            rejects(kind, &arguments, "duplicate --offline");
        }
    }
}
