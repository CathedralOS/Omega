//! Full policy survives loss of the source checkout, cache, and compiler state.

use super::*;
use omega_package_manager::lock::{
    HistoricalPackagePolicyDecisions, HistoricalPackagePolicyLimits, PackageLock, PackageLockError,
    PackageLockRecoveryLimits, PackageLockTarget,
};
use omega_package_manager::review::compile_resolved_package_reviews;

#[path = "package_lock/owners.rs"]
mod owners;

#[test]
fn complete_diamond_lock_recovers_without_any_old_source_or_compiler_state() {
    let (original, text, removed_fixture) = {
        let tree = TempTree::new();
        let closure = resolve_diamond(&tree, "package");
        let target = closure.for_exact_target(TargetProfile::WindowsX64);
        let reviews = compile_resolved_package_reviews(&target, &tree.path("build")).unwrap();
        let source = subject_for(&closure, TargetProfile::WindowsX64);
        let baselines = source
            .packages()
            .iter()
            .map(|source| reviews.review(source.key()).unwrap().policy().clone())
            .collect::<Vec<_>>();
        assert_eq!(baselines.len(), 4);
        let decisions = HistoricalPackagePolicyDecisions::recover_text(
            &format!(
                "omega-policy-decisions 1\nsource {}\ndecisions 0\nend\n",
                source.fingerprint().to_hex()
            ),
            &source,
            HistoricalPackagePolicyLimits::default(),
        )
        .unwrap();

        let mut missing = baselines.clone();
        missing.pop();
        let mut reordered = baselines.clone();
        reordered.swap(0, 1);
        let mut foreign = baselines.clone();
        foreign[0] = baselines[1].clone();
        let mut additional = baselines.clone();
        additional.push(baselines[0].clone());
        for incomplete in [missing, reordered, foreign, additional] {
            assert_eq!(
                PackageLockTarget::from_parts(source.clone(), incomplete, decisions.clone()),
                Err(PackageLockError::BaselineCoverage),
            );
        }
        let lock = PackageLock::from_targets(vec![
            PackageLockTarget::from_parts(source, baselines, decisions).unwrap(),
        ])
        .unwrap();
        let text = lock.canonical_text().unwrap();
        (lock, text, tree.0.clone())
        // Drop releases reviews/resolver custody and removes the test-owned
        // source tree, immutable cache, and build directory before recovery.
    };
    assert!(
        !removed_fixture.exists(),
        "the source and cache must actually be unavailable"
    );
    let recovered = PackageLock::recover_text(&text, PackageLockRecoveryLimits::default()).unwrap();
    assert_eq!(recovered, original);
    assert_eq!(recovered.canonical_text().unwrap(), text);
    let target = recovered.target(TargetProfile::WindowsX64).unwrap();
    assert_eq!(target.source().packages().len(), 4);
    assert_eq!(target.source().dependency_requests().len(), 4);
    assert_eq!(target.baselines().len(), 4);
    for (source, baseline) in target.source().packages().iter().zip(target.baselines()) {
        assert_eq!(baseline.package(), source.key().identity());
        assert_eq!(baseline.target(), TargetProfile::WindowsX64);
    }
    for limits in [
        PackageLockRecoveryLimits {
            maximum_bytes: text.len() - 1,
            ..PackageLockRecoveryLimits::default()
        },
        PackageLockRecoveryLimits {
            maximum_owned_bytes: 0,
            ..PackageLockRecoveryLimits::default()
        },
        PackageLockRecoveryLimits {
            maximum_packages: 3,
            ..PackageLockRecoveryLimits::default()
        },
        PackageLockRecoveryLimits {
            maximum_dependency_requests: 0,
            ..PackageLockRecoveryLimits::default()
        },
        PackageLockRecoveryLimits {
            maximum_policy_elements: 0,
            ..PackageLockRecoveryLimits::default()
        },
    ] {
        assert!(PackageLock::recover_text(&text, limits).is_err());
        assert!(recovered.canonical_text_with_limits(limits).is_err());
    }
}
