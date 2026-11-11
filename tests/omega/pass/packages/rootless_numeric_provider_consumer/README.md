The consumer checks both a finite value and NaN through the library’s public classifier and requires exit 0. The library has no entry. Its private numeric member remains visible to package review even though this program never calls it.

Each target's declared realization table now supplies `F32::negate` (decision `target-realized-float-arithmetic`), and Build programs may not select or override it, so the consumer's four former root choices for `UnusedNumericProvider` are removed. The unused library member remains reviewed and is never an execution candidate. The paragraphs below record the earlier selection-based form.

The migrated consumer agrees in both engines on all four hosts: actual `69ec9f80599b` on Linux and `3b8603164e96` on Linux ARM, Mac and Windows. Fresh package review on each host retains `UnusedNumericProvider::native_negate` with its exact `F32::negate` root choice and no grants. The root-choice migration leaves the library and its wrong-`square_root` control byte-for-byte equal to `69ec9f80599b`. The unused member is reviewed, not invoked by this consumer. Earlier missing-dependency, default-conflict and import refusals remain separate evidence.

At `6b1c338126fa`, the same migrated consumer again completes ordinary package
update, review and execution on Linux, with native/Terminal exit 0. The generated
review and accepted lock retain the unused private negate member and its exact
numeric requirement and binding. The [run](https://github.com/CathedralOS/Omega/actions/runs/42026360839)
performs no additional named-machine inspection or `audit packages` command,
and it still does not invoke that unused member.
