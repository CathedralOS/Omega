# unit_006 triage packet: areas 09 and 13

This ledger gives a disposition for each claim in [record unit_006](../../records/unit_006.md) that falls in area 09 (native stages 00 to 03: record Parts D and E) or area 13 (the native driver: record Part C). Parts A and B belong to area 01 and are covered by [the CLI and foundation ledger](../01-cli-foundation.md). Review baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`; code was read at `a0152d26` (two later commits, neither touching these files). No `second_pass_ranges.tsv` row covers these paths, so no gap packet owns a supplemental range here.

Status: in progress.

| Record section and line | Code path | Contract | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| Part C, `DRV/image/plan_callables.rs` item 6 (record line 557); summary item 5 (line 993) | `driver/image/plan_callables.rs` `plan_callback_target`, `.map(\|(target, _)\| target)` | A private callback body is linked into the program image that the root admits; only a separately published component product carries its own runtime provisions and callable authority. | `plan_callable` still runs `TerminalCallableRuntimeProvisions::admit`, so its checks apply to callbacks. Callback planning runs only on the image route: `driver/mod.rs` `produce` returns before `plan_callables` when the stop is `RuntimeAdmission`. The two dropped values are consumed only by `NativeComponentProduct::from_emitted` (`driver/image/components.rs`), which publishes a component the receiver verifies independently. Callback admission shares the root's `program_provider_context`. | Intended |
