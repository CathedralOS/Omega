# Packet: unit 015 gap 002 (area 10, stage 07 spill and split)

This packet accounts for the open claims in [`records/unit_015_gap_002.md`](../../records/unit_015_gap_002.md), which covers lines 541–5493 of `runtime_spill_and_split.rs` in stage 07 (register homes). The baseline is `24ad2700685e6cc6062026c87ed5fd24ed415265`; the file is unchanged between that revision and the current `origin/main`. No merged triage ledger and no open pull request covered this range when the review began.

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/`. `S07` is `pipeline/07_selected-instructions-to-register-homes/src/allocation/`.

## Claims

### 1. Blocks without a slot summary contribute nothing to slot reuse

- **Original:** record §4 "Fail-open", third bullet, and "What I am unsure about", first bullet; digest 10 lines 187 and 214.
- **Code path:** `S07/runtime_spill_and_split.rs:2229-2232` and `:2292-2295` iterate `block_index in 0..block_count`, with `block_count = function.blocks.len()` (`:2179`). `representations/selected-instructions/src/blocks.rs:892-894` returns `Some(self.0.get(block_index)?.slot_summary())`; `slot_summary` (`blocks.rs:499`) always builds a summary through `OnceLock::get_or_init`.
- **Contract:** every block has a slot summary; `None` means only an out-of-range index.
- **Trace:** the loop never passes an out-of-range index, so the `continue` arm cannot run.
- **Outcome:** intended (unreachable guard). No fix needed.

Review in progress.
