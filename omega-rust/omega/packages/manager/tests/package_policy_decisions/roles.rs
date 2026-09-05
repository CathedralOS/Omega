use super::*;
use omega_package_manager::lock::HistoricalPackagePolicyDecisionSubject;
use omega_package_manager::review::{PackagePolicyDecisionSubject, ReviewOnlyRootRoleContract};

#[test]
fn directional_root_role_obligations_are_resolved_and_recorded_beside_row_choices() {
    let tree = Tree::new();
    source(
        &tree,
        "data Main { }\nmachine Main::main(&mut self) { }\n",
        "",
    );
    let (package_sources, package_reviews) = candidate(&tree, "package");
    let package_lock = lock_from_reviews(&package_sources, &package_reviews);
    fs::write(
        tree.path("sources/root/build.omg"),
        concat!(
            "machine build(builder: &mut Build) {\n",
            " builder.application(\"policy-fixture\");\n",
            " builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);\n",
            "}\n",
        ),
    )
    .unwrap();
    let (application_sources, application_reviews) = candidate(&tree, "application");
    let application_lock = lock_from_reviews(&application_sources, &application_reviews);
    for (accepted, sources, reviews, expected) in [
        (
            &package_lock,
            &application_sources,
            &application_reviews,
            ReviewOnlyRootRoleContract::DependencyCompatibility,
        ),
        (
            &application_lock,
            &package_sources,
            &package_reviews,
            ReviewOnlyRootRoleContract::ApplicationActivation,
        ),
    ] {
        let changes = compare(accepted.target(TARGET), sources, reviews);
        let obligations = changes
            .decision_obligations(PackagePolicyDecisionLimits::default())
            .unwrap();
        let role = obligations
            .iter()
            .find(|obligation| {
                matches!(
                    obligation.subject(),
                    PackagePolicyDecisionSubject::RootRole { .. }
                )
            })
            .unwrap();
        assert!(
            matches!(role.subject(), PackagePolicyDecisionSubject::RootRole { broken_contract, .. } if broken_contract == expected)
        );
        let mut supplied = decisions(&changes, ACCEPT);
        supplied.retain(|decision| decision.obligation().fingerprint() != role.fingerprint());
        assert!(
            resolve_package_policy_decisions(
                &changes,
                &supplied,
                PackagePolicyDecisionLimits::default()
            )
            .is_err()
        );
        supplied.push(changes.policy_decision(role, REJECT).unwrap());
        let resolved = resolve_package_policy_decisions(
            &changes,
            &supplied,
            PackagePolicyDecisionLimits::default(),
        )
        .unwrap();
        assert!(!resolved.all_required_changes_accepted());
        let lock = history_lock(sources, reviews, &changes, &resolved);
        let target = lock.target(TARGET).unwrap();
        let recorded = target
            .decisions()
            .decisions()
            .iter()
            .find(|decision| {
                matches!(
                    decision.subject(),
                    HistoricalPackagePolicyDecisionSubject::RootRole { .. }
                )
            })
            .unwrap();
        assert_eq!(recorded.disposition(), REJECT);
        let HistoricalPackagePolicyDecisionSubject::RootRole {
            package_index,
            broken_contract,
            ..
        } = recorded.subject()
        else {
            unreachable!()
        };
        assert_eq!(*broken_contract, expected);
        assert_eq!(
            target.source().packages()[*package_index].key(),
            sources.graph().root()
        );
    }
}
