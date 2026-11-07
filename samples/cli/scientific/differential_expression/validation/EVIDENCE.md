# BIO02 qualification

The application is the narrower conditional-dispersion NB model documented in
README.md, not full DESeq2. Independent expected numerical values were frozen
before Omega execution. The independent reference programs, outputs, versions
and SHA256 manifests are committed beside this record.

## Producer and environment

Qualification producer: main 66ad153cb238181a1d68cbe72eaf2857ee0e915a plus the
34-line float-landing occurrence-custody repair in this change. Linux x86_64;
nightly-2026-09-04 (rustc 1.100 nightly); saved cloud setup, four build jobs,
4-CPU quota, 16 GiB cgroup memory. One Rust build ran at a time. Immutable executables:

- Omega SHA256 57d1c8e712a0b8dce730a4989fc38ebad043d4af7498267fa12d0e8444db1fe7
- Terminal observer SHA256 7adde704df720595b5ae9068e3ae255533cfab9336d41fb685fac68d548d13fb
- Native application SHA256 7808078fd4ae7fe23c852f1aa87d11dd4a7f7f2ca85ac44b9a242f478d4c41bb
- Frozen SciPy oracle SHA256 ae989957c834fc58cfa1978ba1ceec3d9390874517b6928edd411518e40eb372

`observed.json` retains actual stdout, stderr and exit for every fixture in both
execution modes, and hashes stdout. Numerical comparisons include all six size
factors and all nine quantities of each tested gene, plus every exclusion reason.
No expectation was generated from Omega output. The private observer executes
the exact retained Terminal artifact from ordinary accepted package preparation.

## Observed numerical results

All 19 fixtures passed in both native and Terminal execution. The 38 actual
observations are byte-identical between modes. There are 35 tested-gene rows
per mode. Maximum absolute native/Terminal errors against the frozen oracle:

`[6.454e-6, 6.962e-10, 8.648e-7, 1.904e-6, 5.065e-9, 9.342e-10, 7.916e-8, 1.358e-7, 3.400e-7]`

Size-factor maximum absolute error is 9.581e-9. Large means use the documented
relative tolerance; all columns pass. The checker requires the entire fixture
set and every row, not exit parity alone.

## Independent statistical comparison

DESeq2 1.46.0 / R 4.5.0: seven numerical fixtures independently executed with
matching filters, supplied median-ratio factors and moment dispersions, no
shrinkage, no Cook filtering and no independent filtering. The version-pinned R
script and captured session make the narrower comparison explicit. Maximum
absolute differences against the SciPy oracle, in output-column order:

`[1.364e-12, 2.220e-16, 2.010e-4, 1.349e-5, 3.466e-6, 1.576e-6, 4.905e-9, 2.534e-6, 6.099e-7]`

All are within the documented absolute/relative comparison tolerances. The
largest absolute difference is a fitted mean; it is small relative to that mean.
DESeq2's internal numerical regularization prevents literal equality.

## Observed compiler failure and repair

An authored shifted-Stirling expression failed on current main during package
preparation, with a misleading LinuxArm64 placement-policy premise diagnostic.
The retained reduced witness is `pass/layouts/guarded_policy_state_premise`.
Before repair, its unrelated nested `0.0 - 1.0 / 360.0` fold detached the child
operator's exact authored occurrence from the live expression tree. A later
private execution projection then treated it as an orphan obligation.

The successful exact anonymous fold now retains descendant occurrence identities
on its literal result before removing expression edges. This does not select
an overload, discard an obligation, change authority checks, or weaken a guard.
Existing checked finalization resolves the retained occurrences normally.
The reduced witness was actually rejected before repair and actually checked
after repair. One loaded 120-second corpus run timed out; the complete rerun
with a 300-second fixture cap checked it in 118645 ms. That observation is
retained as a success, not inferred from the timeout.

`run/float/folded_anonymous_operator_custody_exit` checks field, local, call
argument, returned value and anonymous comparison compositions. Actual native
exit 0, Terminal exit 0; the authored expected exit is 0. Existing computed
comparison and nested-float fixtures checked. Anonymous division by zero and
unproved placement-policy callee premises remain rejected for their exact
expected reasons. The sample's intended-invalid control remains rejected with
``cannot prove index `2` is within length 2`` (compiler exit 1).

## Permission and source review

The exact root terminal policy admits three Console functions: process input,
process output and process termination. No build-host request or filesystem
permission was added. The unchanged std source-risk classifications were reviewed
through normal package admission; they are not grants of host authority.
Executable-identity refreshes used ordinary package update/resume decisions.
No identity override, fixture trust, custody bypass or dangerous accept-all
option was used for this sample.

## Rust checks

