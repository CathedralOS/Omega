# build_artifact_only

`Build::artifact_only(builder)` selects the artifact-only application
mode: the build produces artifacts and performs no product compile, but
still drives the full required-output custody loop.

- `invalid_empty_artifact`: an artifact-only application that requires
  nothing rejects — "artifact-only Build requires at least one
  completed output". At least one required output must complete.
- `invalid_artifact_package`: `artifact_only` modifies an application
  declaration only — on `Build::package` project discovery rejects it with
  "artifact-only output requires an application role", before evaluation.
- `invalid_artifact_provider_selection`: the parent probe plus one
  `Build::select_provider`. Artifact-only mode builds no product for the
  provider to serve, so the Build is refused ("artifact-only Build cannot
  select providers") instead of dropping the selection.
- `invalid_artifact_native_binding`: the parent probe plus one native
  `Build::bind<Requirement::machine>`. It is refused the same way
  ("artifact-only Build cannot bind native implementations").
