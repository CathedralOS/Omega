//! Token syntax is public callable meaning, independent of its implementation.

use super::{Fixture, package_identity, project};
use language_core::OperatorSpelling;
use package_evidence::encoding::{
    PackagePolicyRecoveryLimits, PackagePolicyTextRecoveryLimits,
    decode_package_review_canonical_row, encode_package_review_canonical_row,
};
use package_evidence::project_checked_package_review;
use package_evidence::record::{
    PackagePolicyBaseline, PackageReviewCanonicalRow, PackageReviewCanonicalRowKind,
    PackageReviewSourceLocationRole,
};

#[test]
fn direct_machine_token_changes_survive_package_review() {
    let reviews = [
        ("", None),
        ("+ ", Some(OperatorSpelling::Add)),
        ("- ", Some(OperatorSpelling::Subtract)),
    ]
    .map(|(token, spelling)| {
        let fixture = Fixture::local(&format!(
            "pub data Wrapped {{ value: u64; }}
             pub machine {token}Wrapped::combine(left: &Wrapped, right: &Wrapped) -> u64 {{ 0 }}"
        ));
        let review = project_checked_package_review(&fixture.checked)
            .expect("a checked direct machine has a complete review surface");
        let policy = project(&fixture);
        let callable = review
            .callables()
            .iter()
            .find(|callable| callable.identity().path() == "Wrapped::combine")
            .expect("exact authored callable");
        assert_eq!(callable.spelling(), spelling);
        assert_eq!(
            super::callable(&policy, "Wrapped::combine").spelling(),
            spelling
        );
        let rows = review.canonical_rows().unwrap();
        let mut matching = rows.into_iter().filter(|row| {
            row.kind() == PackageReviewCanonicalRowKind::Callable
                && row.source().authored_locations().is_some_and(|locations| {
                    locations.iter().any(|location| {
                        location.relative_path() == "main.omg"
                            && location.role() == PackageReviewSourceLocationRole::Declaration
                    })
                })
        });
        let row = matching.next().expect("main.omg public callable row");
        assert!(
            matching.next().is_none(),
            "one authored callable in main.omg"
        );
        assert_row_recovers(&row);
        let baseline = assert_policy_recovers(&fixture);
        if spelling == Some(OperatorSpelling::Add) {
            let text = baseline.canonical_text().unwrap();
            // Mutate the named spelling tag, without relying on binary offsets.
            assert_eq!(text.matches("tag add 0\n").count(), 1);
            let malformed = text.replacen("tag add 0\n", "tag unknown_token 0\n", 1);
            assert!(
                PackagePolicyBaseline::recover_text(
                    &malformed,
                    PackagePolicyTextRecoveryLimits::default(),
                )
                .is_err()
            );
        }
        (review, policy, row)
    });
    for left in 0..reviews.len() {
        for right in left + 1..reviews.len() {
            assert_ne!(
                reviews[left].0.callables(),
                reviews[right].0.callables(),
                "adding, removing or changing a token changes the callable contract"
            );
            assert_eq!(
                reviews[left].2.key_bytes(),
                reviews[right].2.key_bytes(),
                "token edits keep the exact direct declaration row coordinate"
            );
            assert_ne!(
                reviews[left].2.canonical_bytes(),
                reviews[right].2.canonical_bytes()
            );
            assert_ne!(
                reviews[left].0.canonical_review_bytes().unwrap(),
                reviews[right].0.canonical_review_bytes().unwrap()
            );
            assert_ne!(
                reviews[left].1.canonical_bytes().unwrap(),
                reviews[right].1.canonical_bytes().unwrap()
            );
        }
    }
}

fn assert_row_recovers(row: &PackageReviewCanonicalRow) {
    let envelope = encode_package_review_canonical_row(row).unwrap();
    let recovered = decode_package_review_canonical_row(&envelope).unwrap();
    assert_eq!(recovered.kind(), row.kind());
    assert_eq!(recovered.key_bytes(), row.key_bytes());
    assert_eq!(recovered.canonical_bytes(), row.canonical_bytes());
}

fn assert_policy_recovers(fixture: &Fixture) -> PackagePolicyBaseline {
    let policy = package_evidence::project_checked_package_policy(
        &fixture.checked,
        fixture.target,
        package_identity(),
    )
    .expect("complete token-bearing package policy");
    let bytes = policy.canonical_bytes().unwrap();
    let recovered =
        PackagePolicyBaseline::recover_canonical(&bytes, PackagePolicyRecoveryLimits::default())
            .unwrap();
    assert_eq!(recovered, policy);
    let text = policy.canonical_text().unwrap();
    let recovered =
        PackagePolicyBaseline::recover_text(&text, PackagePolicyTextRecoveryLimits::default())
            .unwrap();
    assert_eq!(recovered, policy);
    assert_eq!(recovered.canonical_bytes().unwrap(), bytes);
    assert_eq!(recovered.canonical_text().unwrap(), text);
    policy
}

#[test]
fn trait_requirement_tokens_survive_review_and_policy_recovery() {
    let variants = [
        ("machine", None),
        ("operator +", Some(OperatorSpelling::Add)),
        ("operator -", Some(OperatorSpelling::Subtract)),
    ];
    let rows = variants.map(|(introducer, spelling)| {
        let fixture = Fixture::local(&format!(
            "pub data Wrapped {{ value: u64; }}
             pub trait Combine {{ {introducer} combine(left: &Wrapped, right: &Wrapped) -> u64; }}"
        ));
        let review =
            project_checked_package_review(&fixture.checked).expect("trait requirement review");
        let declared = review
            .public_traits()
            .iter()
            .find(|definition| definition.identity().path() == "Combine")
            .expect("exact authored trait");
        assert_eq!(declared.requirements().len(), 1);
        assert_eq!(declared.requirements()[0].spelling(), spelling);
        let policy = assert_policy_recovers(&fixture);
        let retained = policy
            .public_traits()
            .iter()
            .find(|definition| definition.identity().path() == "Combine")
            .expect("retained authored trait");
        assert_eq!(retained.requirements().len(), 1);
        assert_eq!(retained.requirements()[0].spelling(), spelling);
        let row = review
            .canonical_rows()
            .unwrap()
            .into_iter()
            .find(|row| row.kind() == PackageReviewCanonicalRowKind::PublicTrait)
            .expect("trait row");
        assert_row_recovers(&row);
        row
    });
    for left in 0..rows.len() {
        for right in left + 1..rows.len() {
            assert_eq!(rows[left].key_bytes(), rows[right].key_bytes());
            assert_ne!(rows[left].canonical_bytes(), rows[right].canonical_bytes());
        }
    }
}
