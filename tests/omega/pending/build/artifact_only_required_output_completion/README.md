# Artifact-only application completes its required output

An authored witness that `Build::artifact_only(builder)` is satisfiable: the
build reserves one `RequiredOutput`, writes and seals its file under the
activation's granted staged-output root, and settles the obligation against
that sealed file.

This is the first `.omg` anywhere in the repository to declare
`Build::artifact_only`, and the first to use the `BuildOutput` facet at all.

## Why this is pending, not in `pass/`

The corpus runner cannot host any build machine that reaches the build
filesystem, whatever the fixture authors. It compiles a fixture with a project
declaration in package mode, and a package-aware activation requires a
compiler-validated canonical Source metadata index before any build effect
runs. The runner's fixture harness binds the root package straight to the
working-tree directory and captures no index, so the fixture records:

    package-aware build filesystem access requires compiler-validated
    canonical Source metadata

## The variants that bound it

Binding the root through `PackageSourceBinding::with_canonical_source_metadata`
in `driver/tests/support/fixture_package_inputs.rs` moves the refusal one layer
down rather than clearing it, because the capture admits only a sealed root:

    physical Source directory tests/omega/pending/build/
    artifact_only_required_output_completion has noncanonical mode 0o755;
    expected 0o555

That is the mode package acquisition leaves on a snapshot, which is why the
real CLI runs this same source: it compiles out of
`.../local-snapshots/source-<digest>/source/`, a sealed copy, not the working
tree. The harness bypasses acquisition, so it has no sealed root to offer.
Making it always stage and seal a snapshot would relocate every fixture's
source root into a temporary directory and churn the repo-relative paths the
golden's diagnostic lines carry, so it is a corpus-wide harness change rather
than a fixture fix.

Verified through the real CLI on exactly this source: it compiles, reports
`wrote_output=true`, and publishes `files/manifest.txt` holding the authored
bytes `omega artifact`.

Move this directory into `pass/` with its golden row once the harness can bind
a fixture root with canonical Source metadata.
`artifact_only_unsealed_output_refused` is this case's sealing control, and
`artifact_only_rollback_overlay_refused` carries the producing-stop admission.
