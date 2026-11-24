# Unit 016 triage packet (areas 10 and 11)

Status: in progress.

This packet gives each claim in [record unit_016](../../records/unit_016.md) a disposition, together with
the matching bullets in the [area 10 digest](../../digests/10-native-stages-04-10.md) and the `isa-aarch64`
bullets in the [area 11 digest](../../digests/11-native-stages-11-15.md). Code was checked at the common
baseline `24ad2700685e6cc6062026c87ed5fd24ed415265`. None of the files in this unit changed between that
baseline and `a0152d26`, the branch base. No open pull request touches stage 09, stage 10 or `isa-aarch64`.

Paths are abbreviated as in the record: `EP/` is stage 09 `entry-planning/src/`, `S9/` is stage 09 `src/`,
`S10/` is stage 10 `src/`, `A64/` is stage 11 `isa-aarch64/src/`.

| # | Original claim | Code path and contract | Evidence | Outcome |
| --- | --- | --- | --- | --- |
| 1 | Record `EP/imports.rs` item 9; area 10 digest line 193. An import locator is dropped without refusal when the call's instruction is not in its function. | `EP/imports.rs:72-76` keeps a locator only for a call whose instruction exists. In the same `plan_function_body`, `S9/physical_facts.rs:28` runs `foreign_call_plans::capture`, which returns `"selected call instruction is missing or repeated"` for that call (`S9/foreign_call_plans.rs:1459-1460`). Stage 14 builds every body object through `body_object.rs:126-133` and `body_object/imports.rs:73-81`, which return `Error::ForeignCalls` on that stored error. A locator of `None` is a non-import binding, which has no import to plan. | Code trace. | Intended: refusal preserved at stage 14. |