Affected `omega` and `validation` crate checks passed. The standalone observer's
Clippy check with `--no-deps -- -D warnings` passed. The compiler repair passed
scoped Clippy, which reported 139 existing warnings outside the changed file.
Strict dependency-wide warnings-as-errors stopped at the unchanged
`optimization_selections.rs:152` useless conversion. No suppression was added.
Single-file Rust formatting and the observer formatting checks passed.

## Main integration

Initial complete qualification used d041496df109 plus the custody repair.
Candidate e8893ed84d38 on main 1a353f2e117 passed all numerical observations,
independent review and normal exact-SHA verification; its verified recovery
bundle was retained before further integration.

Main then advanced to 6ede3322eb881daaa996fb898d4a04732d309d56. Its mutable-call
normal-return, storage-observation and guarded-custody changes are exercised by
this application's parser and normalization calls. Source inspection could not
qualify that newer producer, so both executables were rebuilt sequentially on
the integrated tree and all 19 native plus 19 Terminal cases were observed again.
Every numerical quantity and exclusion passes; both modes' streams and exits
are byte-identical to each other and to the earlier qualification. Even the
rebuilt native application bytes are identical. The frozen SciPy and DESeq2
oracles remain unchanged.

The 6ede corpus checks observed three successful checks and two exact
expected rejections. The guarded-policy witness checked in 117639 ms. Two
existing float fixtures improved from stale timeout pins to checked; only those
scoped rows were refreshed from actual observations. The current-main composition
fixture again executed native exit 0 and Terminal exit 0. The intended-invalid
source still rejects its exact index-2 diagnostic. Integrated crate checks and
the standalone observer's own warnings-as-errors Clippy check passed.

Fresh independent final review reran all 38 numerical receipt checks and all
seven DESeq2 comparisons. It verified both immutable executable hashes and the
retained Terminal artifact hash, the complete 6ede compiler controls,
and the two selectively refreshed timeout pins. No material blocker was found.

## Latest integration and publication

The latest measurement base is 66ad153cb238181a1d68cbe72eaf2857ee0e915a.
Main's intervening immutable checking, float-predicate custody, reference storage
and descriptor-lowering changes affect exercised paths. Both producers were
rebuilt sequentially using the saved cloud setup. Their actual complete numerical
observations, checked against the unchanged independent oracles, are retained in
`observed.json`. The initial and 6ede recovery bundles remain available.

The resumed ordinary package review changed only `baseline_schema` 12 to 13
in 287 exact canonical rows: three root Console permissions, 141 callable and
141 external-supply rows, and two existing dangerous-capability classifications.
Old and new row content was compared after that single schema replacement.
No source association, purpose, execution context, signature, host request or
filesystem grant changed. The independent reviewer confirmed the same exact
proposal before ordinary update/resume acceptance.

Latest compiler controls: three positive checks, two exact expected rejections;
the guarded-policy witness checked in 115291 ms. The composition control
observed native exit 0 and Terminal exit 0. Root omega/validation checks, observer
formatting and its own warnings-as-errors Clippy check passed. The observer
publishes the same retained product after Terminal observation through the
existing product-owned publication API, then that executable is observed for
all native fixtures; no compilation or permission inference occurs during this
publication step.

Subsequent main cf645a79ddb2e99b0b371c0659e4ed1006bc2fbf adds a spectral
sample, corrects an unused std sine coefficient and improves landing metadata
cache reads. Independent integration review confirmed no compiler/runtime
change and no changed function in BIO02's selected private-math/Console graph.
The measurement producer remains honestly pinned above; this incoming diff
does not justify relabeling those binaries or repeating numerical measurement.

All 19 fresh native and 19 fresh Terminal numerical observations passed,
including the 35 tested-gene rows per mode, six factors for each valid case and
all exclusions. Both modes' stdout, stderr and exits are byte-identical, and
remain identical to the previous measured producer's results. The newly
produced executable and Terminal artifact bytes differ from the earlier
producer, so their actual new hashes are recorded rather than inferred.
Terminal artifact SHA256: 5a21f601a13ed45c997e4eb75b6969a716d3bcbeb75b3236c5c55dd5cdf18621

All 19 frozen SciPy cases independently regenerated exactly with SciPy 1.17.0;
all seven pinned DESeq2 comparisons passed again.

Final integration incorporates main 6fd3b2838624bea0c966d87ccd103c227904a748.
Its two additional commits change only refinement fixtures, corpus records,
task status and design documentation. No further compiler, runtime, library or
BIO02 source changed. Final independent review checked every raw observation
and both execution modes' complete receipts, the four producer/product hashes,
all nine numerical error maxima and size factors, and all seven DESeq2 cases;
no material blocker was found. Normal exact-candidate verification covers both
the root CLI/validation closure and the standalone observer workspace.
