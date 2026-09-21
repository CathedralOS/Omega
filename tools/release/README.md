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

```text
python3 tools/release/release_record.py run --target linux_x86_64 --all
```

PowerShell uses the same invocation with `python`. Restrict to one gate with
`--gate RC-REPOSITORY` (repeatable). A red gate still writes a record — the
record is the evidence, open rows and all — so `run` exits 0 after writing
and reports `closure: open` with the open rows.

Platform runs that need a named emulator (only `linux_arm64` per the
contract's runner table):

```text
python3 tools/release/release_record.py run --target linux_arm64 \
    --emulator "qemu-aarch64 9.0.0" --gate RC-NATIVE-MATRIX
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
gate's recorded command no longer matches the contract, an emulator appears
on a non-arm64 row, or the recorded closure contradicts the rows.

`tools/tests/test_release_record.py` guards the substrate itself: the gate
manifest must stay verbatim-equal to the completion document's command
blocks, and committed records under `records/` must re-validate.
