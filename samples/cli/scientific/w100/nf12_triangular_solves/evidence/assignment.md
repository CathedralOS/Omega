NF12 / triangular solves. Source thread 01a0fe5c-7114-7783-9bdc-d3c9f48b60db.
Starting checkout /workspace/Omega, branch work, clean HEAD
6e4f0c37ecd6dbb26d228b3045d40bc6b243d0bb. Fetched main/base
26c92c17b8f4b21f39e14a5be57c85cfdec601de. Isolated branch
scientific/nf12-triangular-solves, worktree /workspace/Omega/.codex/worktrees/nf12.

Scope: owned pivot operations, API requirements, two complete sample sources,
one complete invalid control, independent frozen oracles. Rank-two reusable
entrypoint pending NF01 NF02 NF05 NF10 actual interfaces. No competing descriptor,
metadata, tolerance policy, slice implementation or factorization introduced.
Acceptance: same authored numerical result on check/native/Terminal routes;
precondition refusal for invalid control; exact zero before division.
Current frontier: dependency contracts absent, and package routes refuse cache
custody before source validation. Next: integrate owner signatures, obtain parent
legitimate custody environment configuration, rerun unchanged complete samples.

Ownership: `python tools/claims.py status` timed out with coordination unavailable.
`python tools/claims.py claim --board TASKS.md --item NF12 --owner 'NF12 triangular solves'
--path source/library/numerics/linalg/triangular_solves.omg
--path source/library/numerics/linalg/triangular_solves.api.txt
--path samples/cli/scientific/w100/nf12_triangular_solves --note ...` exited 1:
`Item NF12 not found in TASKS.md (no **<item>.** marker).` No claim registered.
No board or shared dependency ownership changed. Parent owns integration.

Environment: Linux x86_64, pinned rustc 1.100.0-nightly (a69a63265 2026-09-03).
First measurement: nproc=5, free -h reported 17 GiB RAM, 16 GiB available,
df -h reported 30 GiB available. At 2026-10-10T23:18:46Z:
MemTotal=18440136 KiB, MemAvailable=17384968 KiB;
cgroup cpu.max=400000 100000 (4 CPU quota), memory.max=17179869184 bytes;
free disk=28841193472 bytes; load averages 0.77, 0.71, 0.35.
One host build and one Omega execution at a time throughout.

Build command (cwd worktree):
`source /workspace/cloud-setup/activate.sh; CARGO_TARGET_DIR=/workspace/Omega/target mbx build -p omega`
Exit 0, finished dev build in 1m12s; raw log in compiler-build.log.

Library command (cwd worktree; activated environment):
`/workspace/Omega/target/debug/omega --check source/library/numerics/linalg/triangular_solves.omg`
Exit 0, compiled 25 source files. Exact source and stdout in library-check/.

Frozen mathematical certification:
`python samples/cli/scientific/w100/nf12_triangular_solves/verify_oracles.py`
Exit 0; 8 exact rational fixtures certified. Frozen SHA256:
91ca0766aedb08928308493e67643a8510236073786413342cb32569384d9679.
Tolerances were not changed after execution.

For sample route observations, routes-*.json holds exact executable argv,
source SHA256, cwd, base revision, elapsed time, host and exit. Full unchanged
failed sources are in *-source/*.omg.txt. Check exits 1; native/Terminal exits 200.
All package routes say:
`cannot open private source resolver storage: git cache entry '/' is invalid:
cache custody ancestry is owned by an unrelated user`.
This is environment refusal, not source validation or numerical failure.
System ownership, permissions and custody guards were not altered.

The unchanged invalid main and owned library source, copied to a standalone
source directory without build.omg, reject as intended. Command and full
sources are in direct-rejection/. This does not qualify the complete project
route. Exact diagnostic: cannot prove requires contract for call
triangular_checked_pivot from main: 0.0 != 0.0 (needs diagonal != 0.0).

Both source-only companions checked successfully (26 source files each). Their
exact changed provider context is documented in source-only-checks/command.txt.
Original hosted routes remain required and are never replaced by these checks.
An initial companion found state-level effect spelling invalid; sample states
now inherit their enclosing machine effect. Full initial source/diagnostics are
in source-only-companion-initial/. Before-fix package failures are preserved
separately in before-state-syntax-fix/.

Candidate branch publication initially refused GH007 email privacy. The actual
authenticated GitHub user was resolved with gh api user, and the same author was
amended to its verified public noreply address using a command-local git setting.
No global Git or account privacy setting changed. The resulting library candidate
1b12ff26a2 was pushed to scientific/nf12-triangular-solves; main was not touched.

The final upper source-only companion (unit diagonal avoids stored diagonal
reads) also checked 26 source files; its predecessor is preserved separately.
Final complete-project route recording is routes-all.json: all 7 routes
remain cache-custody refused before source validation. Native and Terminal
results remain unqualified; there is no numerical runtime success claimed.

Candidate review only. The descriptor-driven solve is not implemented while
owner contracts are absent. The branch/worktree are retained for parent
integration and the starting checkout remains clean at its original revision.
