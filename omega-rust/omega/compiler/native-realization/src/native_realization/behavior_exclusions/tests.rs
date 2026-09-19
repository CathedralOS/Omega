use super::admit_behavior_exclusion_closure;
use build_evaluation::{BehaviorExclusion, BehaviorExclusions};
use effects::provider_plan::{ProviderPlanDigest, ServiceSchemaDigest};
use effects::{
    CompilerIntrinsicExecutionIdentity, SelectedProviderPlanFacts, TerminalAuthorityClass,
    TerminalAuthorityClosureLeaf, TerminalAuthorityClosureReviewReceipt,
    TerminalAuthorityDisposition, TerminalAuthorityPermissionPolicyIdentity,
    TerminalAuthorityPolicyIdentity, TerminalMechanismIdentity,
};

fn leaf(
    requirement: &str,
    exercised: &[TerminalAuthorityClass],
    permitted: Option<Vec<TerminalAuthorityClass>>,
) -> TerminalAuthorityClosureLeaf {
    TerminalAuthorityClosureLeaf::new(
        ServiceSchemaDigest::from_digest([1; 32]),
        requirement.to_owned(),
        ProviderPlanDigest::from_digest([2; 32]),
        TerminalMechanismIdentity::CompilerIntrinsic(
            CompilerIntrinsicExecutionIdentity::HostedWriteByteI32,
        ),
        TerminalAuthorityDisposition::from_classes(exercised.iter().copied()),
        permitted.map(TerminalAuthorityDisposition::from_classes),
    )
    .expect("leaf satisfies its own admission axes")
}

fn receipt(
    leaves: Vec<TerminalAuthorityClosureLeaf>,
    permission_policy: Option<TerminalAuthorityPermissionPolicyIdentity>,
) -> TerminalAuthorityClosureReviewReceipt {
    TerminalAuthorityClosureReviewReceipt::from_reviewed_leaves(
        [7; 32],
        target::NativeTarget::linux_x64(),
        SelectedProviderPlanFacts::default().identity_digest(),
        TerminalAuthorityPolicyIdentity::from_parts(7, [9; 32]),
        permission_policy,
        leaves,
    )
    .expect("review receipt is well-formed")
}

fn physical_exclusions(classes: &[TerminalAuthorityClass]) -> BehaviorExclusions {
    BehaviorExclusions::from_selections(
        classes
            .iter()
            .map(|&class| BehaviorExclusion::PhysicalAuthorityClass(class)),
    )
}

#[test]
fn physical_exclusion_holds_when_no_leaf_exercises_the_class() {
    let receipt = receipt(
        vec![leaf(
            "test::Store::read()",
            &[TerminalAuthorityClass::FilesystemContentRead],
            None,
        )],
        None,
    );
    let exclusions = physical_exclusions(&[
        TerminalAuthorityClass::ProcessOutput,
        TerminalAuthorityClass::PortIo,
    ]);
    assert_eq!(
        admit_behavior_exclusion_closure(&exclusions, &receipt),
        Ok(())
    );
}

#[test]
fn excluded_exercised_class_fails_without_a_permission_policy() {
    // The leaf carries `permitted: None` — the review made no receiver-
    // admission claim — yet the requested absence is still adjudicated:
    // classification evidence, not a permission grant, answers it.
    let receipt = receipt(
        vec![leaf(
            "test::Console::write()",
            &[TerminalAuthorityClass::ProcessOutput],
            None,
        )],
        None,
    );
    let exclusions = physical_exclusions(&[TerminalAuthorityClass::ProcessOutput]);
    let diagnostics =
        admit_behavior_exclusion_closure(&exclusions, &receipt).expect_err("exclusion must reject");
    let message = format!("{diagnostics:?}");
    assert!(message.contains("test::Console::write()"));
    assert!(message.contains("ProcessOutput"));
}

#[test]
fn receiver_permission_does_not_satisfy_a_requested_exclusion() {
    // A leaf explicitly permitted to exercise a class still violates a
    // requested exclusion of that class: the two axes are independent.
    let receipt = receipt(
        vec![leaf(
            "test::Console::write()",
            &[TerminalAuthorityClass::ProcessOutput],
            Some(vec![
                TerminalAuthorityClass::ProcessOutput,
                TerminalAuthorityClass::FilesystemContentRead,
            ]),
        )],
        Some(TerminalAuthorityPermissionPolicyIdentity::from_parts(
            1, [5; 32],
        )),
    );
    let exclusions = physical_exclusions(&[TerminalAuthorityClass::ProcessOutput]);
    assert!(admit_behavior_exclusion_closure(&exclusions, &receipt).is_err());
}

#[test]
fn every_violating_leaf_is_reported() {
    let receipt = receipt(
        vec![
            leaf(
                "test::Console::write()",
                &[TerminalAuthorityClass::ProcessOutput],
                None,
            ),
            leaf(
                "test::Serial::send()",
                &[TerminalAuthorityClass::PortIo],
                None,
            ),
            leaf(
                "test::Store::read()",
                &[TerminalAuthorityClass::FilesystemContentRead],
                None,
            ),
        ],
        None,
    );
    let exclusions = physical_exclusions(&[
        TerminalAuthorityClass::ProcessOutput,
        TerminalAuthorityClass::PortIo,
    ]);
    let diagnostics =
        admit_behavior_exclusion_closure(&exclusions, &receipt).expect_err("exclusion must reject");
    let message = format!("{diagnostics:?}");
    assert!(message.contains("test::Console::write()"));
    assert!(message.contains("test::Serial::send()"));
    assert!(!message.contains("test::Store::read()"));
}

#[test]
fn an_empty_exclusion_union_is_vacuous() {
    let receipt = receipt(
        vec![leaf(
            "test::Console::write()",
            &[TerminalAuthorityClass::ProcessOutput],
            None,
        )],
        None,
    );
    assert_eq!(
        admit_behavior_exclusion_closure(&BehaviorExclusions::default(), &receipt),
        Ok(())
    );
    // Crash-cause and service selections carry no physical axis either.
    let exclusions = BehaviorExclusions::from_selections([BehaviorExclusion::Service(
        semantic_vocabulary::ServiceId::new(1).unwrap(),
    )]);
    assert_eq!(
        admit_behavior_exclusion_closure(&exclusions, &receipt),
        Ok(())
    );
}
