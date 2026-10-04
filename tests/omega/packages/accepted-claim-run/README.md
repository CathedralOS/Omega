# Accepted dependency claim

`Main::main` cites the foreign `establish` axiom to satisfy its own
`established()` postcondition. The root explicitly grants that claim. After
ordinary package review accepts the claim in `omega.lock`, the program should
run with exit 0 without another approval file. Missing acceptance must request
review; a dependency cannot approve itself.

From the repository root, on a supported host:

```sh
omega update --project tests/omega/packages/accepted-claim-run/app --target macos_arm64 --offline
omega run tests/omega/packages/accepted-claim-run/app/main.omg
```

Select the actual host target for review. Resolve the generated review's exact
decisions and resume with `omega update --resume --project
tests/omega/packages/accepted-claim-run/app` when review reaches that point.

At `9952e21837a` on macOS ARM64, compilation rejects the individual root grant
before review can settle it. Removing the grant for diagnosis reaches native
lowering, which refuses the call to `establish` because it has no boundary plan;
that is not a successful ungranted execution. The source checker accepting the
postcondition alone does not establish package admission or runtime support.

The fixture now uses `Build::accept_boundary<establish>(builder)`. Static
resolution succeeds; package review still rejects individual accepted-machine
admission as described above. The remaining admissions and proof-citation work
must preserve the citation rather than deleting the rejection.
