The consumer checks both a finite value and NaN through the library’s public classifier and requires exit 0. The library has no entry. Its private numeric member remains visible to package review even though this program never calls it.

Core and library choices share the existing default tier, so their two different negate providers conflict. The consumer now makes the required explicit root choice for its library’s `UnusedNumericProvider` on each target. Both the import and provider path name the library’s package root. This is a migration from the original conflicting input; it adds no default tier and removes no captured library choice.

The migrated consumer agrees in both engines on all four hosts: actual `69ec9f80599b` on Linux and `3b8603164e96` on Linux ARM, Mac and Windows. Fresh package review on each host retains `UnusedNumericProvider::native_negate` with its exact `F32::negate` root choice and no grants. The root-choice migration leaves the library and its wrong-`square_root` control byte-for-byte equal to `69ec9f80599b`. The unused member is reviewed, not invoked by this consumer. Earlier missing-dependency, default-conflict and import refusals remain separate evidence.

At `6b1c338126fa`, the same migrated consumer again completes ordinary package
update, review and execution on Linux, with native/Terminal exit 0. The generated
review and accepted lock retain the unused private negate member and its exact
numeric requirement and binding. The [run](https://github.com/CathedralOS/Omega/actions/runs/42026360839)
performs no additional named-machine inspection or `audit packages` command,
and it still does not invoke that unused member.
