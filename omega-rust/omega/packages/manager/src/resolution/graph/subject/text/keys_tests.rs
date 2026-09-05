use super::*;
use crate::declarations::PackageName;
use omega_package_source::{
    ExternalLocalLineage, ExternalSourceContext, SourceRelativePath, WorkspaceLineageIdentity,
    WorkspaceMemberLineage,
};

fn key(lineage: SourceLineage) -> PackageKey {
    PackageKey::new(PackageName::parse("removed-package").unwrap(), lineage)
}

fn keys() -> Vec<PackageKey> {
    let mut keys = [
        "https://github.com/Owner/Library.git",
        "https://gitlab.com/group/subgroup/library.git",
        "https://packages.example:8443/team/library.git",
        "ssh://git@packages.example:2222/team/library.git",
        "git@packages.example:team/library.git",
    ]
    .into_iter()
    .map(|locator| key(SourceLineage::git(locator).unwrap()))
    .collect::<Vec<_>>();
    keys.push(key(SourceLineage::Workspace(WorkspaceMemberLineage::new(
        WorkspaceLineageIdentity::parse_hex(&"11".repeat(32)).unwrap(),
        SourceRelativePath::parse("members/library").unwrap(),
    ))));
    let path = if cfg!(windows) {
        "C:\\unavailable\\package"
    } else {
        "/unavailable/package"
    };
    keys.push(key(SourceLineage::ExternalLocal(
        ExternalLocalLineage::from_recovered_canonical_path(
            path.to_owned(),
            ExternalSourceContext::parse_hex(&"22".repeat(32)).unwrap(),
        )
        .unwrap(),
    )));
    keys
}

#[test]
fn every_lineage_roundtrips_without_source_custody_or_an_old_graph() {
    for key in keys() {
        let (text, write_usage) =
            write_package_key_text(&key, Limits::default(), usize::MAX).unwrap();
        assert!(text.starts_with("name \"removed-package\"\nlineage "));
        let (recovered, recovery_usage) =
            recover_package_key_text(&text, Limits::default(), usize::MAX).unwrap();
        assert_eq!(key, recovered);
        assert!(recovery_usage > 0);
        assert!(write_usage > recovery_usage);
        assert_eq!(
            write_package_key_text(&key, Limits::default(), write_usage).unwrap(),
            (text.clone(), write_usage)
        );
        assert!(
            write_package_key_text(&key, Limits::default(), write_usage - 1)
                .unwrap_err()
                .is_allocation_limit_exceeded()
        );
        assert_eq!(
            recover_package_key_text(&text, Limits::default(), recovery_usage).unwrap(),
            (key, recovery_usage)
        );
        assert!(
            recover_package_key_text(&text, Limits::default(), recovery_usage - 1)
                .unwrap_err()
                .is_allocation_limit_exceeded()
        );
    }
}

#[test]
fn text_and_identity_limits_apply_before_unbounded_fragment_construction() {
    let key = keys().remove(0);
    let (text, _) = write_package_key_text(&key, Limits::default(), usize::MAX).unwrap();
    let limits = Limits {
        maximum_record_bytes: text.len(),
        ..Limits::default()
    };
    assert!(write_package_key_text(&key, limits, usize::MAX).is_ok());
    assert!(recover_package_key_text(&text, limits, usize::MAX).is_ok());
    for limits in [
        Limits {
            maximum_record_bytes: text.len() - 1,
            ..limits
        },
        Limits {
            maximum_identity_bytes: 0,
            ..limits
        },
    ] {
        assert!(write_package_key_text(&key, limits, usize::MAX).is_err());
        assert!(recover_package_key_text(&text, limits, usize::MAX).is_err());
    }
}

#[test]
fn alternate_framing_and_noncanonical_key_spellings_cannot_hide_in_history() {
    let key = keys().remove(0);
    let (text, _) = write_package_key_text(&key, Limits::default(), usize::MAX).unwrap();
    for changed in [
        text.replace("name ", "name  "),
        text.replace('\n', "\r\n"),
        text.replace("removed-package", "\\x72emoved-package"),
        format!("{text}\n"),
        format!("{text}end\n"),
        text.trim_end().to_owned(),
    ] {
        assert!(recover_package_key_text(&changed, Limits::default(), usize::MAX).is_err());
    }
    for end in 0..text.len() {
        assert!(recover_package_key_text(&text[..end], Limits::default(), usize::MAX).is_err());
    }
}
