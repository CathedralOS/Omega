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

A fail fixture may supply `unaccepted_build_output.txt`, listing relative
staged filenames which must remain absent. That selects a caller-owned output
account without private-staging custody or accepted restricted-request meaning.
The compiler must report pending consent before the authored Build writes;
the runner checks the listed effects before discarding the test directory.
Ordinary fixtures retain privately issued snapshot/staging execution. This
is test-supplied authority, not a source spelling or a new product grant mode.

`corpus_runner.rs` compiles every fixture and prints one outcome record each.
Negative fixtures check once under their authored Build sponsors and input
custody. Without a declared program entry, source acceptance records `checked`,
not a rejection caused by demanding an absent entry. With a declared entry,
native production continues the same checked compilation to expose later-stage
refusals; it does not acquire sources or execute Build again. An accepted
negative control remains red until its source semantics or authored expectation
is resolved; no native fallback hides it.
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
`expected_stdout.txt` and `expected_stderr.txt` also opt a fixture into execution
and independently constrain each engine's captured stream. Matching exits cannot
excuse missing/different output, and `--record` cannot bless that result. Captures
drain both native pipes concurrently; byte comparison normalizes only CRLF to LF.
The corpus also attempts them with `--interpret`/`--both` through the admitted
Terminal program/instance core. `interpreter:trapped` comes only from the core's
typed Trap outcome; a declined run, exhaustion or host signal cannot satisfy it.
When native and interpreted legs select the same target, interpretation borrows
the native report's exact Terminal artifact and producer admission profile before
publication. A same-target native refusal records unchecked without running Build
again. The check golden's canonical target and a different runtime target are
distinct program inputs and each owns one production.

For target installations whose entry carries hosted context or service bindings,
`terminal_target_provisions.txt` may contain `compiled-application`. This trusted
fixture configuration requests one ordinary compilation cohort for the host and
the authored target roster. Each target supplies only its own actual admitted
runtime provisions, and every report must retain the same canonical Terminal
artifact. No foreign image is executed by this option; the observations are
Terminal executions. It grants no domain exit, clock or input authority. Without
this option target installations remain artifact-only and refuse missing roots.

An optional `expected_terminal_targets.txt` requests exact target installations
against one retained Terminal artifact. Each nonempty line contains one unique
target tag followed by a decimal i32 exit or `trap`, for example:

```text
macos_arm64 0
windows_x86_64 trap
```

The check leg produces one Terminal semantic artifact; the native leg borrows
its already-retained artifact before publication. Each row calls the public
Terminal runtime without recompilation, providers, a receiver, input, a clock,
or domain-exit delegation. A fixture requiring those provisions refuses rather
than receiving fabricated authority. The runner records `terminal-target.<tag>`,
checks unchanged semantic bytes, identity and canonical module storage after each
installation, and records `terminal-targets:match` only when every row agrees.
Malformed oracles and checked/built rows with missing or mismatched observations
cannot be blessed by `--record`.

`pass/targets/portable_family_entry_installations_exit` pins portable family-root
installation: Windows traps while the other authored targets return Unit. This
is not acceptance of the complete Build companion or a grant of backing authority
from family catalog membership.

`--checked-interpreter` explicitly selects the checked-tree source oracle instead
of Terminal. It uses the same reviewed fixture inputs, selected entry and
explicit installed arguments, but owns its independent checked compilation rather than
reusing a native report's Terminal artifact. No refusal or source spelling chooses
this route. Remaining automatic entry-context construction must migrate under
`EXPLICIT-ENTRY-STORAGE`; a root parameter alone grants no storage. It records
`interpreter-engine:checked-tree`; its separate defaults
are `corpus_checked_interpreted.txt` and, with `--both`,
`corpus_checked_both_<target>.txt`. Until those goldens exist, use an explicit
golden to expose fact moves without recording:

```bash
python3 tools/corpus_gate.py --checked-interpreter \
  --golden tests/omega/corpus_outcomes.txt \
  --filter pass/layouts/runtime_callee_integer_at_fitting_write_exit,pass/calls/runtime_guarded_reference_return_crash
```

The checked engine has no typed Trap outcome. Every execution error remains
declined, including explicit crashes; diagnostic text cannot satisfy a trap
oracle. Both engines enforce the same authored exit and stdout promises before
recording. Trap probes remain declined until the checked API retains their kind.

