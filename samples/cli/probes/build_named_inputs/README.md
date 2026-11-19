# build_named_inputs

Two pinned surfaces:

- Required outputs are not artifact-only: a plain `Build::application`
  completes the full `require` → `resolve` → `create` → `write` →
  `close` → `complete` custody loop for `manifest.txt`.
- `BuildInputs::get(&builder.inputs, name)` returns a `BuildSource`
  facet rooted at a caller-provided slot — an unassigned name rejects:
  `invalid_unassigned_input` pins "named build input `toolchain-key` is
  not assigned to this activation". No caller assigns input slots along
  the `--check` path used here.
