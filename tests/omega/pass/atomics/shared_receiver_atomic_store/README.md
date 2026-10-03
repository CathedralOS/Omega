# Shared atomic storage

`Cell::publish` stores 10 through a shared reference. `Cell::mark` performs
fetch-or with 5 through that same storage, returning the instruction-observed
prior. The caller requires prior 10 and current resident 15, then exits 70.
No program rewrite or exclusive-argument substitution is part of this test.

On macOS ARM64, with the built `omega` executable on PATH:

```sh
omega update --project tests/omega/pass/atomics/shared_receiver_atomic_store --target macos_arm64
omega run --both tests/omega/pass/atomics/shared_receiver_atomic_store/main.omg
python3 tools/corpus_gate.py --both --filter atomics/
```

Use the matching declared target on Linux or Windows. The negative
`fail/atomics/shared_receiver_plain_field_write` keeps ordinary shared stores
forbidden. Serial execution checks event identity and caller-visible storage;
it is not a concurrent memory-model proof.

A fresh checkout requires the package review reported by `omega update` to be
settled, followed by `omega update --project <directory> --resume`. That local
acceptance is not an audit claim; the corpus command exercises the compiler
without changing project acceptance.
