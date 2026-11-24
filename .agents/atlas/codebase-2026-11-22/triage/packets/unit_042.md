# Unit 042 triage receipts: `proofs/bounded`

Status: in progress.

This packet reviews the [unit 042 record](../../records/unit_042.md) for area 07 (Psi semantics). The record covers the
float, classicality and observation files in `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/` and lines
1–3100 of `bounded_denotation.rs`. Lines 3101–17334 belong to gap packets 019–021 and are not reviewed here.

Evidence baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265` (`main`). The survey read commit `4f3c255d0559`. Paths below are
relative to `source-rust/subsystems/compiler/psi/semantics/proofs/bounded/` (`B/`).

## Receipts

| # | Original claim | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | Record `float_integer.rs` §6 and `float_meaning.rs` §6; [digest 07 line 73](../../digests/07-psi-semantics.md?plain=1#L73) and §10 item 6 (line 285): `meaning()` is defined twice. | `B/float_integer.rs:9` imports `crate::float_meaning::meaning`; the one definition is `B/float_meaning.rs:443`. | Merged in #5357 (`psi: share literal float meanings and native integer widths`). `rg "fn meaning\(" B/` finds one definition. | Fixed |
| 2 | Record `bounded_observations.rs` §7 and [digest 07 line 204](../../digests/07-psi-semantics.md?plain=1#L204): the entry counters are not behind `cfg(test)`. | `B/bounded.rs:8-9` declares `#[cfg(test)] mod bounded_observations;`. Both callers are `#[cfg(test)]` (`B/proof.rs:198`, `B/bounded_denotation.rs:14454-14455`). | The survey commit `4f3c255d` has the same attributes (`bounded.rs:8`, `proof.rs:198`, `bounded_denotation.rs:14438`). The record read only the file, not its module declaration. Release builds contain no counters. | Intended (record inaccuracy) |
