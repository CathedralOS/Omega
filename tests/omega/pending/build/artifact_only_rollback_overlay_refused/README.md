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

## Why this is pending, not in `fail/`

It does reject in the corpus today, but for the wrong reason — the harness
refusal described in `artifact_only_required_output_completion/README.md`:

    package-aware build filesystem access requires compiler-validated
    canonical Source metadata

A `fail/` case that rejects before reaching the rule it names pins nothing, so
this stays a pending witness until the harness can bind a fixture root with
canonical Source metadata. Move it into `fail/` then, with `expected.txt`
holding the artifact-only rollback fragment above and its golden row recorded.
