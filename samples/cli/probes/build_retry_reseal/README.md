# build_retry_reseal

The dep's build completes its required output in two rounds:

1. `require` + `resolve` + `create` + `write`, then `complete` while the
   writer is still live — the evaluator answers
   `OutputCompletion::Retry { obligation, file }` because the staged file
   is not yet sealed.
2. The retry path seals the writer, `include_source`s the file, and calls
   `complete` again — which must now yield `Sealed`.

The pinned failure direction (Retry then `fail`) already exists as an
invalid control under `build_package_custody`; this sample covers the
success direction of the same custody loop.
