# NF03 assignment and interface checkpoint

Outcome: one reusable `Complex<T>` with explicit add, multiply, conjugate,
squared norm and scaled magnitude; two scientifically distinct complete sample
projects and one complete invalid alias control. Acceptance requires separate
check, native, Terminal comparison and intended-rejection observations.

Starting checkout: `/workspace/Omega`, clean branch `work`, revision
`6e4f0c37ecd6dbb26d228b3045d40bc6b243d0bb`.
Fetched current main / isolated base:
`9d4451128dd05885e7b9c591796b2760189851db`.
Worktree: `/workspace/Omega/.codex/worktrees/nf03`, branch `nf03-complex`.
Read `AGENTS.md`, `.agents/skills/advance/SKILL.md`, cloud coordination,
`tools/claims.md`, `tools/testing.md`, current module and Build contracts.

Ownership checks: `python tools/claims.py status` returned exit 3 after its
15-second coordination deadline. Ownership is unconfirmed. Claim attempt
`python tools/claims.py --timeout-seconds 5 claim --board TASKS.md --item NF03
--owner 'NF03 complex carrier ops delegated worker' --path source/library/numerics/complex
--path samples/cli/scientific/w100/nf03_complex_carrier_ops --note 'Own reusable
Complex<T> and two distinct samples plus invalid control; NF02 numerical policy
dependency; qualification pending'` returned exit 1: NF03 has no board marker.
No successful claim is asserted and no shared board was modified.

Measured executor: `nproc` = 5. Startup `free -h`: 17 GiB total, 706 MiB used,
16 GiB available, no swap. Startup `df -h /workspace`: 32 GiB filesystem,
30 GiB free. During the sole Rust build, four rustc processes each used about
99% CPU; their RSS readings were 1,342,724 / 329,920 / 498,220 / 310,412 KiB.
`free -m` then reported 14,858 MiB available; disk had 29,726 MiB available.
These are measured instantaneous observations, not peak-resource claims.

Dependencies: NF02 policy interfaces absent; exact requested consumer signatures
and policy questions are in `complex_carrier_ops.api.txt`. NF01 owns the absent
`source/library/numerics/build.omg`; sample requester alias is `numerics`.
No competing package metadata or shared numerical-policy implementation is added.
Temporary qualification metadata only lives in the retained harness snapshot.

Environment failure: ordinary sandbox Omega commands stop before source
validation with `git cache entry '/' is invalid: cache custody ancestry is owned
by an unrelated user`. `ls -ld /` reports owner `nobody:nogroup` in that sandbox.
No ownership, permissions, or custody checks were changed. Approved execution
outside the remapping got past this check. Preflight with the old bundled
compiler then rejected static Build syntax; that is distinct from new-main
qualification. See retained command/log snapshots for exact results.

Next necessary action: build fresh current-main compiler, qualify owned sources
sequentially in scratch metadata, record each route, then integrate actual NF01
metadata and NF02 signatures once distributed. Candidate commits are for parent
review; no main publication is claimed.
