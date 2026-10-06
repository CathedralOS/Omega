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

Alternatively, add `expected_native_exit.txt` containing one decimal exit code
or `trap`. This opts the fixture into execution regardless of its name and makes
the native gate enforce that result even with `--record`. `trap` requires the
target's trap signal/exception, not an arbitrary abnormal termination. These
fixtures must bind the running host; missing execution fails the oracle.
The corpus also attempts them with `--interpret`/`--both` through the admitted
Terminal program/instance core. `interpreter:trapped` comes only from the core's
typed Trap outcome; a declined run, exhaustion or host signal cannot satisfy it.
When native and interpreted legs select the same target, interpretation borrows
the native report's exact Terminal artifact and producer admission profile before
publication. A same-target native refusal records unchecked without running Build
again. The check golden's canonical target and a different runtime target are
distinct program inputs and each owns one production.

## Reviewed interpreter migration debt

`tests/omega/interpreter_migration.json` designates only the original selected
reference-result canary. Its valid Terminal signature retains alternative
backings that the runtime cannot yet transport; the common program/start boundary
reports this capability before entry binding or effects. The source and authored
exit 70 remain unchanged. Replay it beside the singleton-reference exit-0 and
typed-Trap controls from the repository root:

```bash
python3 tools/corpus_gate.py --interpret \
  --golden tests/omega/corpus_outcomes.txt \
  --migration-roster tests/omega/interpreter_migration.json \
  --filter pass/references/selected_record_loan_disjoint_fields_exit,pass/borrows/static_record_reference_return_exit,pass/calls/runtime_guarded_reference_return_crash
```

The command compares with the existing checked golden because the repository
does not yet ship an interpreted golden. Its new runtime facts are visible moves;
the reviewed exact marker reports unresolved interpreter debt and keeps the gate
nonzero. It does not bless a checker refusal, timeout, wrong exit/stdout, partial
execution or native failure. When backing transport is implemented, the same
designation resumes the ordinary unchanged exit/stdout oracle automatically.
Other entry-arrival, provider-installation, host-import and external-completion
adapters remain `TERMINAL-RUNTIME-DRIVERS` dependencies; undesignated missing
capabilities still fail their ordinary oracles. This corpus route does not
establish ordinary CLI package admission or migrate the CLI's comparison driver.

Do not add per-feature Rust test files to this directory. The crate sets
`autotests = false`, so a new file here is not a test target, and each extra
target would link the whole compiler again. `support/` holds the package-input
helpers the runner uses.

## Running a fixture through the CLI

The corpus constructs its package inputs explicitly. Its native result does
not establish the ordinary install/update workflow. An unlocked CLI invocation
supplies no accepted semantic bindings; a fixture depending on package-owned
std can therefore stop at entry-schema admission before reaching the compiler
behavior under test. Do not bypass that boundary or infer acceptance from the
dependency's name or location.

For example, prepare the reference-pressure witness on macOS ARM64 from the
repository root:

```text
omega update --project tests/omega/pass/borrows/runtime_reference_array_pressure_exit --target macos_arm64
```

If review requests decisions, inspect the reported file and accept or reject
the actual findings. Only after resolving them, resume without repeating
`--target`, then run the original source:

```text
omega update --resume --project tests/omega/pass/borrows/runtime_reference_array_pressure_exit
omega run tests/omega/pass/borrows/runtime_reference_array_pressure_exit/main.omg
```

The authored success is process exit 70, not 0; compilation failure is CLI
exit 200. `run --both` also compares checked interpretation; supply finite
stdin as described in the [CLI guide](../../../application/cli/README.md).
The local lock retains this project's exact source/compiler acceptance, not a
portable fixture oracle. Source or compiler changes require fresh review, not
copying bindings or deleting a stale lock to turn it into an unlocked request.
