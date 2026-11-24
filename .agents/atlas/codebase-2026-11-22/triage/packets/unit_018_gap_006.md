# Packet unit_018_gap_006: x86-64 selected-form encoder, lines 19001-21917

This packet gives a disposition for each claim in [record unit_018_gap_006](../../records/unit_018_gap_006.md), which covers `isa-x86_64/src/selected_form_encoding.rs` lines 19001-21917 (area 11, native stage 11). Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. The file is unchanged between that baseline and `a0152d26`; current line numbers are one higher than the record's in the production tail (the file has 21918 lines, the record says 21917).

Status: in progress (early receipt).

File abbreviation: `X64sfe` = `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`.

## Claims

### C1. `trapping_form` returns `None` for other kinds (record §21655-21671, "Fail-open"; digest 11 line 160)
- Code path: `trapping_forms::trapping_form` (`X64sfe:21665-21670`) is called once, by `decoding::footprint` (`X64sfe:2989`).
- Contract: `footprint` must give every kind its declared effects. After the early return for trapping forms, the same function matches `kind` exhaustively with no `_` arm, and lists `Kind::TrappingInteger { .. } => unreachable!("trapping forms handled above")` (`X64sfe:3084-3086`).
- Trace: a `None` means the kind is not `TrappingInteger`; control continues to the IEEE, atomic and exhaustive per-kind arms. No kind reaches a default.
- Outcome: **intended**. A kind filter, not a fail-open. No fix.
