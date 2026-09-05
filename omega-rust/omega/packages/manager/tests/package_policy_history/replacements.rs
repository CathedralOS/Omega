use super::*;
use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisionSubject as HistorySubject, HistoricalPackagePolicyError,
    HistoricalPackagePolicyReplacementSite as HistorySite,
};
use omega_package_manager::review::PackagePolicyReplacementSite;

fn bindings(active: &str) -> String {
    format!(
        "builder.depend_as(\"active\", Source::Path {{ location: \"../{active}\" }});\n\
         builder.depend_as(\"old_available\", Source::Path {{ location: \"../old\" }});\n\
         builder.depend_as(\"new_available\", Source::Path {{ location: \"../new\" }});\n"
    )
}

#[test]
fn binding_flip_preserves_present_baseline_and_exact_replacement_site() {
    let tree = Tree::new();
    package(&tree.path("sources/old"), "same-name", "");
    package(&tree.path("sources/new"), "same-name", "");
    source(&tree, "", &bindings("old"));
    let (old_sources, old_reviews) = candidate(&tree, "old-binding");
    let initial = compare(None, &old_sources, &old_reviews);
    let accepted = history_lock(
        &old_sources,
        &old_reviews,
        &initial,
        &resolution(&initial, ACCEPT),
    );

    source(&tree, "", &bindings("new"));
    let (sources, reviews) = candidate(&tree, "new-binding");
    let changes = compare(accepted.target(TARGET), &sources, &reviews);
    let [replacement] = changes.source_replacements() else {
        panic!("only the established active binding changes its selected source");
    };
    assert!(
        matches!(replacement.site(), PackagePolicyReplacementSite::Dependency { alias, .. } if alias.as_str() == "active")
    );
    assert_ne!(replacement.baseline(), replacement.candidate());
    assert_eq!(
        replacement.baseline().name(),
        replacement.candidate().name()
    );

    for disposition in [ACCEPT, REJECT] {
        let resolved = resolution(&changes, disposition);
        let lock = history_lock(&sources, &reviews, &changes, &resolved);
        let target = lock.target(TARGET).unwrap();
        let history = target.decisions();
        let source = target.source();
        assert!(
            source
                .packages()
                .iter()
                .any(|package| package.key() == replacement.baseline())
        );
        let decision = history
            .decisions()
            .iter()
            .find(|decision| matches!(decision.subject(), HistorySubject::SourceReplacement { .. }))
            .unwrap();
        let HistorySubject::SourceReplacement {
            baseline,
            package_index,
            site:
                HistorySite::Dependency {
                    requester_index,
                    alias,
                },
        } = decision.subject()
        else {
            panic!("history must preserve the requester-local replacement");
        };
        assert_eq!(baseline, replacement.baseline());
        assert_eq!(
            source.packages()[*package_index].key(),
            replacement.candidate()
        );
        assert_eq!(
            source.packages()[*requester_index].key(),
            sources.graph().root()
        );
        assert_eq!(alias.as_str(), "active");
        assert_eq!(decision.package_index(), Some(*package_index));
        assert_eq!(decision.conflict(), replacement.fingerprint().digest());
        assert_eq!(decision.disposition(), disposition);

        let text = history
            .canonical_text(source, HistoricalPackagePolicyLimits::default())
            .unwrap();
        let (recovered, usage) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
            &text,
            source,
            HistoricalPackagePolicyLimits::default(),
            usize::MAX,
        )
        .unwrap();
        assert_eq!(&recovered, history);
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text_with_usage(
                &text,
                source,
                HistoricalPackagePolicyLimits::default(),
                usage.owned_bytes(),
            )
            .unwrap()
            .0,
            recovered
        );
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text_with_usage(
                &text,
                source,
                HistoricalPackagePolicyLimits::default(),
                usage.owned_bytes() - 1,
            )
            .unwrap_err(),
            HistoricalPackagePolicyError::AllocationLimitExceeded
        );

        let prefix =
            format!("decision replacement dependency {package_index} {requester_index} active ");
        assert!(text.contains(&prefix));
        for wrong in [
            format!("decision replacement dependency {requester_index} {requester_index} active "),
            format!("decision replacement dependency {package_index} {package_index} active "),
            format!(
                "decision replacement dependency {package_index} {requester_index} missing_alias "
            ),
        ] {
            let malformed = text.replacen(&prefix, &wrong, 1);
            assert_ne!(malformed, text);
            assert!(matches!(
                HistoricalPackagePolicyDecisions::recover_text(
                    &malformed,
                    source,
                    HistoricalPackagePolicyLimits::default(),
                ),
                Err(HistoricalPackagePolicyError::InvalidSubject)
            ));
        }

        // A distinct digest cannot create a second historical replacement at
        // the same binding. Preserve framing, fragment bytes and sorted order.
        let start = text.find(&prefix).unwrap();
        let line_end = start + text[start..].find('\n').unwrap();
        let line = &text[start..line_end];
        let fields = line.split(' ').collect::<Vec<_>>();
        let length = fields[6].parse::<usize>().unwrap();
        let end = line_end + 1 + length;
        let last_digest = "ff".repeat(32);
        assert_ne!(fields[7], last_digest);
        let duplicate = text[start..end].replacen(fields[7], &last_digest, 1);
        let mut malformed = text.clone();
        malformed.insert_str(end, &duplicate);
        malformed = malformed.replacen(
            &format!("decisions {}\n", history.decisions().len()),
            &format!("decisions {}\n", history.decisions().len() + 1),
            1,
        );
        assert_eq!(
            HistoricalPackagePolicyDecisions::recover_text(
                &malformed,
                source,
                HistoricalPackagePolicyLimits::default(),
            )
            .unwrap_err(),
            HistoricalPackagePolicyError::NonCanonicalDecisions
        );
    }
}

