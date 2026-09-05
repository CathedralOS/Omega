use super::{
    PROCESS_BUILD, PROCESS_SOURCE, TARGET, accept_document, accept_install, assert_status,
    authority_decision, authority_fixture, check_import, fs, package_section, review,
};

#[test]
fn missing_lock_update_requires_fresh_exact_authority_decisions_before_publication() {
    let fixture = authority_fixture(PROCESS_BUILD, PROCESS_SOURCE);
    accept_install(&fixture);
    let previous = fixture.lock();
    let previous_target = previous.target(TARGET).unwrap();
    assert_eq!(previous_target.source().packages().len(), 3);
    let review_path = fixture.path("root/build/package-manager/review-linux_x86_64.txt");
    let previous_review = fs::read(&review_path).unwrap();
    let proposal_path = fixture.path("root/build/package-manager/proposal");
    let pending_path = fixture.path("root/build/package-manager/pending");
    assert!(!proposal_path.exists());
    assert!(!pending_path.exists());

    // Remove only this fixture's accepted lock; installed declarations and
    // historical accepted review tokens must not supply a replacement baseline.
    fs::remove_file(fixture.path("root/omega.lock")).unwrap();
    let before = fixture.accepted_files();
    assert!(before.0.is_some());
    assert!(before.1.is_none());
    for selection in ["process_exit", "process-exit"] {
        let output = fixture.omega(&["update", selection, "--target", "linux_x86_64"]);
        assert_status(&output, 1);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("omega.lock is missing; run omega update without package selections to review the complete graph first"),
            "{stderr}"
        );
        assert_eq!(fixture.accepted_files(), before);
        assert_eq!(fs::read(&review_path).unwrap(), previous_review);
        assert!(!proposal_path.exists());
        assert!(!pending_path.exists());
    }

    let output = fixture.omega(&["update", "--target", "linux_x86_64"]);
    assert_status(&output, 3);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("No accepted lock baseline: reviewing the complete candidate graph."),
        "{stdout}"
    );
    assert_eq!(fixture.accepted_files(), before);
    assert!(proposal_path.is_file());
    assert!(!pending_path.exists());
    let (path, document) = review(&fixture, &output);
    assert_eq!(path, review_path);
    assert!(document.lines().any(|line| line == "baseline none"));
    assert_eq!(
        document
            .lines()
            .filter(|line| line.starts_with("package "))
            .count(),
        previous_target.source().packages().len()
    );
    for package in previous_target.source().packages() {
        package_section(&document, package.key().name().as_str());
    }
    let decision = authority_decision(
        package_section(&document, "process-exit"),
        "added",
        "Console",
    );
    let decision_count = document
        .lines()
        .filter(|line| line.starts_with("decision "))
        .count();
    assert!(
        decision_count > 1,
        "other choices must not accept the dangerous row"
    );
    assert_eq!(document.lines().filter(|line| *line == decision).count(), 1);
    accept_document(&path, &document);
    let accepted_document = fs::read_to_string(&path).unwrap();
    let accepted_decision = decision.replace(" pending", " accept");
    let proposal = fs::read(&proposal_path).unwrap();
    for disposition in ["pending", "reject"] {
        let blocked_document = accepted_document.replace(
            &accepted_decision,
            &decision.replace(" pending", &format!(" {disposition}")),
        );
        fs::write(&path, &blocked_document).unwrap();
        assert_status(&fixture.omega(&["update", "--resume"]), 3);
        assert_eq!(fixture.accepted_files(), before);
        assert_eq!(fs::read_to_string(&path).unwrap(), blocked_document);
        assert_eq!(fs::read(&proposal_path).unwrap(), proposal);
        assert!(!pending_path.exists());
    }

    fs::write(&path, accepted_document).unwrap();
    assert_status(&fixture.omega(&["update", "--resume"]), 0);
    assert_eq!(fixture.accepted_files().0, before.0);
    assert!(fixture.accepted_files().1.is_some());
    assert!(!proposal_path.exists());
    assert!(!pending_path.exists());
    let published = fixture.lock();
    let published_target = published.target(TARGET).unwrap();
    assert_eq!(published_target.source(), previous_target.source());
    assert_eq!(published_target.baselines(), previous_target.baselines());
    assert!(
        published_target
            .decisions()
            .baseline_source_subject()
            .is_none()
    );
    assert_eq!(
        published_target.decisions().decisions().len(),
        decision_count
    );
    check_import(&fixture, "process_exit");
}
