# Packet: unit 018, gap 004 (area 11, x86-64 selected-form encoding lines 7001-13000)

Status: in progress. This early receipt records the first settled claims; the full slice follows.

Baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`; current checkout `a0152d26` (the two commits between them do not touch `psi-to-native`).
`F` means `source-rust/subsystems/compiler/psi-to-native/pipeline/11_bound-framed-machine-to-selected-form-encoding/isa-x86_64/src/selected_form_encoding.rs`.
Record: [`records/unit_018_gap_004.md`](../../records/unit_018_gap_004.md). Line numbers below are current; the record's numbers still match within one line.

| # | Record section / line | Code path | Outcome | Evidence |
| --- | --- | --- | --- | --- |
| 1 | `12750-13000 memory::frame`, item 9 (record L:167): `validate` returned `ImmediateOutsideU12` for a displacement outside i32 | `F` `memory::frame::validate` L:12887 | Fixed | [#5392](https://github.com/CathedralOS/Omega/pull/5392) (`59841944`) now maps the failure to `MemoryDisplacementOutsideI32`; the 07-13 ledger row "x86 memory immediate and width diagnostics" covers it. |
