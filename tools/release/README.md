# Release-record substrate

`release_record.py` is the substrate for the release record required by
[the completion contract](../../wiki/drafts/rust_compiler_completion.md#closure-rule):
the record must contain the commit, pinned Rust toolchain, host OS and
architecture, commands, results, elapsed time, and the exact list of expected
skips. The tool owns the eight-gate manifest verbatim, runs a gate's command
block on one host, and writes one `omega-release-record/1` JSON document per
runner row under `records/`.

The substrate records evidence; it does not certify the matrix. A failed
command is recorded as a `fail` gate, an unrun gate or runner row stays
`open`, and the closure reads `closed` only when all eight gates pass and all
four platform runs are recorded on the same commit. `check` re-validates a
stored record so a hand-edited record cannot read as closed.

Requirements: Python 3.9+ and Git. Standard library only; identical under
PowerShell on Windows and sh on macOS/Linux. Gate commands run through `mbx`
when installed and through Cargo otherwise, per AGENTS.md.

## Recording a runner row

Each runner row is a *lane*: the contract requires the emitted programs to
be executed directly on the matching host, so `run` must be invoked on a
host that can execute the lane's target, and must carry the direct-execution
observation for that lane:

```text
python3 tools/release/release_record.py run --target linux_x86_64 \
    --native-execution "mbx nextest run -p omega-native-differential-test"
```

`--native-execution` is the lane's native-observation evidence: one command
that runs the emitted programs natively and exits 0 on success. Its command,
exit, elapsed time, and bounded output tail are stored on the lane's
`platform_runs` row; the row reads `recorded` only when it exits 0. A lane
that fails its observation still writes the record — the row stays `open`.
On a host that cannot execute the lane at all, `run` refuses rather than
writing a record that cannot carry the lane.

PowerShell uses the same invocation with `python`. Restrict to one gate with
`--gate RC-REPOSITORY` (repeatable). A red gate still writes a record — the
record is the evidence, open rows and all — so `run` exits 0 after writing
and reports `closure: open` with the open rows.

The `linux_arm64` lane additionally accepts any host when the record names
the emulator (the contract's "emulation is acceptable only when the release
record names the emulator and version" allowance); the observation command
runs through that emulator:

```text
python3 tools/release/release_record.py run --target linux_arm64 \
    --emulator "qemu-aarch64 9.0.0" --gate RC-NATIVE-MATRIX \
    --native-execution "qemu-aarch64 <path-to-emitted-elf>"
```

## Expected skips

A platform test may skip only when irrelevant to its runner. Declare each
expected skip while recording:

```text
--expect-skip 'RC-NATIVE-MATRIX|native_differential::uefi_row|no UEFI host here'
```

Skipped tests observed in nextest/cargo output that are not declared this way
land in `unlisted_skips` and keep the closure `open` — the contract requires
the *exact* list.

## Validating a record

```text
python3 tools/release/release_record.py check records/linux_x86_64__*.json
```

`check` exits nonzero when the schema drifts, a required field is missing, a
gate's recorded command no longer matches the contract, a `recorded`
platform row lacks a passing direct-execution observation or its lane's
matching host, an emulator appears on a non-arm64 row, or the recorded
closure contradicts the rows.

`tools/tests/test_release_record.py` guards the substrate itself: the gate
manifest must stay verbatim-equal to the completion document's command
blocks, and committed records under `records/` must re-validate.
