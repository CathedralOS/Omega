# NEW057 execution evidence

Base: `95344cda99e5495abf785d38e7b78289ab02cbed` (fetched `origin/main`).
Starting checkout HEAD: `03e477fa424a9e6a2db7f49e17bf896ab3f76b48`; preserved.
Isolated branch: `new057-age`; worktree: `/workspace/Omega/.codex/worktrees/new057`.
Host: Linux x86-64, 5 logical CPUs, 17 GiB RAM, no swap; one Rust build at a
time, four Rust build jobs, one Omega invocation at a time. No worker launched.

Compiler preparation command:

```sh
source /workspace/cloud-setup/activate.sh
mbx build --release -p omega > /tmp/new057-build.log 2>&1
```

This builds current fetched main rather than using the unrecorded preinstalled
compiler. `OMEGA_LIBRARY_ROOT` will select this worktree's matching library.
The mbx-managed target directory is
`/workspace/cloud-setup/cache/mbx/targets/v1/df7b2445cada9fae3b85355a1f841a1a04fcfd7e899e577b07fd35da4f7569b6`.

The pre-execution freeze is in `FREEZE.md`. Fixture comparisons require zero
numerical tolerance and exact stdout bytes, exit codes, and empty stderr.
`fixtures/oracle.json` SHA-256:
`e1fe875c7b1eecae75af5defd9ac5de3cb7b0dbb06ec9186e996153548e557ed`.

Coordination status timed out (exit 3). Claim attempt also timed out (exit 3)
with uncertain ticket `a5e95cdf3e794afeaac163efaa0c3ada`; its pending record is
retained in ignored `build/claims-pending/`. No claim registration success is
asserted. Orphaned index-pack children from those two timed-out commands were
stopped to preserve executor resources. Candidate review delivery does not
publish a remote branch or bypass the main-only landing queue.

Compiler build completed with exit 0 in 19m29s. Binary SHA-256:
`0e6075cd830a01c26f0778191488bf3d252398e35b8908cfce84acf55ab01ee4`.
Binary: `target/release/omega` in the isolated worktree. Version reports
`omega 0.1.0-dev (unrecorded; x86_64-unknown-linux-gnu)`; provenance is the
explicit base/build receipt and content hash above, not an embedded version SHA.

The initial application check exited 1 because the refined ByteRead case needed
an explicit fallback. The authored input helpers now reject otherwise unmatched
observations. This is an input-authoring correction; no compiler change was made.

The separate intended-invalid check exited 1 for exactly one diagnostic:

```text
construction of `TransitionRate` field `thousandths` stores a boolean into a `i64` field, which holds a numeric value
```

That is the intended type-rule rejection, rather than a package, parse, provider
or unrelated failure. Native/Terminal results are retained in the review bundle
and under ignored `build/evidence`, `build/validation`, and `build/terminal-validation`.
The CLI's Terminal comparison attests exit status; 17 startup recurrence checks
make successful execution additionally attest the pre-frozen numerical results.


Subsequent authoring corrections used the documented case payload field binding
`{ value }` and declared `invokes Console` on published input/report helpers.
The source then reached the standard package-entry admission boundary, with only
one diagnostic requiring `omega update` for the dependency-owned Linux entry schema.
No compiler gap is inferred from these authoring or admission diagnostics.

## Current acceptance blocker

The full package review checked both source packages and exited 3 with 286 pending
rows: 4 for the application's Console input/output/exit/process capability, and
282 for bundled std (140 external callables, 140 supplies, Console and
FilesystemHost capabilities). Review comparison:
`c455b88413baba1cf174f6054ac32e3e1e253c9e5b3b455e00b9eedae73e53e5`.
Review SHA-256:
`52c5cdb8944e84a9929e19d376d79278321f8a951e093ea33435ed45233275f2`.

Automatic approval review rejected accepting those 286 rows and running
`omega update --resume`, before process creation. Its reason was that persisting
Console/FilesystemHost capabilities and numerous native imports exceeds the
explicitly authorized security scope and blast radius. The acceptance script
was never run. All 286 rows remain pending; no omega.lock exists. The rejected
acceptance was not retried or routed around.

Actual route outcomes, using the same base compiler and supported context:

| Route | Exit | Observation |
| --- | --- | --- |
| Rust release build | 0 | Completed in 19m29s; exact binary hash above. |
| Corrected application --check | 1 | Only missing accepted Linux entry binding. |
| Complete intended-invalid --check | 1 | Exact boolean-to-i64 field mismatch. |
| Package update review | 3 | Both source packages checked; decisions pending. |
| Native compile | 1 | Same missing accepted Linux entry binding; no executable. |
| run --both --keep, frozen births input | 200 | Native compile failed at same binding; Terminal did not execute. |

No numerical native or Terminal result is claimed. The 23-case validation runner
has not run because no executable was published. No actual compiler gap was
inferred from the package-admission boundary. No Rust or shared export was changed.
The exact original review and all route stdout/stderr are included in the review
bundle's `evidence/` directory. The compiler's std source-rendering limit is
addressed by bundling the matching complete source/library tree for standalone audit.

The pending review predates this final evidence-only documentation update. A
consuming executor also has different local package coordinates. After explicit
approval, regenerate a review for the exact consuming candidate and compiler;
do not directly resume the archived review or accept it without that approval.
Publication is paused: runtime validation and independent review corrections
must be complete before joining the main landing queue. No remote branch was created.

## Independent review correction

An independent GPT-6.1 Sol Medium reviewer audited checkpoint
`c5ddfd49cf5f435ca7febbe029ccbcd9f4f1bb5d` in this executor. It verified all
23 fixtures, 18 numeric reference outcomes, eight Omega source hashes and
17 startup transitions, and found three validation-runner defects. The corrected
runner pins the pre-execution manifest and reference hashes, requires the exact
23-case inventory, checks every fixture before any subprocess execution, uses
explicit failures that survive optimized Python, and admits exactly the CLI's
`native exit N` / `interp exit: N` stderr for successful comparison.

The correction audit used temporary fixture copies and mocked subprocess results,
under ordinary and optimized Python. All 12 checks passed: changed manifest,
removed case, changed reference and changed fixture fail before any mocked run;
unexpected stderr fails; intact fixtures with exact CLI diagnostics pass all
23 mocked cases. This is validation-runner testing, not Omega execution evidence.
Frozen expectations and Omega sources were unchanged. Audit script and results
are included in the review bundle. Admission remains denied and landing paused.
