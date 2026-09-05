use super::{
    HOST, PROCESS, TARGET, assert_locked_authority, assert_no_proposal, assert_recorded_pin,
    assert_status, check_import, install_authority, package_section, review,
};
use package_manager::lock::HistoricalPackagePolicyDecisionSubject;
use package_manager::resolution::graph::CanonicalDependencySourceRequest;
use package_manager::review::ReviewOnlyRootPolicyDisposition;
use package_source::ImmutableSourceResolution;
use std::fs;

const REMOVED: &str = "13e4afd9c907503cb674d4450fdd3b1a19033d5d";
const RESTORED: &str = "7926228b0918574dd532dda0008a6aa80881bce9";

#[test]
#[ignore = "requires network and private CathedralOS process-exit/host-services access over SSH"]
fn pinned_ssh_process_updates_review_removed_and_reintroduced_authority() {
    assert_recorded_pin(REMOVED);
    assert_recorded_pin(RESTORED);
    let fixture = install_authority("process-exit", PROCESS, "Console", "Process");
    let original = fixture.lock();
    let original_target = original.target(TARGET).unwrap();
    let original_consumer = original_target
        .source()
        .packages()
        .iter()
        .find(|package| package.key().name().as_str() == "process-exit")
        .unwrap();
    let original_policy = original_target
        .baselines()
        .iter()
        .find(|policy| policy.package() == original_consumer.key().identity())
        .unwrap();
    let original_callable = original_policy
        .callables()
        .callables()
        .iter()
        .find(|callable| callable.identity().path().contains("terminate"))
        .unwrap();
    let mut previous_pin = PROCESS;
    let mut previous_review = None;
    for (pin, change) in [(REMOVED, "removed"), (RESTORED, "added")] {
        let before = fixture.accepted_files();
        let output = fixture.omega(&[
            "update",
            "process_exit",
            "--to",
            pin,
            "--target",
            "linux_x86_64",
        ]);
        assert_status(&output, 3);
        assert_eq!(fixture.accepted_files(), before);
        let (path, document) = review(&fixture, &output);
        let section = package_section(&document, "process-exit");
        assert!(section.contains("source-changed true\n"), "{section}");
        let authority_rows = section
            .split("\nchange ")
            .filter(|row| row.starts_with(&format!("dangerous_capability {change}\n")))
            .collect::<Vec<_>>();
        assert_eq!(authority_rows.len(), 1, "{section}");
        assert!(authority_rows[0].contains("Console"), "{section}");
        let decision = authority_rows[0]
            .lines()
            .find(|line| line.starts_with("decision ") && line.ends_with(" pending"))
            .expect("authority change must have its own pending choice");
        let accepted = document
            .lines()
            .map(|line| {
                if line.starts_with("decision ") {
                    format!("{} accept\n", line.strip_suffix(" pending").unwrap())
                } else {
                    format!("{line}\n")
                }
            })
            .collect::<String>();
        let accepted_decision = decision.replace(" pending", " accept");
        for choice in ["pending", "reject"] {
            fs::write(
                &path,
                accepted.replace(
                    &accepted_decision,
                    &decision.replace(" pending", &format!(" {choice}")),
                ),
            )
            .unwrap();
            assert_status(&fixture.omega(&["update", "--resume"]), 3);
            assert_eq!(fixture.accepted_files(), before);
        }
        // A different row's choice cannot stand in for this authority delta.
        let other = accepted
            .lines()
            .find(|line| line.starts_with("decision ") && *line != accepted_decision)
            .expect("reach/invocation changes need choices separate from dangerous authority");
        fs::write(&path, accepted.replace(&accepted_decision, other)).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 1);
        assert_eq!(fixture.accepted_files(), before);
        if let Some(stale) = &previous_review {
            fs::write(&path, stale).unwrap();
            assert_status(&fixture.omega(&["update", "--resume"]), 1);
            assert_eq!(fixture.accepted_files(), before);
        }
        fs::write(&path, &accepted).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 0);
        assert_ne!(fixture.accepted_files().0, before.0);
        assert_ne!(fixture.accepted_files().1, before.1);
        assert_no_proposal(&fixture);

        let lock = fixture.lock();
        let target = lock.target(TARGET).unwrap();
        assert_eq!(lock.targets().len(), 1);
        assert_eq!(target.source().packages().len(), 3);
        assert_eq!(target.source().dependency_requests().len(), 2);
        for original_package in original_target.source().packages() {
            let current = target
                .source()
                .packages()
                .iter()
                .find(|package| package.key() == original_package.key())
                .expect("revision changes must preserve every package's stable identity");
            if current.key() == original_consumer.key() {
                let ImmutableSourceResolution::Git { commit, .. } = current.resolution() else {
                    panic!("consumer must remain Git-pinned");
                };
                assert_eq!(commit.to_hex(), pin);
            } else if current.key().name().as_str() == "host-services" {
                assert_eq!(current.resolution(), original_package.resolution());
                let ImmutableSourceResolution::Git { commit, .. } = current.resolution() else {
                    panic!("host must remain Git-pinned");
                };
                assert_eq!(commit.to_hex(), HOST);
            }
        }
        for original_edge in original_target.source().dependency_requests() {
            let current = target
                .source()
                .dependency_requests()
                .iter()
                .find(|edge| {
                    edge.requester() == original_edge.requester()
                        && edge.selected().key() == original_edge.selected().key()
                })
                .expect("update must retain both dependency edges");
            let CanonicalDependencySourceRequest::Git {
                repository,
                revision,
                ..
            } = current.request()
            else {
                panic!("update must retain SSH requests");
            };
            if current.selected().key() == original_consumer.key() {
                assert_eq!(repository, "git@github.com:CathedralOS/process-exit.git");
                assert_eq!(revision, pin);
            } else {
                assert_eq!(current.request(), original_edge.request());
            }
        }
        let policy = target
            .baselines()
            .iter()
            .find(|policy| policy.package() == original_consumer.key().identity())
            .unwrap();
        let callable = policy
            .callables()
            .callables()
            .iter()
            .find(|callable| callable.identity() == original_callable.identity())
            .expect("same public callable must survive the authority change");
        if change == "removed" {
            assert!(policy.dangerous_capabilities().is_empty());
            assert!(callable.declared_service_reach().unwrap().is_empty());
            assert!(
                callable
                    .checked_service_reach()
                    .realized()
                    .unwrap()
                    .is_empty()
            );
            assert!(callable.realized_synchronous_invocations().is_empty());
        } else {
            assert_locked_authority(&fixture, "process-exit", pin, "Console", "Process");
            assert_eq!(
                policy, original_policy,
                "restored source must restore its policy"
            );
            assert!(section.contains("audit-recommended true\n"), "{section}");
        }
        let mut reviewed = document
            .lines()
            .filter(|line| line.starts_with("decision "))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let mut recorded = target
            .decisions()
            .decisions()
            .iter()
            .map(|decision| {
                assert_eq!(
                    decision.disposition(),
                    ReviewOnlyRootPolicyDisposition::AcceptCandidateChange
                );
                let HistoricalPackagePolicyDecisionSubject::Row(digest) = decision.subject() else {
                    panic!("authority updates must record exact rows");
                };
                let fingerprint = digest
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                format!("decision row {fingerprint} pending")
            })
            .collect::<Vec<_>>();
        reviewed.sort();
        recorded.sort();
        assert_eq!(recorded, reviewed);
        let patch = fixture.read("root/build/package-manager/source-diff.txt");
        assert!(
            patch.contains(&format!("baseline_git_commit {previous_pin}\n")),
            "{patch}"
        );
        assert!(
            patch.contains(&format!("candidate_git_commit {pin}\n")),
            "{patch}"
        );
        assert!(patch.contains("entry main.omg\n"), "{patch}");
        let direction = if change == "removed" {
            "removed"
        } else {
            "added"
        };
        assert!(
            patch.contains(&format!(
                "{direction} lf     console.exit_process(return_code);\n"
            )),
            "{patch}"
        );
        check_import(&fixture, "process-exit");
        previous_pin = pin;
        previous_review = Some(accepted);
    }
}