#[test]
fn root_name_replacement_roundtrips_exact_old_and_new_keys() {
    let tree = Tree::new();
    source(&tree, "pub const VALUE: u64 = 7;\n", "");
    let (old_sources, old_reviews) = candidate(&tree, "old-root");
    let initial = compare(None, &old_sources, &old_reviews);
    let accepted = history_lock(
        &old_sources,
        &old_reviews,
        &initial,
        &resolution(&initial, ACCEPT),
    );
    package(&tree.path("sources/root"), "renamed-root", "");
    fs::write(
        tree.path("sources/root/main.omg"),
        "pub const VALUE: u64 = 7;\n",
    )
    .unwrap();
    let (sources, reviews) = candidate(&tree, "renamed-root");
    let changes = compare(accepted.target(TARGET), &sources, &reviews);
    let [replacement] = changes.source_replacements() else {
        panic!("one root replacement");
    };
    assert_eq!(replacement.site(), &PackagePolicyReplacementSite::Root);
    assert!(changes.root_role_change().is_none());
    let resolved = resolution(&changes, ACCEPT);
    let lock = history_lock(&sources, &reviews, &changes, &resolved);
    let target = lock.target(TARGET).unwrap();
    let decision = target
        .decisions()
        .decisions()
        .iter()
        .find(|decision| {
            matches!(
                decision.subject(),
                HistorySubject::SourceReplacement {
                    site: HistorySite::Root,
                    ..
                }
            )
        })
        .unwrap();
    let HistorySubject::SourceReplacement {
        baseline,
        package_index,
        ..
    } = decision.subject()
    else {
        unreachable!()
    };
    assert_eq!(baseline, old_sources.graph().root());
    assert_eq!(
        target.source().packages()[*package_index].key(),
        sources.graph().root()
    );
    assert_ne!(baseline, sources.graph().root());
    assert_eq!(decision.conflict(), replacement.fingerprint().digest());
    let text = target
        .decisions()
        .canonical_text(target.source(), HistoricalPackagePolicyLimits::default())
        .unwrap();
    assert!(text.contains(&format!("decision replacement root {package_index} ")));
    assert_eq!(
        HistoricalPackagePolicyDecisions::recover_text(
            &text,
            target.source(),
            HistoricalPackagePolicyLimits::default(),
        )
        .unwrap(),
        *target.decisions()
    );
}
