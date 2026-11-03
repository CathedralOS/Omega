# Check a call before using a constant result

A constant helper could use its unfinished result to justify a call made while
calculating that result. The compiler now rejects that circular proof while
preserving calls whose preconditions can be established independently.

For example, a helper calculating a bound of seven may first call a function
that requires its input to fit a separately calculated bound. That works. If
the requirement instead depends on the helper's own unfinished bound, checking
rejects it. An earlier call's completed result can still justify the next
arithmetic operation.

Each deferred check records whether it is a prerequisite or a check of a
completed result. The evaluator and interpreter enforce that distinction, and
serialized programs carry it explicitly. Older artifacts without it reject.

The evaluator also keeps the ordinary checker's storage and termination facts
for the selected calls. Losing those facts had rejected a valid recursive
library calculation. Every member of a recursive cycle still needs a valid
termination proof; unresolved call conditions cannot supply that proof.

## Results

R13 was built from the integrated source below. All runtime results here were
observed on macOS ARM64. The other three hosted targets have authored bindings;
execution on those hosts is not claimed.

| Check | Observed result |
| --- | --- |
| Frozen baseline, nine circular twins | Wrongly accepted by optimized R6 |
| Original, generic-function and pending-call examples | All 30 positives check and run with native and Terminal exit 0 |
| Their invalid controls | All 28 normal updates reach the intended proof diagnostic |
| Subsequent invalid source checks | 19 reach the proof diagnostic; nine stop at the compiler-identity review fence and receive no proof credit |
| Existing layout customer | Normal update succeeds; native and Terminal exit 70 |
| Focused Rust tests | 48 passed, including rejection of incompatible formats 236 and 237 |
| Source corpus selection | 13 checked, 10 intended rejections, no wrong-reason refusal |
| Retained calling policy | Portable execution and inspection match after releasing the source session |
| CLI and corpus-runner clippy | Both exit 0 |

The delegated termination controls include five valid source checks and three
actual native/Terminal agreements at zero. All six invalid controls reject.
Four match their expected text; two have stale expected text, byte-identical
to the retained R11/R12 observations. Those two are not counted as matched
controls, and their fixtures are unchanged. The separately run layout customer
completes the 12-fixture termination selection.

The golden update adds 19 fixture rows and changes four existing rows. No keys
or diagnostic lines are removed, and unrelated row blocks remain byte-for-byte.
Unchanged outcomes retain their earlier timings; fresh timings remain in the
raw observations. The original circular control changes from wrongful
acceptance to its intended rejection. The other existing cases still pass.

## Source and replay records

[The fresh R13 evidence snapshot](https://github.com/CathedralOS/Omega/blob/07563764c321256405f33fdb652fa7bcbd81a7fa/README.md)
contains the unchanged source bundle, raw receipts and restoration instructions.
It is a separate parentless evidence commit, outside the feature branch and
main. Raw data is not added to this code tree.

R13 was built from exact source
`4b50ded92ae9cb9f5216e8e8ed6115f9054d7cc2` with an empty compiler-input patch,
on fixed integration base `e3c4ae4fb406f0a40e8988e2b79100ceeeaff5b3`.
Its optimized CLI SHA-256 is
`0420b257867901596eec8c2fb1bca87d547932d304fd107db2adb2b49c7b4bc7`;
the runner SHA-256 is
`aa0dc08ecdfda46930b4fe976617548d4006b489610c3e520c6b93bcc28b8c9a`.
Later commits wrap one Rust function call and update documentation and recorded
outcomes. Formatting leaves the call and its sole argument unchanged. These
remain observations of the R13 producer; no later compiler build is claimed.

[Earlier baseline and R12 records](https://github.com/CathedralOS/Omega/blob/f7790912afea928f7a4fbbbd412ca51991159e9c/README.md)
remain under their original identities. Fresh R13 qualification was required
because actual conflict resolution changed checking, interpreter and encoding
inputs. Earlier observations were not relabeled as integrated results.

This closes circular call admission for the provider-bound fact. Crash-capable
bound helper admission and the broader scalar-range migration remain open.
The unused-constant control establishes checking of its calculation, not direct
runtime observation of eight. GUI qualification remains deferred.
