# Package publication

Start at `../publication.rs`. This operation writes accepted project files; it
does not certify packages or prove anyone audited them.

```text
publication/
├── checked.rs       join staged source, target reviews, and exact decisions
├── checked_error.rs checked-publication errors
├── transaction.rs   publish/recover the build.omg and omega.lock pair
├── journal.rs       bounded old/new byte framing; no selectable paths
├── directory.rs     retain project and ignored build/package-manager state
└── error.rs         distinguish pre-write failure from pending recovery
```

`publish_reviewed_package_change` checks edit, stage and root identity,
accepted lock bytes and complete target coverage. It compares against that
baseline without rerunning the compiler, requires each target's accepting
decisions, rechecks snapshots and live local dependency pins, and verifies the
original source is unchanged. It never silently drops an accepted target section.

Open `PackageFileTransaction` before reading an install/update baseline,
recover any pending write, and hold the guard while using the pair.
`transaction.lock` holds an OS mutex; its existence does not mean a process
is running. Contention returns `Busy`. Ordinary project preparation joins in
when the state or an accepted lock exists. A fresh locked checkout
creates it before fetching dependencies; unlocked source-only reads do not.

The bounded `pending` journal records exact old and new bytes of the two
files, including an explicitly absent old lock. Once written, it commits the
change: recovery finishes it forward. Each file must match its old or new
contents; another party's edit stops recovery and keeps the journal. A failure before the journal leaves the pair
unchanged; `Pending` means publication may have progressed and must be
inspected or recovered.

Atomic stages stay under `build/package-manager`, not in source input.
Replacement keeps permissions, including the executable bit in source
identity. Files are synced before publication; Unix directory sync errors are
reported. Windows does not flush directory metadata, so it promises no
equivalent power-loss durability. Cross-device atomic operations fail with no
non-atomic copy fallback.

A reader ignoring the mutex may see one rename without the other. This
coordinates processes and recovers interruptions; it does not stop a hostile
process running as the same user. Do not delete pending journals as
build-cache cleanup. [Package commands](../../../README.md#install-and-update)
save and resume review files and pick pins for selective updates. Their `proposal` restart
record is separate from this publication journal.
