# Packet: unit_028_gap_015 (area 12, native representations)

Scope: [record](../../records/unit_028_gap_015.md) and [unit](../../units/gap_015.tsv), one range:
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit/src/identity.rs`
lines 4401-5662 (below `identity.rs:N`, line numbers at baseline `24ad2700`). Area 12 only.
Starting evidence: [07-13 ledger](../07-13-psi-and-native.md) (no row for this range) and the open
PR list on 2026-11-24 (no open PR touches `identity.rs`).

Status: in progress.

## Claims

### C1. `ElementViewRead` writes either a place id or `0` plus an argument (record §1 "How it decides"; digest 12 none)
- Code: `identity.rs:4484-4489`.
- Contract: the two forms must not decode the same. The record inferred, without checking, that a place id is never 0.
- Trace: `semantic_vocabulary` ids are declared by `semantic_id!` as `NonZeroU64`
  (`source-rust/subsystems/compiler/foundation/semantic-records/src/identity.rs:135-145`), and
  `CanonicalBytes::id` writes `value.get()` as a u64 (`identity.rs:3010-3012`). A place id therefore
  never writes eight zero bytes, so the leading `u64(0)` distinguishes the path form.
- Outcome: **intended**. No fix.
