# Package publication

Start at `../publication.rs`. This operation publishes accepted project files;
it does not certify packages or prove that anyone audited them.

```text
publication/
├── checked.rs       join staged source, target reviews, and exact decisions
├── transaction.rs   publish/recover the build.omg and omega.lock pair
├── journal.rs       bounded old/new byte framing; no selectable paths
├── directory.rs     retain project and ignored build/package-manager state
├── error.rs         distinguish pre-write failure from pending recovery
└── tests/           process interruption and filesystem behavior
```

`publish_reviewed_package_change` checks edit/stage/root identity, accepted lock
bytes, and complete target coverage. It compares against that baseline without
rerunning the compiler, requires each target's accepting decisions, rechecks
snapshots and live local dependency pins, and verifies unchanged original source.
Previously accepted target sections cannot be silently dropped.

Open `PackageFileTransaction` before reading an install/update baseline, recover
pending intent, and retain the guard while using the pair. `transaction.lock`
holds an OS mutex; its continued existence does not mean a process is running.
Contention returns `Busy`. Ordinary project preparation participates when state
already exists or an accepted package lock is present. A fresh locked checkout
creates coordination state before acquiring dependencies; unlocked source-only
reads do not create it.

The bounded `pending` journal records exact old/new bytes for the two fixed
files, including an explicitly absent old lock. Once recorded, it is commit
intent: recovery completes forward. Each live file must match its old or new
contents. A third-party edit stops recovery before further replacement and
retains the journal. Pre-intent failures leave the pair unchanged; `Pending`
means publication may have progressed and must be inspected or recovered.

Atomic stages stay under `build/package-manager`, not in source input.
Replacement preserves permissions, including the executable bit used in source
identity. Files are synchronized before publication; Unix directory sync errors
propagate. The Windows utility does not flush directory metadata, so it makes no
equivalent power-loss durability claim. Cross-device atomic operations fail
without a non-atomic copy fallback.

Two renames are not simultaneous for readers that ignore the mutex. This is
process coordination and interruption recovery, not protection from a hostile
process acting as the same user. Do not delete pending journals as build-cache
cleanup. [Package commands](../package_commands/README.md) own review-file
persistence/resume and selective update pin selection. Their `proposal` restart
record is separate from this publication journal.
