# Where compiler tests go

The compiler is tested end to end. A test is a fixture, not a Rust file here:

- `tests/omega/pass/<feature>/main.omg` compiles.
- `tests/omega/fail/<feature>/main.omg` rejects, and its `expected.txt` holds a
  fragment of the expected diagnostic.
- `tests/omega/run/<feature>/` compiles; its input and output expectation files
  describe the run.

Name the fixture for the compiler behavior it pins, not the sample that exposed
it. A fixture that imports `omega_language_std` needs a `build.omg` like its
neighbours: it depends on `source/library/std` and binds the program entry roots.

`corpus_runner.rs` compiles every fixture and prints one outcome record each.
`tools/corpus_gate.py` diffs those records against
`tests/omega/corpus_outcomes.txt`:

```bash
python3 tools/corpus_gate.py --filter providers/          # the loop
python3 tools/corpus_gate.py --record --filter providers/ # pin intended movement
python3 tools/corpus_gate.py --native --filter providers/ # build, run, diff natively
```

A pass fixture named `*_exit` is executed by the native leg, and its exit code
is recorded in the host's golden; name a fixture that way when its observable
result is the exit code.

Do not add per-feature Rust test files to this directory. The crate sets
`autotests = false`, so a new file here is not a test target, and each extra
target would link the whole compiler again. `support/` holds the package-input
helpers the runner and `tests/native-differential` share.
