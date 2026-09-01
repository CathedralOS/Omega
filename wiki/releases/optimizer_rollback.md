# Exact-Rule Optimizer Rollback

This is the operational rollback procedure for the exact rules published in
[Optimizer Exact-Rule Release Notes V1](optimizer_exact_rules_v1.md). Rollback
is native-production tooling; it does not change the `build.omg` selection or
create an optimization profile.

## Apply one exact rollback

Add the repeatable command argument for the affected exact rule:

```text
omega --disable-optimization ControlFlowCleanup \
      --target linux_x86_64 main.omg
```

Use the exact case-sensitive spelling from the release inventory. Confirm the
rule applies to the target and that the selected phase composition is supported
before treating a compile failure as a rule regression.

Unknown and duplicate names reject before compilation. A known name absent
from this build's authored selection is an accepted visible no-op, so the same
fleet rollback can cover builds with different explicit selections.

## Read and retain the receipt

A successful CLI publication prints one line in this shape:

```text
optimizer rollback: requested=[ControlFlowCleanup]; applied=[ControlFlowCleanup]; effective=[]
```

- `requested` is the exact command-line rollback set.
- `applied` is its intersection with the build-authored selection.
- `effective` is the authored selection after subtraction.

Capture standard output with the build log. The receipt is retained by the
published in-memory compile report and printed only after successful CLI
publication; it is not currently persisted as a receipt file. A nonempty
rollback request rejects `--check` and Terminal-artifact production because
neither route can publish this native realization receipt.

## Verify the rollback

1. Confirm `requested` contains every intended exact name and no other name.
2. If the build selected the rule, confirm it appears in `applied` and not in
   `effective`. If it did not, confirm the visible no-op has `applied=[]`.
3. When `effective=[]`, confirm compilation rejoined the ordinary no-selection
   path. The repository's hosted-target golden firewall checks semantic,
   proof, object-text, image-byte, and metadata parity for this case.
4. Run the affected target/OS and workload verification before deployment.
5. Retain the command, build log, compiler identity, target, and output digest
   for the rollback incident.

## Restore the rule

Remove only that exact `--disable-optimization` argument after the repaired
compiler passes the rule's semantic/corruption, differential, determinism,
target-matrix, measurement, and rollback evidence again. Do not replace the
exact selection with an `O1`/`O2`/`O3`, debug, or release bundle.
