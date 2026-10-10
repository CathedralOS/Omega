# NF14 assignment and interface checkpoint

Outcome: SPD Cholesky factorization and solve, rank-two column-major descriptors,
explicit backing slices, checked/try errors, explicit allocation and copying.
Owned paths: this sample tree and the two cholesky_spd_solve library files.
Dependencies: NF01 descriptor/package, NF02 accuracy policy, NF11/NF12 matrix
copy and triangular solve contracts (parent must confirm which owner supplies which).

Started 2026-10-10 UTC. Starting checkout /workspace/Omega, clean branch work at
6e4f0c37ec. Fetched main and isolated branch nf14-cholesky at
26c92c17b8f4b21f39e14a5be57c85cfdec601de, worktree
/workspace/Omega/.codex/worktrees/nf14. CPU count 5; MemTotal about 17 GiB,
available 16 GiB, no swap; free disk 30 GiB. Claims status returned coordination
unavailable (Git deadline); subsequent NF14 claim returned item not found in
TASKS.md. No ownership registration claimed. NF14 ownership is the explicit
delegation. No shared boards or dependency-owner files edited.

Current main has no source/library/numerics package, descriptors, copy or
triangular solver. Existing sqrt signature is
F64::square_root(value: f64) -> f64 in core::float_operations.
Exact dependency requirements are in cholesky_spd_solve.api.txt.
Independent work: freeze exact integer-derived factor/solution oracles and
sample arithmetic designs, exercise existing sqrt and borrowed/copied storage,
and implement the owned scalar pivot helper. Full descriptor API integration
remains pending the dependency owners; independent samples do not establish it.

Acceptance: each sample must check, execute natively with authored exit 0,
compare Terminal execution separately, and the invalid control must reject for
mutation of readonly storage. Keep commands, statuses and diagnostic logs.
Parent independently reviews candidate commits and performs normal landing.

Candidate publication initially rejected GH007 (private configured email).
Authenticated GitHub account was verified; unpublished NF14 commits were
reauthored using its official noreply address, preserving all source trees.
The public oracle freeze commit is 276f4aa16b; original 0f6ccf05b5 exists in
the local prepublication bundle. Both freezes precede any Omega execution.

Verification frontier: release Rust build exit 0 (6m35s); pivot library --check
exit 0; independent rational fixtures pass. Full sample check/native/Terminal
and complete invalid control all encounter the same host cache-custody ancestry
refusal as an existing float baseline. Two preserved source-only mathematical
probes check; source-only readonly mutation rejects for the intended reason.
No system/custody changes made. Exact route commands and logs in evidence/.
Next: receive NF01/NF02/NF11/NF12 canonical signatures and legitimate environment
configuration, integrate descriptor factor/solve, then execute all full routes.
Parent retains integration and normal landing responsibility.

Refreshed main after validation: f9700f52dac1dce6ee8fe340e0d282c454aa3295. Its tree still contains no
source/library/numerics or scientific/w100 dependency-owner interfaces.
Candidate retains the recorded base for review; no stale-base validation is
described as verification of refreshed main.
