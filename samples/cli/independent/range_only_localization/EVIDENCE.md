# ADD09 validation evidence

Base compiler source: `9a0463cc42af5f7178372c169ae55e528fe43878` (fetched main).
Isolated branch `add09-range`; worktree `/workspace/Omega/.codex/worktrees/add09`.
Compiler built directly from that worktree, with no Rust edits or other workers.

Build command:

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/add09-target CARGO_BUILD_JOBS=4 mbx build -p omega
```

Build exit 0, elapsed 3m26s, unoptimized dev profile, pinned
nightly-2026-09-04. Compiler binary `/workspace/add09-target/debug/omega`:
SHA256 `f2129021ac24244b42762cfe77417806ecaed0c9f369d986c05f6646205bfaa5`.
`--version`: `omega 0.1.0-dev (unrecorded; x86_64-unknown-linux-gnu)`.
The fresh isolated target and source revision establish identity even though the
version text lacks an embedded commit.

Executor: Linux x86_64, kernel 6.18.44, UID 1000, five CPUs,
18440136 KiB total memory. At most one Rust build and one Omega compiler/execution
process run at a time. `OMEGA_LIBRARY_ROOT` always selects this worktree's
`source/library`. The default sandbox rejected synthetic root ownership with
`cache custody ancestry is owned by an unrelated user`. The supported ordinary
per-command escalation was requested for compiler execution; no filesystem
ownership or custody guard was changed.

Oracle freeze occurred before Omega execution: 18 cases, manifest SHA256
`00b3e7636698202552def906938d3510be5c8656fca67628e6f2e4d540075a65`.
Reference set intersection confirms every manually constructed expected position;
continuous first-two-circle intersections are exactly (3,4) and (3,-4).
Integer output tolerance 0; circle witness tolerance 1e-12.

Actual acceptance results and artifact identities follow below when complete.

First authoring check rejected an ordinary call to internal state `digits` in
`number`; corrected to a normal state transition. This was an application
source error, not a compiler-gap claim. Command used the same binary and
`--check --target linux_x86_64`; captured log `/tmp/add09-check.log`.

The second check (`--check --timings --target linux_x86_64`) spent 228043187
microseconds checking std and 254420.641 ms total, then rejected the exported
formatter's omitted `invokes Console;`. Added that documented published ceiling.
This was an application declaration error, not a compiler defect.

Unoptimized `omega update --project ... --target linux_x86_64` produced no review
or diagnostic by the ten-minute attempt bound. The owned PID was terminated at
10m13s; shell exit 143 is a timeout observation, not a source rejection.
No custody, proof, provider or package-admission check was changed. An optimized
dev binary was then built from the identical compiler base with:

```sh
source /workspace/cloud-setup/activate.sh
CARGO_TARGET_DIR=/workspace/add09-opt-target CARGO_BUILD_JOBS=4 \
  CARGO_PROFILE_DEV_OPT_LEVEL=1 mbx build -p omega
```

The prior unoptimized build remains preserved in `/workspace/add09-target`.

Optimized dev build exit 0, elapsed 8m41s. Binary SHA256:
`81579b8ec3697d6ac9f2ccab123b44ec2a0b61164156a6b60d1f3f86046ab2ae`.
The per-command execution context reported UID 1000, root `/` UID 0, and
`/workspace` UID 1000. No custody guard was altered.

The first optimized check exposed a clause-separator authoring error; bodyful
`reaches Console` must precede `invokes Console;` without ending the machine
declaration. Corrected using existing bodyful corpus examples. The corrected
check took 35173.315 ms and reached the expected accepted-program-entry boundary:
`LinuxX86_64Application ... needs an accepted program-entry binding; run omega
update to review and accept it`. The normal review, not a hand-edited lock, is
used for this acceptance. A 180-second optimized review attempt timed out with
exit 124 and no diagnostic; the next attempt was bounded at 600 seconds.

The original 18-case oracle manifest remains byte-identical. A separately frozen
wide-band oracle admits all 121 points in [0,10]^2 under three [0,20] ranges;
this challenges multi-digit coordinates, counts and squared residuals. Five
protocol/window variants reuse the already frozen unique/error numerical output.

The 600-second review completed in approximately three minutes, checking two
source packages and generating a pending review. Inspection counted 286
pending rows: std 140 existing callable imports, 140 existing external supplies,
2 declared capability classes; application 3 Console terminal permissions and
1 Console process capability class. The application's permissions are input,
output and termination only. Accepting these project decisions does not assert
a full standalone audit of std's other exported platform imports.

Two Python bytecode files generated during oracle authoring were removed from
the private source tree. The normal resume guard rejected the changed source
snapshot, so that proposal was discarded and the clean source was re-reviewed.
No locked-source or source-consumption guard was weakened.

Admitted check exit 0: 27 source files, 35678.005 ms total, no output product.
Native production on original base 9a0463cc42 exited 2 (Incomplete), with:
`main has no executable body ... state graph: terminator: guarded jump
successors: loan transfer`, coverage_transition_forms limit 0 requested 1,
source coordinate 755 (main.omg line 18). This is the first native failure.
The original application is preserved; no expected output was weakened.
`repro/loan_transfer.omg` is a complete standalone control for borrowing a
local fixed byte array into a slice state parameter; its normal expected output
is `3456\n` and exit 0. Current main was fetched as
2f145503f24ecc1cac43588dc1c593dc069f9e7d before any compiler repair.
