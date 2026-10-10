# NF15 interface and verification checkpoint

Assignment: reusable Householder QR, full-rank least squares, rank status,
explicit residual and orthogonality reports. Dependency owners: NF01, NF02,
NF09, NF11, NF12. Acceptance remains integrated Omega execution of both full
sample solves and a source ownership rejection on distinct routes.

Starting checkout: /workspace/Omega, branch work,
6e4f0c37ecd6dbb26d228b3045d40bc6b243d0bb, clean. Fetched origin/main and
created isolated /workspace/Omega/.codex/worktrees/nf15, branch nf15-householder,
base 9d4451128dd05885e7b9c591796b2760189851db. No source/library/numerics
exists in this base; no shared interfaces were invented. Read AGENTS.md,
.agents/skills/advance/SKILL.md, tools/claims.md, tools/testing.md, core slice
and float surfaces, std math, and existing sample/ownership fixtures.

Resources: Linux x86_64, 5 visible and affinity CPUs; initially 16 GiB memory
available and about 30 GiB free disk. resources.json gives measured counts,
bytes, available-memory value and load average. No parallel Rust build or Omega
execution was launched by this worker.

Claims status initially timed out. A claim with --board TASKS.md --item NF15
was rejected because NF15 is not on that board. The off-board delegated claim
then timed out with may_have_published=true and ticket
0d267bfe60d24be8b106649de0419b60; follow-up status also timed out. Registration
and active peer ownership could not be confirmed. Do not duplicate the claim
before resolving that ticket. Pending local record is under ignored
build/claims-pending/0d267bfe60d24be8b106649de0419b60.json.

Freeze evidence: oracle-freeze.sha256 was generated before reference.py ran;
all three JSON hashes still match. Full primary solutions/residuals were authored
with exact integer arithmetic before any numerical run. report tolerances and
rank cutoff were frozen at 1e-12. reference-results.json records two full-rank
runs and six variant cases. Both project drivers ran independently with exit 0;
Python source compilation and diff whitespace check also succeeded. The exact
Fraction normal-equation witnesses establish stationary residual and uniqueness
without using Householder or computing a floating-point normal-equation solve.

Important numerical decisions preserved in API requirements: scaled reflector
ratios avoid alpha-beta overflow, beta sign avoids cancellation, zero active
columns avoid division, Q construction reverses reflector application order,
and stopped unpivoted pivots report independent prefix rather than invented
matrix rank. Full-rank solve delegates to NF12. Original data and explicit
reflector scratch are distinct. Rank-deficient tables return unsupported status
rather than a perturbed or minimum-norm solution.

Initial route attempts failed before starting because the shell had not activated
the provisioned tools; route-attempts.json retains them. Discovery of
/workspace/cloud-setup/activate.sh provided the pinned nightly toolchain, mbx,
and provisioned Omega. activated-route-attempts.json retains each exact command,
binary hash, exit and full stderr. All 9 provisioned-binary attempts (check,
native, Terminal for both valid projects and invalid control) failed before
source validation because private resolver storage rejected cache ancestry `/`
owned by an unrelated user. Check/Terminal exits 1; native compile exits 200.
These are environment failures, not observed source/compiler failures and not
intended ownership rejection. The parent reported the same NF02 environment
failure. No system ownership, permission, or custody guard was changed.

Current source build: source /workspace/cloud-setup/activate.sh; mbx build -p
omega --release. Exit 0, release profile, 13m49s; full output is rust-build.log
and rust-build-result.json records revision/toolchain/binary hash. Retried all
9 routes with this just-built binary at 9d4451128d: identical cache-custody
failures before source validation, with check/Terminal exits 1 and native exits
200. current-route-attempts.json records all full commands and diagnostics.
No source rejection or scientific API acceptance was observed.

Missing contracts are enumerated with full semantic signatures in the owned
.api.txt file. Next action: parent distributes NF01 descriptor/package/checked
indexing, NF02 policy, NF09 stable norm, NF11 explicit transfer/reflector ownership,
and NF12 triangular solve interfaces. Then implement the owned .omg library and
replace oracle-only main.omg programs with reusable API calls, retaining every
frozen oracle and tolerance. The .omg library is intentionally not fabricated.
No integrated NF15 acceptance or publication is claimed at this checkpoint.
