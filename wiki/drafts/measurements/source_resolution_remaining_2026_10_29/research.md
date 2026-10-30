# Windows ancestry traversal research

2026-10-29. Question: can cache ancestry verification retain each verified parent
instead of reopening every prefix from the drive, preserving current pathname,
no-follow and object identity checks?

Reused the existing macOS implementation in `custody/platform.rs` and its
classified-directory replacement controls in `custody/tests/macos_acl.rs`.
The earlier stage-by-stage ledger explicitly preserves before/after cache
verification; that boundary remains unchanged here. Existing source-assembly
ownership research does not justify bypassing any custody check.

Exact search: `site.docs.rs cap_fs_ext DirExt open_dir_nofollow`. Read the
[official DirExt API](https://docs.rs/cap-fs-ext/latest/cap_fs_ext/trait.DirExt.html),
particularly `open_dir_nofollow`. Documentation version 4.0.3 matches Cargo.lock.
The operation is relative to an owned directory and refuses a symlink at its
named component. Supplying one component per step preserves the no-follow
boundary; passing an entire remaining suffix would not be equivalent.

Local source inspection: Windows `verify_cache_ancestry` calls
`open_absolute_directory_nofollow` for every ancestor, and that helper walks all
components again. The macOS helper already opens each child from its validated
parent. A Windows top-down walk can reuse this pattern while still classifying
each absolute pathname and comparing it with the newly opened child identity.
The final root checks and before/after resolver checks remain in place.

Candidate gate: normal nested directories, a child replaced after classification,
a replaced parent with a different child, and a junction ancestor; existing
focused custody tests; native image/exit equivalence and changed-source/invalid
source/damaged-record controls; three-workload unprofiled interleaved compilation.
No new dependency or permission change. Reject a bypass or unsupported behavior
change even if faster. Retain latency claims only where the observed spread
supports them; do not treat the ~100 ms root-retention profile as a guaranteed
whole-compilation saving.
