//! Canonical outer framing and cumulative recovery ceilings over real policies.

use omega_package_manager::lock::{PackageLock, PackageLockRecoveryLimits};

fn rejects(text: &str) {
    assert!(PackageLock::recover_text(text, PackageLockRecoveryLimits::default()).is_err());
}

pub(super) fn assert_canonical_framing(text: &str) {
    assert!(text.starts_with("omega_lock 1\ntargets 2\n"));
    rejects(&text.replacen("omega_lock 1\n", "omega_lock 99\n", 1));
    rejects(&text.replacen("omega_lock 1\n", "omega_lock 01\n", 1));
    rejects(&text.replace('\n', "\r\n"));
    rejects(&format!("{text}\n"));
    rejects(&format!("{text}end\n"));
    let target = text
        .lines()
        .find(|line| line.starts_with("target "))
        .unwrap();
    rejects(&text.replacen(target, "target omega.target-profile.v1:unknown", 1));
    rejects(&text.replacen(target, "target linux_x86_64", 1));
    rejects(&text.replacen("end_target\n", "end_unknown\n", 1));

    for label in ["targets", "source", "baselines", "baseline", "decisions"] {
        let prefix = format!("{label} ");
        let line = text.lines().find(|line| line.starts_with(&prefix)).unwrap();
        let count: usize = line[prefix.len()..].parse().unwrap();
        for altered in [
            format!("{label} 0{count}"),
            format!("{label} +{count}"),
            format!("{label} -1"),
            format!("{label} {}", count + 1),
            format!("{label} {}", count - 1),
            format!("{label} 184467440737095516160"),
        ] {
            rejects(&text.replacen(&format!("{line}\n"), &format!("{altered}\n"), 1));
        }
    }

    // A malicious byte length can split a UTF-8 code point. Recovery must
    // reject without invoking unchecked string slicing or reading a child.
    let source_header = text
        .lines()
        .find(|line| line.starts_with("source "))
        .unwrap();
    let source_start = text.find(&format!("{source_header}\n")).unwrap();
    let payload_start = source_start + source_header.len() + 1;
    let split_character = format!(
        "{}source 1\né{}",
        &text[..source_start],
        &text[payload_start + 1..]
    );
    rejects(&split_character);

    for length in [0, "omega_lock 1\n".len() - 1, text.len() - 1] {
        rejects(&text[..length]);
    }
    for (length, _) in text.match_indices('\n').step_by(53) {
        rejects(&text[..length]);
    }
}

fn minimum_owned(text: &str) -> usize {
    let mut low = 0;
    let mut high = PackageLockRecoveryLimits::default().maximum_owned_bytes;
    assert!(PackageLock::recover_text(text, PackageLockRecoveryLimits::default()).is_ok());
    while low < high {
        let middle = low + (high - low) / 2;
        let limits = PackageLockRecoveryLimits {
            maximum_owned_bytes: middle,
            ..PackageLockRecoveryLimits::default()
        };
        if PackageLock::recover_text(text, limits).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

pub(super) fn assert_aggregate_owned_boundary(lock: &PackageLock, text: &str) {
    let exact = minimum_owned(text);
    assert!(exact > 0);
    let limits = PackageLockRecoveryLimits {
        maximum_owned_bytes: exact,
        ..PackageLockRecoveryLimits::default()
    };
    assert_eq!(PackageLock::recover_text(text, limits).unwrap(), *lock);
    assert_eq!(lock.canonical_text_with_limits(limits).unwrap(), text);
    assert!(
        lock.canonical_text_with_limits(PackageLockRecoveryLimits {
            maximum_owned_bytes: exact - 1,
            ..limits
        })
        .is_err()
    );
    assert!(
        PackageLock::recover_text(
            text,
            PackageLockRecoveryLimits {
                maximum_owned_bytes: exact - 1,
                ..limits
            }
        )
        .is_err()
    );

    let largest_child = lock
        .targets()
        .iter()
        .map(|target| {
            let child = PackageLock::from_targets(vec![target.clone()]).unwrap();
            minimum_owned(&child.canonical_text().unwrap())
        })
        .max()
        .unwrap();
    assert!(
        exact > largest_child,
        "target sections must not reset the owned budget"
    );
    assert!(
        lock.canonical_text_with_limits(PackageLockRecoveryLimits {
            maximum_owned_bytes: largest_child,
            ..limits
        })
        .is_err()
    );
    assert!(
        PackageLock::recover_text(
            text,
            PackageLockRecoveryLimits {
                maximum_owned_bytes: largest_child,
                ..limits
            }
        )
        .is_err()
    );
}
