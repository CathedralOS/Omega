use super::{tests::fixture, *};
use crate::encoding::PackagePolicyTextRecoveryLimits;

fn recover(text: &str) -> Result<PackagePolicyBaseline, Error> {
    PackagePolicyBaseline::recover_text(text, PackagePolicyTextRecoveryLimits::default())
}

#[test]
fn named_text_recovers_exact_binary_and_typed_meaning() {
    let value = fixture();
    let text = value.canonical_text().unwrap();
    for label in [
        "public_api",
        "callables",
        "selected_providers",
        "terminal_permissions",
        "representation",
        "external_supplies",
        "dangerous_capabilities",
        "slack_uses",
        "semantic_dependencies",
        "boundary_applications",
    ] {
        assert!(text.contains(&format!("field {label} {{")), "{label}");
    }
    assert!(text.contains("tag layout 1"));
    let (binary, _) = crate::encoding::recovery::decode_policy_text_scalars(
        &text,
        PackagePolicyRecoveryLimits::default(),
    )
    .unwrap();
    assert_eq!(binary, value.canonical_bytes().unwrap());
    let recovered = recover(&text).unwrap();
    assert_eq!(value, recovered);
    assert_eq!(text, recovered.canonical_text().unwrap());
}

#[test]
fn labels_variants_whitespace_and_scalar_spellings_are_authoritative() {
    let text = fixture().canonical_text().unwrap();
    for changed in [
        text.replacen("field public_api", "field private_api", 1),
        text.replacen("tag layout 1", "tag ownership_behavior 1", 1),
        text.replacen("u16 1", "u16 01", 1),
        text.replacen("u16 1", "u16 +1", 1),
        text.replacen("  field", " field", 1),
        text.replace('\n', "\r\n"),
        format!("{text}\n"),
    ] {
        assert_ne!(text, changed);
        assert!(recover(&changed).is_err(), "noncanonical text accepted");
    }
    assert_eq!(
        recover(&text.replacen(
            "omega_package_policy_text 1",
            "omega_package_policy_text 2",
            1
        )),
        Err(Error::UnsupportedVersion)
    );
    for (end, _) in text.match_indices('\n').step_by(7) {
        assert!(recover(&text[..end]).is_err(), "truncated text at {end}");
    }
}

#[test]
fn text_diff_localizes_one_changed_declared_constant() {
    let original = fixture();
    let mut changed = original.clone();
    changed.public_api.consts[0].canonical_value_encoding = "43".to_owned();
    let before = original.canonical_text().unwrap();
    let after = changed.canonical_text().unwrap();
    let differences = before
        .lines()
        .zip(after.lines())
        .filter(|(before, after)| before != after)
        .collect::<Vec<_>>();
    assert_eq!(differences.len(), 1);
    assert!(differences[0].0.contains("string \"42\""));
    assert!(differences[0].1.contains("string \"43\""));
    assert_eq!(recover(&after).unwrap(), changed);
}

#[test]
fn expanded_text_and_all_owned_recovery_storage_obey_shared_limits() {
    let value = fixture();
    let text = value.canonical_text().unwrap();
    let binary = value.canonical_bytes().unwrap();
    let limits = |owned| {
        PackagePolicyTextRecoveryLimits::new(
            text.len(),
            PackagePolicyRecoveryLimits::new(binary.len(), binary.len(), 65_536, owned, 128),
        )
    };
    assert_eq!(
        PackagePolicyBaseline::recover_text(&text, limits(64 * 1024 * 1024)).unwrap(),
        value
    );
    let mut low = 0;
    let mut high = 64 * 1024 * 1024;
    while low < high {
        let middle = low + (high - low) / 2;
        if PackagePolicyBaseline::recover_text(&text, limits(middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(
        low > 2 * binary.len(),
        "binary reconstruction and canonical scratch coexist with typed data"
    );
    assert_eq!(
        PackagePolicyBaseline::recover_text(&text, limits(low - 1)),
        Err(Error::AllocationLimitExceeded)
    );
    assert_eq!(
        PackagePolicyBaseline::recover_text(
            &text,
            PackagePolicyTextRecoveryLimits::new(
                text.len() - 1,
                PackagePolicyRecoveryLimits::default()
            )
        ),
        Err(Error::InputTooLarge)
    );
    assert_eq!(
        PackagePolicyBaseline::recover_text(
            &text,
            PackagePolicyTextRecoveryLimits::new(
                text.len(),
                PackagePolicyRecoveryLimits::new(
                    binary.len() - 1,
                    binary.len(),
                    65_536,
                    64 * 1024 * 1024,
                    128
                )
            )
        ),
        Err(Error::InputTooLarge)
    );
}
