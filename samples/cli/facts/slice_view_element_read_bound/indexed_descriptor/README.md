# Indexed descriptor production

A slice view (`&[u64]`) whose length is guarded (`view.len >= 3u64`) proves safe indexed element read at `view[2u64]` and preserves the length lower bound to discharge exact subtraction `view.len - 3u64` across all nine places.

This production of the [parent fact](../README.md) stores views in `Window` records and selects the record with a runtime index. `read_guarded` establishes the length bound on `windows[selected].values`; the receiving machine consumes that exact selected descriptor, reads element 2 and subtracts 3. No caller observation supplies the proof.

Each positive calls the full view and the short view. The full view selects `[10, 20, 30, 40]`, observes 30 and extent difference 1, and returns 0. The short view borrows only the first two cells of a six-cell array and must return 1 through the fallback. The entry traps unless both observations agree. The match-join place executes both join arms. These distinct values detect a wrong selector, stale descriptor, or replacement of view length by backing-array length.

| Place | Positive | Checking twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| other state | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Every twin changes only the establishing guard from `>= 3` to `>= 2`, retaining the consumer's `>= 3` requirement and the read/subtraction. It must reject during `omega --check` with `cannot prove requires contract for call read from read_guarded` and the missing `windows[selected].values.len >= 3` fact.

All positive entries bind the four hosted `ProgramEntry` targets and declare exit 0. The root and nine places passed native and Terminal execution on Linux x86-64 with optimized producer `ce4f5f765ad1` (SHA256 `d109cb7d7512ca25e654f3d8f56b1c610aa15a51e8b6b63f7e6c16bb7add6cce`); all nine checking twins matched their diagnostics. Exact command results and source hashes are in `build/checker-qualification/indexed-descriptor-row-observations.json`. Foreign host execution remains unmeasured for this production.

```sh
omega run --both samples/cli/facts/slice_view_element_read_bound/indexed_descriptor/<place>/main.omg
omega --check samples/cli/facts/slice_view_element_read_bound/indexed_descriptor/<place>/invalid/main.omg
```

The pass and fail fixture pairs are `tests/omega/{pass,fail}/facts/slice_view_indexed_descriptor_through_<place>_{exit,insufficient_guard}`. Their authored sources and Build declarations copy each place exactly; corpus recording follows measured acceptance.

## Requirements and evidence

The parent outcome is **CHECKER-CARRIES-OPERATION-DECISIONS** in [TASKS.md](../../../../../TASKS.md). This production establishes exact runtime-selector transport into descriptor extent terms; the original row establishes direct view transport.

1. **Establishment:** checking discharges the selected record index, the guarded extent, element index 2, and exact subtraction 3 at their respective operation entries.
2. **Identity and lifetime:** both shared views retain their own backing loans and lengths while the owning arrays stay live. A readable descriptor supplies metadata, never authority to invent referent contents.
3. **Composition:** every place carries the same `Window` collection and selected descriptor into the same consumer. Calls, joins, nested machines and target selection preserve its checked decisions.
4. **Soundness:** lowering consumes those decisions; the insufficient-guard twin rejects before product construction. The short-view branch has a different authored result and cannot silently perform the guarded operation.
5. **Checked to Terminal:** structural paths retain checked runtime index expressions through fields and reference crossings to `Extent`, rather than discarding selectors or requiring a source replay.
6. **Settlement and layout:** Build selects the ordinary named entry implementation; record, array, descriptor and call layouts reach product construction on the measured Linux x86-64 target.
7. **Instructions and activation:** the selected descriptor and element read execute through normal calls and branches; native and Terminal runs both observe the authored full/short results.
8. **Encoding and image:** normal native artifact validation and emission produce the executed Linux image. The same source's Terminal execution agrees on exit 0 in every positive place.
