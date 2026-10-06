# Artifact-only application completes its required output

An authored witness that `Build::artifact_only(builder)` is satisfiable: the
build reserves one `RequiredOutput`, writes and seals its file under the
activation's granted staged-output root, and settles the obligation against
that sealed file.

This is the first `.omg` anywhere in the repository to declare
`Build::artifact_only`, and the first to use the `BuildOutput` facet at all.

## How the corpus hosts it

The corpus runner cannot compile a build machine that reaches the build
filesystem out of the working tree. It compiles a fixture with a project
declaration in package mode, and a package-aware activation requires a
compiler-validated canonical Source metadata index before any build effect
runs. The capture that produces that index admits only the canonical modes
package acquisition leaves on a snapshot (`0o555` directories, `0o444` files),
which is why the real CLI runs this same source out of
`.../local-snapshots/source-<digest>/source/` rather than the checkout.

The harness supplies that custody instead of the compiler relaxing the demand:
every fixture root is staged as a sealed private snapshot keeping its
repository-relative path (`support/sealed_fixture_staging.rs`), and the root
package is bound to the snapshot while dependency rows stay resolved against
the authored directory. Mirroring the relative path is what keeps every
recorded diagnostic position unchanged.

The real CLI on this source reports `wrote_output=true` and publishes
`files/manifest.txt` holding the authored bytes `omega artifact`. The corpus
checks the publication itself: `expected_published.txt` names
`files/manifest.txt`, and the runner builds and publishes the fixture as the
CLI does and records `published:files/manifest.txt`.
`artifact_only_unsealed_output_refused` is this case's sealing control, and
`artifact_only_rollback_overlay_refused` carries the producing-stop admission;
both declare an empty `expected_published.txt`.
