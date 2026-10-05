# Artifact-only production refuses a nonempty release-rollback overlay

The authored build is the artifact-only application of
`artifact_only_required_output_completion`; the only difference is this case's
`rollback.txt`, which declares the overlay `--disable-optimization CopyPropagation`.

`wiki/spec/build/optimizations.md#release-rollback-and-promotion` says a product
cannot claim rollback for stages it does not execute. An artifact-only product
publishes completed build outputs and executes no product optimization stage, so
a nonempty overlay names nothing it could subtract. Product admission rejects it:

    artifact-only builds execute no product optimization stages to roll back

The refusal's placement is the point. The build has already run and its required
output has reached sealed staged custody when admission refuses, and it refuses
before the report that would publish those outputs is constructed, so no partial
product set is published.

Verified through the real CLI on exactly this source: the refused invocation
leaves only the build-directory owner stamp and writes no `completed/` result
set, while the same source without `--disable-optimization` publishes
`files/manifest.txt`.

## How the corpus hosts it

This case reached the harness's own refusal before the rule it names, because
the runner bound the root package straight to the writable fixture directory
and so had no canonical Source metadata to offer a package-aware build
activation. The runner now stages every fixture root as a sealed private
snapshot and binds the root package to it, as
`pass/build/artifact_only_required_output_completion/README.md` describes, so
the rejection recorded here is the authored one.
