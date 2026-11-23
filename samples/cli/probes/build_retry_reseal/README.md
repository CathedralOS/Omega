# build_retry_reseal

The dep's build completes its required output in two rounds:

1. `require` + `resolve` + `create` + `write`, then `complete` while the
   writer is still live — the evaluator answers
   `OutputCompletion::Retry { obligation, file }` because the staged file
   is not yet sealed.
2. The retry path closes the writer, passes the captured bytes to
   `Build::include_source` as `generated.omg`, and calls `complete` on the file.
   The second completion must yield `Sealed`.

The pinned failure direction (Retry then `fail`) already exists as an
invalid control under `build_package_custody`; this sample covers the
success direction of the same custody loop.

The generated `value()` forwards to the authored `seed()`. The
`invalid_authored_selects_generated` control inverts that direction: the
dep's authored `value()` calls a generated `generated_value()`, which the
generated-source boundary forbids (`wiki/spec/build/execution.md`), and the
rejection must be the same whether the dep is reviewed on its own or
checked inside its consumer.


The `invalid_foreign_include` control tries to publish outside the dependency’s
logical namespace. The `invalid_path_as_source_bytes` control passes a filesystem
handle where publication requires a byte slice.
