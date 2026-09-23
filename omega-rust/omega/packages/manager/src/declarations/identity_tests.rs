use super::{AliasName, PackageKey, PackageName};
use package_source::{
    SourceLineage, SourceRelativePath, WorkspaceLineageIdentity, WorkspaceMemberLineage,
};

fn package_name() -> PackageName {
    PackageName::parse("arithmetic_kernels").unwrap()
}

fn lineage(locator: &str) -> SourceLineage {
    SourceLineage::git(locator).unwrap()
}

#[test]
fn package_names_require_canonical_snake_case_and_reject_spoofs() {
    for valid in ["arithmetic_kernels", "sha256", "codec_2"] {
        assert_eq!(PackageName::parse(valid).unwrap().as_str(), valid);
    }
    for invalid in [
        "",
        "Arithmetic-kernels",
        "arithmetic-kernels",
        "_arithmetic",
        "arithmetic_",
        "arithmetic__kernels",
        "arithmetic.kernels",
        "123-tools",
        "arithmetіc-kernels",
    ] {
        assert!(PackageName::parse(invalid).is_err(), "accepted {invalid:?}");
    }
}

#[test]
fn aliases_require_canonical_snake_case_identifiers() {
    for valid in ["arithmetic_kernels", "sha256", "codec_2"] {
        assert_eq!(AliasName::parse(valid).unwrap().as_str(), valid);
    }
    for invalid in [
        "",
        "Arithmetic_kernels",
        "arithmetic-kernels",
        "_arithmetic",
        "arithmetic_",
        "arithmetic__kernels",
        "123_tools",
        "arithmetіc_kernels",
    ] {
        assert!(AliasName::parse(invalid).is_err(), "accepted {invalid:?}");
    }
    assert_eq!(
        package_name().default_alias().as_str(),
        "arithmetic_kernels"
    );
}

#[test]
fn source_or_name_change_replaces_a_package_key() {
    let original = PackageKey::new(
        package_name(),
        lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
    );
    let transport_equivalent = PackageKey::new(
        package_name(),
        lineage("git@github.com:cathedralos/arithmetic-kernels"),
    );
    let other_source = PackageKey::new(
        package_name(),
        lineage("https://github.com/Other/arithmetic-kernels.git"),
    );
    let other_name = PackageKey::new(
        PackageName::parse("arithmetic_core").unwrap(),
        lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
    );

    assert_eq!(original, transport_equivalent);
    assert_ne!(original, other_source);
    assert_ne!(original, other_name);
}

#[test]
fn package_key_identity_uses_canonical_name_and_source_lineage() {
    let https = PackageKey::new(
        package_name(),
        lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
    );
    let ssh = PackageKey::new(
        package_name(),
        lineage("git@github.com:cathedralos/arithmetic-kernels"),
    );
    let other_name = PackageKey::new(
        PackageName::parse("arithmetic_core").unwrap(),
        lineage("https://github.com/CathedralOS/arithmetic-kernels.git"),
    );
    let other_lineage = PackageKey::new(
        package_name(),
        lineage("https://github.com/Other/arithmetic-kernels.git"),
    );

    assert_eq!(https.identity(), ssh.identity());
    assert_ne!(https.identity(), other_name.identity());
    assert_ne!(https.identity(), other_lineage.identity());
    assert_eq!(
        https.identity().digest(),
        [
            0xa2, 0xe8, 0x18, 0x6f, 0xbe, 0x29, 0x5a, 0x79, 0xd4, 0xe7, 0x88, 0x77, 0xa8, 0x58,
            0x7b, 0x06, 0xb4, 0x1c, 0x4c, 0x53, 0xb9, 0x45, 0x95, 0x2d, 0x49, 0x7f, 0x1f, 0xa6,
            0x19, 0xf4, 0x19, 0x0e,
        ]
    );
}

#[test]
fn workspace_package_key_identity_preserves_its_canonical_encoding() {
    let root = lineage("https://github.com/CathedralOS/workspace.git");
    let workspace = WorkspaceLineageIdentity::from_root_source(&root).unwrap();
    let member = SourceLineage::Workspace(WorkspaceMemberLineage::new(
        workspace,
        SourceRelativePath::parse("packages/arithmetic-kernels").unwrap(),
    ));
    let key = PackageKey::new(package_name(), member);

    assert_eq!(
        key.identity().digest(),
        [
            0x9f, 0x55, 0x50, 0xd9, 0x81, 0x35, 0x4a, 0xfa, 0x6c, 0x24, 0x54, 0x66, 0x3b, 0xe1,
            0x54, 0x3c, 0x5e, 0x7d, 0x0d, 0x40, 0xfc, 0xb3, 0x9d, 0x70, 0x92, 0x80, 0x48, 0xa3,
            0x96, 0xbc, 0x34, 0xdd,
        ]
    );
}