## Reviewed interpreter migration debt

`tests/omega/interpreter_migration.json` designates the original selected
reference-result and external-completion canaries. The selected reference's valid
Terminal signature retains alternative
backings that the runtime cannot yet transport; the common program/start boundary
reports this capability before entry binding or effects. The source and authored
exit 70 remain unchanged. Replay both beside the singleton-reference exit-0 and
typed-Trap controls from the repository root:

```bash
python3 tools/corpus_gate.py --interpret \
  --golden tests/omega/corpus_outcomes.txt \
  --migration-roster tests/omega/interpreter_migration.json \
  --filter pass/references/selected_record_loan_disjoint_fields_exit,pass/collections/runtime_fixed_array_selector_call_store_exit,pass/borrows/static_record_reference_return_exit,pass/calls/runtime_guarded_reference_return_crash
```

The command compares with the existing checked golden because the repository
does not yet ship an interpreted golden. Its new runtime facts are visible moves;
the reviewed exact marker reports unresolved interpreter debt and keeps the gate
nonzero. It does not bless a checker refusal, timeout, wrong exit/stdout, partial
execution or native failure. When execution completes, the designation resumes
the ordinary unchanged exit/stdout oracle automatically.
The second designation preserves the authored exit 70 of
`runtime_fixed_array_selector_call_store_exit`. Its genuine Terminal artifact
declares `External(ProcessExit)`; the shared core refuses the missing closed
invocation before effects. Normal-return Unit/scalar adapters cannot satisfy an
external completion, and completion kind participates in binding signature
equality. This is truthful missing capability, not interpreter exit-70 support.
The [Process-exit contract](../../../../../TASKS.md#process-exit-contract) owns
the remaining representation, lowering and runtime work.

Other entry-arrival, provider-installation and host-import
adapters remain `TERMINAL-RUNTIME-DRIVERS` dependencies; undesignated missing
capabilities still fail their ordinary oracles. This corpus route does not
establish ordinary CLI package admission or migrate the CLI's comparison driver.

Do not add per-feature Rust test files to this directory. The crate sets
`autotests = false`, so a new file here is not a test target, and each extra
target would link the whole compiler again. `support/` holds the package-input
helpers the runner uses.

An optional `expected_terminal_inspections.txt` lists qualified machine names,
one per line, optionally followed by `debug` to require a debug companion.
After checking succeeds, the corpus selects each through the public inspection
producer and reloads its canonical artifact. Debug companions
must retain the same program identity; changing that identity must reject at
external ingress. This tests inspection without requiring a native program entry.

An optional `expected_native_component_descriptions.txt` contains the nonzero
number of independently emitted native component products expected by a fixture.
The native leg consumes each canonical description through `verify_component`,
compares its Terminal artifact, realization identity and exact bridge-inclusive
stack demand with the emitted product, and exercises publication receipt
rejection of missing/altered metadata and sibling-export substitution. It uses
source-produced components; no Rust-built module or native body stands in for
the fixture.

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

### Captured Build input fixtures

An authored fixture may supply named host-API inputs as immediate directories
under `build-inputs/<slot>/`. The corpus harness captures each sealed directory
and calls `BuildSnapshotRequest::with_inputs` before both preliminary acceptance
and production compilation. Build reads those slots through
`BuildInputs::get(&builder.inputs, "<slot>")` and ordinary `BuildSource`
operations. Slot names do not select host paths at evaluation time.

The shipped CLI currently exposes Source capture selection, not named slot
assignments. A manual CLI control for a named-input fixture must therefore
observe a missing-slot refusal; it is not a successful named-input run. The
corpus supplies the public compilation host API, without fixture handling in
the shipped product.

Pinned qualification can retain each native publication and its exact portable
Terminal envelope with `OMEGA_CORPUS_CAPTURE_ARTIFACTS=<fresh-directory>`.
Files are created once under `<tier>/<group>/<fixture>/<target>/` and made read-only;
existing artifacts cause a refusal rather than replacement. A subsequent run
uses `OMEGA_CORPUS_VERIFY_ARTIFACTS=<retained-directory>` to require byte equality
before interpretation and before/after native execution, without rewriting the
retained files. Native execution also retains the pathname's filesystem identity
and timestamps across execution (including Unix change time on Unix hosts). The modes are mutually exclusive. The ordinary compile, image
audit, runtime authority, numerical and stream oracles remain unchanged. The
qualification validator hashes and binds those pre-existing artifacts and the
compiler build receipts before the verification run.

Inspection oracle rows may append `runtime linux_x86_64` and an explicit trusted
fixture provision. These host calls require explicit trusted harness opt-in
`OMEGA_CORPUS_HOST_PROVISIONS=1`. Default source/native inspection still produces
and decodes every selected artifact, recording host-neutral `unrequested` facts.
Opted-in runtime observations record `match` on that actual host and
`unavailable-linux_x86_64` elsewhere; unavailable is not execution acceptance.
On Linux x86_64, `libc-inet <comma-separated-u32s>` binds exact typed
imports to actual libc inet_makeaddr/inet_lnaof calls, checks observed host values
and consumption, and rejects an incompatible adapter signature before effects.
`opaque-entry` supplies an abstract owned entry value and requires the exact value
back. `materialized-entry` supplies a record field and requires whole completion;
`relocated-materialized-entry` additionally requires its relocated guest backing.
These are corpus-host provisions, not source-selected runtime authority or CLI
adapters. Missing host support records an unavailable observation.

Run the owned-arrival and libc carrier observations on Linux x86_64:

```sh
OMEGA_CORPUS_HOST_PROVISIONS=1 python3 tools/corpus_gate.py \
  --golden tests/omega/corpus_host_provisions_linux_x86_64.txt \
  --filter pass/facts/opaque_closed_carrier_exchange_exit,pass/facts/opaque_nested_carrier_exchange_exit,pass/facts/opaque_wrapped_carrier_exchange_exit,pass/core/materialized_owned_entry_return
```

The host-specific golden requires seven machine observations. An unsupported
host's unavailable record cannot match it. This separate runtime golden keeps
ordinary source acceptance portable without claiming unavailable executions.

`runtime portable cleanup-resume` selects a trusted SDK lifecycle provision on
the existing projected nominal cleanup fixture's `exercise_unit_projection`.
With `OMEGA_CORPUS_HOST_PROVISIONS=1`, it consumes the roundtripped inspection
artifact and explicitly admits one exact normal Unit/i32 Console byte import
under the corpus host's policy, then binds it to a recorded semantic sink.
Uninstalled construction must refuse; catalog availability alone supplies no
provider. It checks the authored `OPBA` line under unbounded execution and
one retained instance funded one refused site at a time: before edge cleanup,
inside the parent hook after `P`, and inside the child hook after `B`. Unfunded
retries preserve output, effects and fuel; live close stays Busy, and completion
matches the entire reference effect stream and per-site fuel usage. This is
portable interpreter acceptance, not native Console or native fuel evidence.
Without the opt-in it records `unrequested`; unknown portable provisions refuse.
The ordinary immutable capture/verification environment also binds runtime
inspection artifacts under `inspection/<program-fingerprint>/portable/terminal.psiart`
before the SDK provision executes; verification compares bytes without replacing
the retained artifact.

The macOS ARM64 record requires the actual SDK runtime match:

```sh
OMEGA_CORPUS_HOST_PROVISIONS=1 python3 tools/corpus_gate.py \
  --golden tests/omega/corpus_host_provisions_macos_arm64.txt \
  --filter pass/drops/runtime_projected_nominal_cleanup_exit
```

Other hosts can execute the portable provision but remain unqualified by this
host's record. Ordinary source/native goldens retain the opt-out observation.

`runtime portable external-extent-view` supplies one admitted external allocation
to the source-authored `extent_view_external_content::attempt` inspection. It
observes the two original words, a wrong-word trap, incompatible transfer rejection,
missing-backing refusal, and an identical-geometry/different-provenance profile
crosswire refusal followed by a successful retry with the original allocation.
The returned extent retains its original grant; no observation grants vacancy
or resident custody. This uses the public SDK, not compiler-internal mutation.
Provider entry binding reserves two logical cells, refuses with one cell of
headroom while returning the original buffer, then succeeds on retry. Teardown
releases the allocation and every transported descriptor/profile row.
The read provision and `external-extent-write` execute borrowed and loaded
artifacts across one-unit fuel pauses. Their eight-byte transfers fit exactly
eight payload bytes; seven bytes and one byte refuse with sticky live-storage
exhaustion, no published result and no additional charge on resume. Scratch
retires before completion or refusal. These observations cover compiler-owned
transfer payloads, not the host-supplied allocation or generic temporary memory.
Run with `OMEGA_CORPUS_HOST_PROVISIONS=1` and filter
`pass/layouts/extent_view_external_content`; without opt-in it records
`unrequested`. Cathedral's QEMU boot supplies the separate native observation.

`runtime portable resident-storage <cell-peak> <payload-peak> <u64-words...>`
executes the authored whole/aggregate resident `attempt` through initialization, helper transport,
extraction and original-buffer handback. Borrowed and loaded artifacts run with
the exact ceilings, one cell less, one payload byte less, one payload byte total,
and actual write-only supply despite the source provider's read/write description.
A competing root reservation after the complete initialization image retires
independently forces decoder refusal. Fuel pauses do not repeat effects;
budget/authority refusal retains custody until teardown. Success preserves the
unused tail, checks the extracted value in source and refuses repeated handback.
Use the same opt-in and filters `pass/layouts/resident_whole_extraction` and
`pass/layouts/resident_aggregate_extraction`. The payload oracle names owned
placement-image bytes, not codec metadata, host-owned backing capacity or RSS.

`runtime portable mutable-boundary` supplies an observation-only host adapter
to `build/receiver_view_writeback_exit`'s source-produced Build helper. It loads
the external artifact, checks the three stored-view lifetime promises, and
requires effects 1 and 2 exactly once across fuel pauses, original-receiver
identity, and final identifier readback `after`. A different receiver must
reject without a normal-return observation. Run with
`OMEGA_CORPUS_HOST_PROVISIONS=1`; without opt-in it records `unrequested`.
This is Terminal embedding evidence, not a claim that production Build execution
has left the checked-tree engine.

Fixtures may explicitly supply fresh installed entry RAM with `entry_ram.txt`:
one `source-position=hex-bytes` row per declared occurrence. These are requests,
not host addresses or assumed authority; normal native admission checks the exact
entry, placement, complete initialized type and loan custody. An optional
`expected_entry_ram.txt` uses the same format for independent original-backing
oracles after normal return. Native execution captures its dedicated runtime
lane under the existing child deadline; Terminal execution observes its own
original allocations. Missing, extra or differing observations cannot be recorded
as successful runtime oracles. An empty input file explicitly tests absent supply.

`runtime portable build-bindings <slots> <entry> <identifier-or->` supplies the
Build binding imports with source-issued designated operands. Slots are an
ordered comma-separated list. Both borrowed and loaded artifacts must retain
the original receiver, produce those inert selections exactly once through fuel
pauses, and preserve the requested identifier readback. Replayed tickets,
unissued operation identities and a foreign receiver refuse. The conditional
Build fixture checks that an unexecuted binding has no effect; the receiver
fixture also exercises explicit reborrowing. Run with
`OMEGA_CORPUS_HOST_PROVISIONS=1`; without opt-in it records `unrequested`.
The companion is not reconstructed from external artifact bytes or debug names.
Description-query execution and production Build scheduling still use the tree
engine and are not established by this provision.

`runtime portable storage-cells <peak> <unsigned-result|unit>` runs an authored helper
with its exact live-cell ceiling and with one cell less, replenishing fuel one
unit at a time. The sufficient run must return the authored scalar or Unit and report
the exact peak. The short run must refuse for live storage, retain its activation
without a result, and repeat the same refusal without changing usage. Teardown
must release all live cells and payload bytes. This measures the named logical
account, not process memory. `receiver_view_writeback_exit` covers a scalar record,
an owned call result, and a copy that survives a write to the original.
Enable with `OMEGA_CORPUS_HOST_PROVISIONS=1`; otherwise it is unrequested.

Authored inspections compile independently through the public Inspection product.
For positive fixtures they still run when native realization refuses; the row
retains its native diagnostics and rejected verdict alongside the inspection
facts. Such a result can establish Terminal-only behavior, never native
acceptance. Intended-invalid fixtures do not execute inspection provisions.
