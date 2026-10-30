# Cache independence, 2026-10-28

No stale acceptance or native-output mismatch was found in **34 warm/fresh
comparisons and 56 native executions** on Windows x86-64. This establishes the
named regression cases below, not complete dependency tracking or a proof of
cache soundness. No production compiler code changed in this round.

The tested producer is the optimized `67358c0345be...` compiler used for the
[Cathedral desktop boot](../full-boot/README.md). The compiler-switch control is
the distinct optimized `fce1b143fc4c...` shared-UI producer. Full binary hashes
and optimization profiles are in [result.json](result.json). The source base
before adding this gate is `abc9757e1e8`.

## Observations

- All six unchanged projects reported retained-check hits (two per project,
  three for the declared dependency), rather than merely having a warm cache.
- Body edits, changed entry selection, and transitive module edits produced
  the authored trap in both cache states, then returned to exit zero after
  restoration. Cached old code would have returned zero during mutation.
- Tightening an imported contract and a declared path-package contract
  rejected the same call in both cache states. Both edits preserved file size
  and modification time. Restoring the contract restored acceptance.
- Deleting an imported module refused in both states; recreating it recovered.
- Ordinary compilation refused changed source/compiler identities under the
  previous lock. Only explicit update renewed the acceptance used afterward.
- Truncated records, damaged payloads, wrong compiler identity, and wrong
  format magic were observed as invalid cache records. Missing records and
  orphan temporary writes also recovered with the same native output.
- A writer was killed after its unpublished temporary record reached 40 bytes.
  The previously published record remained unchanged. Subsequent compilation
  of the edited source matched a fresh build and executed the trap.
- Switching producers created separate retained-source record namespaces;
  switching back agreed with fresh compilation.

There were 24 accepted pairs and 10 rejected pairs. Every accepted pair had
identical native image hashes and exit/stdout/stderr. Rejected pairs had the
intended matching diagnostics and no image in their fresh output directories.
Only the known cache-root prefix was normalized in diagnostics.

## Replay and harness checks

Run the [documented gate](../../../../../tools/testing.md#cache-independence)
with two distinct optimized producer identities and a new output directory.
All generated projects and cache roots are disposable. The user's real cache
and original checkouts were not mutated. Native outputs and full logs remain
locally under `build/cache-independence-03`.

After the full run, review settlement was tightened to require the exact review
path emitted by the current invocation; the benchmark helper's fallback to an
older review file is inappropriate here. Three focused harness tests passed,
including failed update, missing reported review, and refused resume controls.
A real declared-package comparison in two initially empty cache roots passed
with the final harness, adding two native executions. The existing
`shared_compiler_rechecks_changed_dependency_contract_and_recovers_after_refusal`
test also passed through `mbx nextest run --release -p omega-build --test
shared_compilation` (one test, exit zero).

[observations.tar.gz](observations.tar.gz) retains commands, source snapshots,
native hashes/results, diagnostics, counters, and focused test logs. It includes
the exact harness for each run, bound to the recorded SHA256, and the final
focused replay script. Native image bytes and producer binaries remain local;
they are not duplicated in this archive.

An initial harness run stopped on different cache-root spellings in identical
missing-file diagnostics; this was a comparison defect, not a compiler mismatch.
The first PowerShell test wrapper also remained open after nextest reported
success; its childless wrapper was terminated, then the focused test was rerun
with direct file logging and a verified zero process exit. Only the latter is
used for the final process-exit claim.

Not tested: every source provider or target, concurrent writers, power loss,
hostile cache forgery, or complete dependency closure. This is not a new proof
certificate format, a Cathedral reboot, a performance comparison, remote-main
qualification, or a sub-second compilation result.
