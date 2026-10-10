# NF14 candidate qualification

The release compiler built successfully in 6m 35s from compiler base
26c92c17b8f4b21f39e14a5be57c85cfdec601de. The numerical library helper passed
`--check` (25 source files, 11.759s). Independent exact-rational oracles passed.
See compiler-build.txt, host.json, independent-oracles.txt and routes.json.
The initial full-project candidate tested was
606e5cf1fe84629533d3c38dcdbea3be01f58ec8; each route records source hashes.

| Route | Diagonal readonly | Correlated copied |
| --- | --- | --- |
| Full project check | environmental refusal, exit 1 | environmental refusal, exit 1 |
| Native run | environmental refusal, exit 200 | environmental refusal, exit 200 |
| Terminal comparison | environmental refusal, exit 200 | environmental refusal, exit 200 |
| Separate source-only check | exit 0, 11.959s | exit 0, 12.151s |

Every full-project route, the existing float baseline check, and the complete
invalid control stopped before source validation with:

```
cannot open private source resolver storage: git cache entry `/` is invalid: cache custody ancestry is owned by an unrelated user
```

`/` is owned by nobody/nogroup on this host; /workspace and the selected cache
are agent-owned. No system ownership, permissions or custody checks were changed.
The baseline reproduced the same environmental signature. This does not
attribute any compiler/source failure, approve a review or count as intended
readonly rejection. A legitimate environment configuration is a parent-owned
shared dependency; this candidate does not repair or bypass it.

The separate source-only probes are retained in source-only/ with complete
sources, exact command vectors, hashes and raw stdout/stderr. Both valid probes
passed. The invalid probe rejected (exit 1, 1.997s) with:

```
error: machine `overwrite_readonly` state `entry` assignment cannot write `source` because it is not mutable in this state (if `source` is undeclared -- a typo -- no such field or local exists)
```

These probes remove only std Console imports, the Console binding field,
Console reach declaration and process exits. They have no build metadata and
cannot qualify full-project checking, native execution or Terminal comparison.
Full authored projects are retained unchanged. The intended readonly diagnostic
fragment was updated to the observed source-only wording; full-project
rejection remains unverified.

The source check of the scalar pivot helper is not an execution result. Its
result cases for NaN/Inf/signed zero/negative pivots need runtime coverage after
package/host custody is legitimately usable.

No descriptor-based factor or solve implementation is claimed: NF01 descriptor
and package, NF02 policy, and NF11/NF12 copy/triangular interfaces are unavailable
on the base. The adjacent library API checkpoint names exact required
signatures. Private sample two-by-two classifications are independent data
oracles, not verified shared solver errors. Shape, policy, general stride,
multiple RHS, nonfinite intermediates and alias/error-mutation frontiers remain
integration acceptance requirements. Parent review and normal landing remain.
