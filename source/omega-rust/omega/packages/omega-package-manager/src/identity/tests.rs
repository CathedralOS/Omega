use super::{AliasName, PackageKey, PackageName};
use omega_package_source::SourceLineage;

fn package_name() -> PackageName {
    PackageName::parse("arithmetic-kernels").unwrap()
}

fn lineage(locator: &str) -> SourceLineage {
    SourceLineage::git(locator).unwrap()
}

#[test]
fn package_names_require_canonical_kebab_case_and_reject_spoofs() {
    for valid in ["arithmetic-kernels", "sha256", "codec-2"] {
        assert_eq!(PackageName::parse(valid).unwrap().as_str(), valid);
    }
    for invalid in [
        "",
        "Arithmetic-kernels",
        "arithmetic_kernels",
        "-arithmetic",
        "arithmetic-",
        "arithmetic--kernels",
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
        PackageName::parse("arithmetic-core").unwrap(),
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
        PackageName::parse("arithmetic-core").unwrap(),
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
            0x8a, 0xbb, 0x4a, 0x34, 0x3b, 0xf9, 0x0f, 0xa2, 0x95, 0x8e, 0x85, 0x8b, 0x18, 0xb0,
            0x30, 0x66, 0xa1, 0xb7, 0x4d, 0xa2, 0x95, 0x20, 0xd5, 0x8a, 0x7e, 0xed, 0x84, 0x06,
            0xa3, 0xe9, 0x63, 0xe5,
        ]
    );
}
