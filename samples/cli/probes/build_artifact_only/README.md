# build_artifact_only

`Build::artifact_only(builder)` selects the artifact-only application
mode: the build produces artifacts and performs no product compile, but
still drives the full required-output custody loop.

- `invalid_empty_artifact`: an artifact-only application that requires
  nothing rejects — "artifact-only build `build` completed no required
  output". At least one required output must complete.
- `invalid_artifact_package`: `artifact_only` modifies an application
  declaration only — on `Build::package` it rejects "modifies an
  application declaration; this build declares package" during package
  resolution, before evaluation.
