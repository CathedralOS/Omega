mod fixture;

use super::*;
use fixture::{pending, source};

#[test]
fn canonical_round_trip_preserves_both_kinds_and_lock_presence() {
    for kind in [PackageCommandKind::Install, PackageCommandKind::Update] {
        for before_lock in [None, Some([0xef; 32])] {
            let mut proposal = pending();
            proposal.kind = kind;
            proposal.before_lock = before_lock;
            let text = proposal.encode().unwrap();
            let recovered = PendingPackageChange::recover(&text).unwrap();
            assert_eq!(recovered.encode().unwrap(), text);
            assert_eq!(recovered.before_build, proposal.before_build);
            assert_eq!(recovered.before_lock, proposal.before_lock);
            assert_eq!(recovered.original_content, proposal.original_content);
            assert_eq!(recovered.proposed_build, proposal.proposed_build);
            assert_eq!(recovered.source, proposal.source);
            assert_eq!(recovered.targets, proposal.targets);
            assert!(matches!(
                (&recovered.kind, &proposal.kind),
                (PackageCommandKind::Install, PackageCommandKind::Install)
                    | (PackageCommandKind::Update, PackageCommandKind::Update)
            ));
            assert!(text.starts_with("omega-package-proposal 1\nkind "));
            let source_text = proposal.source.canonical_text(Default::default()).unwrap();
            assert!(text.ends_with(&format!(
                "source {}\n{source_text}\nend\n",
                source_text.len()
            )));
        }
    }
}

#[test]
fn build_payload_is_inert_utf8_with_exact_byte_framing() {
    for payload in [
        "",
        "not Omega",
        "é🦀",
        "a\r\nb\0c",
        "end\nsource 0\n",
        "final LF\n",
    ] {
        let mut proposal = pending();
        proposal.proposed_build = payload.into();
        let text = proposal.encode().unwrap();
        assert!(text.contains(&format!(
            "proposed-build {}\n{payload}\nsource ",
            payload.len()
        )));
        assert_eq!(
            PendingPackageChange::recover(&text).unwrap().proposed_build,
            payload
        );
    }
}

#[test]
fn all_catalog_targets_round_trip_and_order_uses_canonical_names() {
    for target in TargetProfile::ALL {
        let mut proposal = pending();
        proposal.targets = vec![target];
        proposal.source = source(target);
        let recovered = PendingPackageChange::recover(&proposal.encode().unwrap()).unwrap();
        assert_eq!(recovered.targets, [target]);
        assert_eq!(recovered.source.target_profile(), target);
    }
    let mut proposal = pending();
    proposal.targets = TargetProfile::ALL.to_vec();
    proposal.targets.sort_by_key(|target| target.target_name());
    assert_eq!(proposal.targets[0], TargetProfile::CrossPlatformCli);
    proposal.source = source(proposal.targets[0]);
    assert_eq!(
        PendingPackageChange::recover(&proposal.encode().unwrap())
            .unwrap()
            .targets,
        proposal.targets,
    );
}

#[test]
fn rejects_noncanonical_envelope_rows() {
    let text = pending().encode().unwrap();
    for malformed in [
        text.replacen("proposal 1\n", "proposal 2\n", 1),
        text.replacen("kind install", "kind remove", 1),
        text.replacen("kind install", "kind  install", 1),
        text.replacen("kind install", "kind install ", 1),
        text.replacen("kind install\n", "", 1),
        text.replacen("kind install\n", "kind install\nkind install\n", 1),
        text.replacen("before-lock absent", "before-lock none", 1),
        text.replace('\n', "\r\n"),
        format!("{text}\n"),
        format!("{text}trailing"),
        format!("\u{feff}{text}"),
    ] {
        assert!(PendingPackageChange::recover(&malformed).is_err());
    }
}

#[test]
fn rejects_every_truncated_envelope() {
    let text = pending().encode().unwrap();
    for length in 0..text.len() {
        assert!(
            PendingPackageChange::recover(&text[..length]).is_err(),
            "length {length}"
        );
    }
}

#[test]
fn rejects_malformed_and_noncanonical_digests() {
    let mut proposal = pending();
    proposal.before_lock = Some([0xef; 32]);
    let text = proposal.encode().unwrap();
    for (label, digest) in [
        ("before-build", proposal.before_build),
        ("before-lock", proposal.before_lock.unwrap()),
        ("original-content", proposal.original_content),
    ] {
        let digest = write_digest(&digest);
        for value in [
            digest.to_uppercase(),
            digest[..63].into(),
            format!("{digest}0"),
            format!("g{}", &digest[1..]),
            format!(" {digest}"),
            format!("{digest} "),
        ] {
            let malformed = text.replacen(
                &format!("{label} {digest}\n"),
                &format!("{label} {value}\n"),
                1,
            );
            assert!(PendingPackageChange::recover(&malformed).is_err());
        }
    }
}

