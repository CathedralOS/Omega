# Write-only IEEE store readback

Fact: an IEEE f64 computed and stored through a write-only borrow (&write) preserves its exact bit pattern on readback, satisfying the 0.75 comparison across all nine places.

Owning Task: `WRITE-ONLY-BORROW` (`TASKS.md`) — "computed IEEE store/readback" acceptance item.

## Places

| Place | Positive Program | Reject Twin (invalid) | Transport Mechanism |
| --- | --- | --- | --- |
| root | [`main.omg`](main.omg) | — | canonical witness: computed `6.0 / 8.0` through `&write f64`, read back and compared to `0.75` |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | read-back value bound to a local before the comparison |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | read-back value carried through a record field |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | read-back value passed as a machine parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | read-back value returned from a machine call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | read-back value unified across reconverging branches |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | a named constant supplies the dividend of the computed store |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | read-back value crosses a nested-machine transition |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | a separate machine forwards the read-back value |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | per-target implementations supply the compared expectation |

## Pinned Fixtures

- `tests/omega/pass/facts/write_only_ieee_store_readback_through_<place>_exit`
- `tests/omega/fail/facts/write_only_ieee_store_readback_through_<place>_rejected`

Each invalid twin adds one line — the store reads its `&write` referent — and rejects with `reads write-only parameter`; write-only authority is the enabling requirement of the fact's transport.
