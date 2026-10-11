# NEW077 validation record

Host: Linux x86_64, UID 1000, five exposed CPUs, ~18 GB RAM, no swap.
Compiler base: `b4008bb3c7894aebad47eaccf8af12bb4a8006b1` (fetched main).
Starting checkout `03e477fa424a9e6a2db7f49e17bf896ab3f76b48` was preserved.
Candidate worktree: `/workspace/Omega/.codex/worktrees/new077`, branch `sample/new077`.

A single Rust build ran at a time, with `CARGO_BUILD_JOBS=4`, private
`CARGO_TARGET_DIR=/workspace/new077-target`, pinned nightly-2026-09-04.
Actual build command (Cargo invoked directly):

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/new077-target CARGO_BUILD_JOBS=4 cargo build -p omega
```

Build exit 0, 2m29s, debug/unoptimized. The existing release binary was
unrecorded and could not establish this base identity, so was not reused.
Compiler `/workspace/new077-target/debug/omega` SHA-256:
`4a4d1e51770eae3313ee816ad3e97f7fec1707e9e674aaeb26f1fa7c7c1ff7c8`.
The compiler and libraries match the base; no Rust or shared export changes.

Every compiler invocation selected the candidate checkout's stable library:
`OMEGA_LIBRARY_ROOT=/workspace/Omega/.codex/worktrees/new077/source/library`.
Environment activation selects the existing XDG cache/config/data directories.
Only one Omega compilation/execution runs at a time.

The first ordinary sandbox `omega --check .../main.omg` reported cache
custody rejection (compiler status not separately retained by that initial shell): `git cache entry / is invalid: cache custody ancestry is owned by an
unrelated user`. The sandbox exposed root ownership as 65534. A per-command
`require_escalated` check was approved and reached source resolution without
changing ownership or custody guards. The first such attempt found a wrong
relative std dependency path, corrected from five parent components to four.

Two later checks reached application checking and rejected authoring errors:
non-tail self-recursion in whitespace readers (replaced with state loops), then
missing blocking acknowledgements on parser helper calls (added `block`).
A later check rejected the acknowledgement in a transition arm; it was moved into an ordinary finishing state. None of these findings justified a compiler repair. Frozen fixture expectations were
not changed. Final source validation and execution results follow below.

`python3 tools/claims.py status` exceeded its coordination deadline. No claim
registration was reported; isolated scope is retained in `ASSIGNMENT.md`.
No remote worker branch was created, and no main publication was attempted.

Frozen oracle was authored before any Omega check or execution. Manifest SHA-256:
`d4f566be2f960fa708f16d9cd039950c65c4c890969d258e3245ddd8d1950a6e`.
All nine input and output hashes were verified against it after authoring fixes.
The numerical tolerance is zero. Source hashes are retained in `SOURCE_SHA256.txt`.

After recovery, the supported environment status returned `ready`. Base, worktree,
compiler and frozen oracle were intact; original package-update process PID 19036
and session 62368 were preserved and monitored to completion. That preparation
exited 1 with 11 mutable/read-only forwarding diagnostics. Scalars were staged
before mutable forwarding per documented argument evaluation order. Independent
GPT-6.1 Sol / Medium review accepted the corrections; see `REVIEW.md`.

The corrected application `--check` exited 1 with one remaining diagnostic:
`linux_x86_64::ProgramEntry` uses the dependency's `LinuxX86_64Application` schema
and requires an accepted program-entry binding. No application authoring
diagnostic remained. This is not a complete check pass.

Separate complete intended-invalid control `--check` exited 1 with exactly one
intended diagnostic: `cannot prove index __hoist_14 is within length 256`.
The source's only indexed site selects value 256 in a length-256 owner; no
package, parser, unrelated type or provider rejection was counted as success.

The next preparation exited 1 because documentation updates changed the project
while its immutable local-source snapshot was being captured. No review or
acceptance resulted. This was a workflow error by the primary, not a compiler
gap or a denied acceptance. The full sample tree will stay fixed during subsequent
compiler runs; logs/receipts are written only to ignored build output.

Incoming main `a8c42a443795d5aeb6f1a299ac7261fe3c548801` changes compiler and core
library inputs. Integrated validation therefore requires a newly built compiler.
The planned profile uses `CARGO_PROFILE_DEV_OPT_LEVEL=1` per AGENTS.md, a private
target directory, four Rust build jobs, and one Omega execution at a time.
