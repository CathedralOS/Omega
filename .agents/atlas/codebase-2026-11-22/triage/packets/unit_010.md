# Atlas packet unit_010: stage 04 target lowering and its audit

Status: in progress. The first receipt is verified; the rest of the slice is under review.

This packet gives one disposition per correctness claim in
[record unit_010](../../records/unit_010.md) and the matching lines of the
[area 10 digest](../../digests/10-native-stages-04-10.md). Evidence is read
from `main` at `24ad2700685e6cc6062026c87ed5fd24ed415265`. No supplemental
range in `second_pass_ranges.tsv` belongs to unit 10, so nothing was skipped.

Paths below shorten
`source-rust/subsystems/compiler/psi-to-native/pipeline/04_abstract-operations-to-target-operations/src/`
to `S04/`. The audit is the independent validator in `S04/preparation/`; it
runs on an explicit validator request and in tests, never in ordinary
compilation (`S04/preparation/legalization.rs:793-803`).

## Receipts

| # | Claim (original location) | Current source | Contract, test or trace | Outcome |
| --- | --- | --- | --- | --- |
| 1 | `SourceDominators::dominates` returns `true` for a block missing from the order (record §1 fail-open 1; digest §5 line 146, §10 item 3) | `S04/preparation/legalization_target.rs:818-830`; the same convention in `InputScope::admits` (`:451-469`, `self.interval.is_none()`) | A block missing from the order is unreachable from the entry, and every block vacuously dominates an unreachable one. The audit's unit comes from stage 03, whose session validates it (`03_abstract-operations-to-abstract-operations/src/abstract_optimization/pass_manager.rs:1355`) through `validate_total_cfg`, which refuses any unreachable block (`representations/abstract-operations/optimization-unit-semantics/src/unit_validation.rs:6062-6085`, called at `:9917`). Lowering also refuses one (`S04/target_lowering/control_flow.rs:84-86`, `:3061-3062`). The audit pairs target blocks with these source blocks by position and id, so no block it judges can be missing. | Intended; unreachable input is refused upstream |
