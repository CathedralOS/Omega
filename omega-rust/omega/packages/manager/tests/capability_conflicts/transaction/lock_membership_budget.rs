//! Semantic-owner work is charged across every baseline and target section.

use omega_package_evidence::encoding::PackagePolicyMembershipLimits;
use omega_package_manager::lock::{PackageLock, PackageLockRecoveryLimits};

pub(super) fn assert_aggregate_identity_boundary(lock: &PackageLock, text: &str) {
    let per_target = lock
        .targets()
        .iter()
        .map(|target| {
            target
                .baselines()
                .iter()
                .map(|baseline| {
                    baseline
                        .validate_package_membership(
                            |identity| {
                                target
                                    .source()
                                    .packages()
                                    .iter()
                                    .any(|package| package.key().identity() == identity)
                            },
                            PackagePolicyMembershipLimits::default(),
                        )
                        .unwrap()
                        .identity_nodes()
                })
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    let exact = per_target.iter().sum::<usize>();
    let largest_target = *per_target.iter().max().unwrap();
    assert!(largest_target > 0);
    assert!(exact > largest_target);
    let limits = PackageLockRecoveryLimits {
        maximum_identity_nodes: exact,
        ..PackageLockRecoveryLimits::default()
    };
    assert_eq!(PackageLock::recover_text(text, limits).unwrap(), *lock);
    assert_eq!(lock.canonical_text_with_limits(limits).unwrap(), text);
    for maximum_identity_nodes in [0, exact - 1, largest_target] {
        let limited = PackageLockRecoveryLimits {
            maximum_identity_nodes,
            ..limits
        };
        assert!(PackageLock::recover_text(text, limited).is_err());
        assert!(lock.canonical_text_with_limits(limited).is_err());
    }
    for (target, maximum_identity_nodes) in lock.targets().iter().zip(per_target) {
        let child = PackageLock::from_targets(vec![target.clone()]).unwrap();
        let child_text = child.canonical_text().unwrap();
        let child_limits = PackageLockRecoveryLimits {
            maximum_identity_nodes,
            ..limits
        };
        assert_eq!(
            PackageLock::recover_text(&child_text, child_limits).unwrap(),
            child
        );
        assert_eq!(
            child.canonical_text_with_limits(child_limits).unwrap(),
            child_text
        );
    }
}
