//! Compact consent rows are charged across every target section.

use package_manager::lock::{PackageLock, PackageLockRecoveryLimits};

pub(super) fn assert_aggregate_acceptance_boundary(lock: &PackageLock, text: &str) {
    let per_target = lock
        .targets()
        .iter()
        .map(|target| {
            target
                .baselines()
                .iter()
                .map(|baseline| baseline.rows().len())
                .sum::<usize>()
        })
        .collect::<Vec<_>>();
    let exact = per_target.iter().sum::<usize>();
    let largest_target = *per_target.iter().max().unwrap();
    if exact == 0 {
        let limits = PackageLockRecoveryLimits {
            maximum_policy_elements: 0,
            ..Default::default()
        };
        assert_eq!(PackageLock::recover_text(text, limits).unwrap(), *lock);
        assert_eq!(lock.canonical_text_with_limits(limits).unwrap(), text);
        return;
    }
    assert!(exact > largest_target);
    let limits = PackageLockRecoveryLimits {
        maximum_policy_elements: exact,
        ..PackageLockRecoveryLimits::default()
    };
    assert_eq!(PackageLock::recover_text(text, limits).unwrap(), *lock);
    assert_eq!(lock.canonical_text_with_limits(limits).unwrap(), text);
    for maximum_policy_elements in [0, exact - 1, largest_target] {
        let limited = PackageLockRecoveryLimits {
            maximum_policy_elements,
            ..limits
        };
        assert!(PackageLock::recover_text(text, limited).is_err());
        assert!(lock.canonical_text_with_limits(limited).is_err());
    }
    for (target, maximum_policy_elements) in lock.targets().iter().zip(per_target) {
        let child = PackageLock::from_targets(vec![target.clone()]).unwrap();
        let child_text = child.canonical_text().unwrap();
        let child_limits = PackageLockRecoveryLimits {
            maximum_policy_elements,
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
