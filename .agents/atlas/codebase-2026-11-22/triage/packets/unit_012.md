# Triage packet: unit_012 (area 10, stage 05 instruction selection)

Status: in progress (early receipt).

This packet gives a disposition for each correctness claim in
[records/unit_012.md](../../records/unit_012.md) and the matching area-10 digest
lines. The unit covers five stage 05 files under
`source-rust/subsystems/compiler/psi-to-native/pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/`
(abbreviated `S05/` below). No second-pass gap range covers these files.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. Code was read at
`a0152d26` (two commits later; neither touches stage 05). Open PRs at start:
#5409, #5405, #5368, #5338, #5336, #5307; none touches these files.

## Claims

| # | Original location | Code path | Contract | Evidence | Outcome |
| --- | --- | --- | --- | --- | --- |
| 1 | Record §1 "Fail-open risks", `:970`; digest line 166 | `S05/selection_access_lifecycles.rs` `resource_profile::compatibility::run::assess`, `u64::MAX.checked_div(footprint)` | [Typed runs](../../../../../wiki/spec/resources/typed_runs.md): zero-footprint cells have an explicit count and occupy no bytes. The Psi reference `validate_run_relocation_resources` (`psi/access/validated-placements/resources.rs:709`) checks only cell 0 when the footprint is zero. | With footprint 0 the emitted check still requires `count * 0 == length`, so the length must be 0, and then checks cell 0 only, as the reference does. No byte is read or written for a zero-size cell, so an unbounded count cannot reach memory. | Intended |
| 2 | Record §1 "Fail-open risks", `:1003-1004` | Same function, `if matches!(role, SupplyRole::Source) { return Ok(()) }` before `check_cell` | Typed runs: relocation moves a `Live` run into compatible vacant backing. The reference checks the source with `require_stable_region_supply(read)` over the occupied range and runs per-cell `validate_initialization_resources` only on the destination. | The emitted code does the same: the source gets the Stable read coverage pass, and per-cell placement and alignment checks run only for `Destination`. The source cells' placement was established when the run became `Live`. | Intended |
| 3 | Record §1 "Fail-open risks", `:1803`; digest line 166 | `S05/selection_access_lifecycles.rs` `assess_cell`, `if operation != View` around the whole-extent `extend_coverage` | [Access policies](../../../../../wiki/spec/resources/access.md#establishment-and-retirement): View interprets established content; Initialize needs exclusive Stable storage. The reference splits the same way: `validate_access_resources` (used for views, e.g. `psi/semantics/src/access/access_ram_correspondence.rs:210`) checks each field descriptor and region, and only `validate_initialization_resources` adds the whole-image Stable supply. | For View, the emitted code still checks each field descriptor against one covering region, its reach and alignment, and each field region under its own observation model. Requiring Stable whole-extent coverage would wrongly refuse External or Atomic views. | Intended |
