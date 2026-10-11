# NEW078 validation evidence

This checkpoint preserves the complete application and its unchanged frozen
oracle while a necessary compiler dependency is investigated. It is not runtime
acceptance or a claim that the application is landed.

Initial base: `b4008bb3c7894aebad47eaccf8af12bb4a8006b1`.
Worktree: `/workspace/Omega/.codex/worktrees/new078`, branch `new078-mold`.
Original checkout `/workspace/Omega` was preserved.

The independent set/graph reference and selected expected files were frozen
before the first Omega check. `oracle.sha256` still matches. Exact integer and
byte tolerance is zero. The independent path enumeration covers 256 flood
states; runtime acceptance requires 10 selected, 12 invalid and 619 tiny cases.

The base compiler SHA-256 is
`ae086d2061992b4f1529ee77633190bb8df2f0792b170ce8f2fae4a853cbf864`.
It rejects the complete `controls/local_array_loan` native source at loan
transfer. A bounded carrier registration repair permits that control to realize.
The repaired final compiler SHA-256 is
`8a3036046c71c256baab19977e877acb15db7d7fb3f38f6524ca77f259d93ff8`.

Scoped Clippy completed successfully. The checked corpus gate is clean for
shared byte-array reads, exclusive writes and the shared/exclusive loan rejection
control (2 checked, 1 rejected with its expected fragment). The intended-invalid
`arrival[16]` control rejects with exactly one bounds diagnostic, and the valid
`arrival[15]` companion checks successfully.

Ordinary package review inventoried and accepted 286 exact rows for the local
application and unchanged bundled standard library. Root terminal permissions
are input, output and process exit. The standard library's broader public API
and external ABI disclosures do not grant root filesystem execution permission;
opaque external supplies remain trust disclosures, not checked implementations.
No assumed-binding-contract findings or acceptance denials occurred.

Application `--check --target linux_x86_64` succeeds with 28 source files.
Native emission currently fails with
`RegisterAllocation(RuntimeSpill(Homes(NoCommonCandidate { function: 18, register: 8 })))`.
Native runtime and Terminal runtime acceptance are pending this physical staging
dependency. The full source and unchanged output checks remain in place.

The default compiler context failed cache custody on synthetic root ownership.
Approved per-command `require_escalated` execution retained UID 1000 and real root
ownership; no ownership or custody guards were changed. After a disconnect
notification, successful read-only execution, preserved session observation,
worktree status and frozen hash checks verified the same executor remained ready.
One Rust build and one Omega execution were active at most; corpus jobs were one.

Detailed command logs, exact production artifact metadata and local review files
are retained for the review bundle. Generated outputs and host-local locks remain
untracked under ignored `build/` paths.