#[test]
fn rejects_counts_before_payload_allocation() {
    let text = pending().encode().unwrap();
    for count in [
        "0",
        "33",
        "18446744073709551615",
        "18446744073709551616",
        "01",
        "+1",
        "-1",
        "1 ",
        "",
        "١",
    ] {
        assert!(
            PendingPackageChange::recover(&text.replacen(
                "targets 1\n",
                &format!("targets {count}\n"),
                1
            ))
            .is_err()
        );
    }
    for (label, maximum) in [
        ("proposed-build", MAX_BUILD_DECLARATION_BYTES),
        (
            "source",
            CanonicalSourceClosureSubjectLimits::default().maximum_record_bytes,
        ),
    ] {
        let original = text
            .lines()
            .find(|line| line.starts_with(&format!("{label} ")))
            .unwrap();
        for count in [
            "18446744073709551616".into(),
            (maximum + 1).to_string(),
            "01".into(),
            "-1".into(),
            "+1".into(),
        ] {
            assert!(
                PendingPackageChange::recover(&text.replacen(
                    original,
                    &format!("{label} {count}"),
                    1
                ))
                .is_err()
            );
        }
    }
}

#[test]
fn rejects_wrong_lengths_missing_separators_and_utf8_splits() {
    let mut proposal = pending();
    proposal.proposed_build = "é".into();
    let text = proposal.encode().unwrap();
    for malformed in [
        text.replacen("proposed-build 2\n", "proposed-build 1\n", 1),
        text.replacen("proposed-build 2\n", "proposed-build 3\n", 1),
        text.replacen("é\nsource ", "ésource ", 1),
        text.replacen("é\nsource ", "é\r\nsource ", 1),
        text.replacen("é\nsource ", "é\n\nsource ", 1),
        text.replacen("\nend\n\nend\n", "\nend\nend\n", 1),
    ] {
        assert_ne!(malformed, text);
        assert!(PendingPackageChange::recover(&malformed).is_err());
    }
}

#[test]
fn rejects_duplicate_unordered_unknown_and_mismatched_targets() {
    let proposal = pending();
    let text = proposal.encode().unwrap();
    let first = TargetProfile::CrossPlatformCli.identity().as_str();
    let second = TargetProfile::LinuxArm64.identity().as_str();
    let target_row = format!("target {first}\n");
    for rows in [
        format!("target {first}\ntarget {first}\n"),
        format!("target {second}\ntarget {first}\n"),
    ] {
        let malformed =
            text.replacen("targets 1\n", "targets 2\n", 1)
                .replacen(&target_row, &rows, 1);
        assert!(PendingPackageChange::recover(&malformed).is_err());
    }
    for identity in ["linux_x64", "unknown", second] {
        assert!(
            PendingPackageChange::recover(&text.replacen(
                &target_row,
                &format!("target {identity}\n"),
                1
            ))
            .is_err()
        );
    }
    for targets in [
        vec![],
        vec![proposal.targets[0]; 33],
        vec![proposal.targets[0]; 2],
        vec![TargetProfile::LinuxArm64, proposal.targets[0]],
        vec![TargetProfile::LinuxArm64],
    ] {
        let mut proposal = pending();
        proposal.targets = targets;
        assert!(proposal.encode().is_err());
    }
    let mut proposal = pending();
    proposal.targets.push(TargetProfile::LinuxArm64);
    proposal.source = source(TargetProfile::LinuxArm64);
    assert!(
        proposal.encode().is_err(),
        "source membership alone is insufficient"
    );
    let source_text = proposal.source.canonical_text(Default::default()).unwrap();
    let original_source = text.find("source ").unwrap();
    let malformed = format!(
        "{}source {}\n{source_text}\nend\n",
        text[..original_source]
            .replacen("targets 1\n", "targets 2\n", 1)
            .replacen(&target_row, &format!("{target_row}target {second}\n"), 1),
        source_text.len()
    );
    assert!(PendingPackageChange::recover(&malformed).is_err());
}

#[test]
fn embedded_source_must_be_canonical_and_consistent() {
    let text = pending().encode().unwrap();
    for malformed in [
        text.replacen("omega-source-closure 1", "omega-source-closure 2", 1),
        text.replacen("packages 1", "packages 0", 1),
        text.replacen("lineage github", "lineage GitHub", 1),
        text.replacen("11111111", "31111111", 1),
    ] {
        assert!(PendingPackageChange::recover(&malformed).is_err());
    }
}

#[test]
fn exact_declaration_bound_round_trips_and_larger_build_rejects() {
    let mut proposal = pending();
    proposal.proposed_build = "x".repeat(MAX_BUILD_DECLARATION_BYTES);
    let text = proposal.encode().unwrap();
    assert_eq!(
        PendingPackageChange::recover(&text)
            .unwrap()
            .proposed_build
            .len(),
        MAX_BUILD_DECLARATION_BYTES
    );
    proposal.proposed_build.push('x');
    assert!(proposal.encode().is_err());
}

#[test]
fn whole_text_limit_and_writer_limit_are_checked() {
    let oversized = "x".repeat(MAXIMUM_TEXT_BYTES + 1);
    assert!(matches!(
        PendingPackageChange::recover(&oversized),
        Err(error) if error.contains("text byte limit")
    ));
    let mut writer = Writer::new(3);
    writer.append("abc").unwrap();
    assert!(writer.append("d").is_err());
    assert_eq!(writer.finish(), "abc");
}
