# NEW100 validation record — blocked candidate

This is a complete authored application candidate, not an accepted or landed workflow.
No Rust or standard-library source changes are included.

Compiler base: `c24b141cb29b311e92de0a75b9609ec37e9bf28a` (main-only fetch).
Compiler SHA-256: `1376ed5a535e35404a7dda8cf6418690e5d225a77c0138b7ad91175befec73b3`.
Actual binary: `/workspace/cloud-setup/cache/mbx/targets/v1/2c1af08c93ee9b14a8c18d5567bacd6d03c087d064bf2e5ed9b03812dd2bcc55/release/omega`.
The isolated worktree's `target/release/omega` points to this binary.
One `mbx build -p omega --release` succeeded, using nightly-2026-09-04,
CARGO_BUILD_JOBS=4, five logical CPUs and 18007 MiB reported memory; elapsed 21m09s.
Every subsequent compiler execution ran serially. Compiler source remained at base.

The default sandbox check rejected synthetic root ownership (UID 65534).
Individually approved require_escalated executions retained UID 1000 and observed
actual root ownership UID 0. No custody guard or ownership was modified.
No automatic approval review rejected an execution request.

The oracle manifest and all 42 input/output hashes were frozen before any Omega
execution. Numerical tolerance is zero. The independent Python set oracle matches
all nine valid outputs. Independent GPT-6.1 Sol / Medium source review checked
4,681 term tables, 37,448 assignment cases and 224,688 intervention cases without
a blocking correctness finding. It also checked all 21 fixture classifications.
Its small separator-grammar documentation correction is included.

Three initial checks exposed ordinary authoring errors, all corrected: state-level
reach clauses, an Exact cursor fact invalidated by a call, and an omitted helper
invocation ceiling. Subsequent check reached the required accepted ProgramEntry
binding gate. There is no demonstrated compiler defect and no compiler repair.

Package commands (repository root; OMEGA points to verified binary):

```sh
"$OMEGA" update --project samples/cli/independent/correlated_fault_tree_audit --target linux_x86_64
"$OMEGA" update --resume --project samples/cli/independent/correlated_fault_tree_audit
```

Initial update exited 3 with a pending review after checking the app and unchanged
local std. Std source rendering exceeded its output limit; its exact local sources
were available and unchanged from base. The recorded review contains 286 decisions
(3 terminal permissions, 3 dangerous capabilities, 140 callables, 140 external
supplies). The application binds only Console, not Filesystem. Existing std's
broader cross-target catalog is explicitly recorded in the evidence bundle.

One digest-pinned resume attempt exited 1 with:

```
local source `<isolated project>` changed while its immutable snapshot was being captured
```

The agent edited README during capture. README is captured input; the source-drift
guard independently matches this diagnostic. This was an avoidable authoring
process error, not a compiler defect. Logs identify the project, not the exact file;
README attribution also relies on the edit timing. The rejection was preserved;
package acceptance was not retried, discarded, bypassed or weakened.

Final validation commands are the four README validate.py routes. Check exited 1,
native exited 200 at the first fixture, and Terminal exited 200 at the first fixture.
The application routes are blocked by the missing accepted
`linux_x86_64::ProgramEntry` / `LinuxX86_64Application` binding. No application
native image or successful Terminal replay was produced; remaining runtime cases
are frozen expectations, not executed passes.

The separate complete intended-invalid control passed: --check exited 1 with
`cannot prove index 3 is within length 3`. Its validator excludes wrong-reason
package/custody rejection. Full stdout/stderr and timing records are retained in
the supported Library bundle.

Main landing is blocked by the unmet application execution gates. No landing
queue ticket was enqueued and no main push was attempted. Claims coordination
originally timed out with mutation uncertainty (ticket
`e1318a1999f840f0871122b22fb0cfee`); registration is not claimed successful.
The original worktree remains preserved. This namespace is isolated on
`new100-correlated` for independent review.
