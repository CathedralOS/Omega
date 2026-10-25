# Saved observation of an unwritten array element

A saved observation of an unwritten array element stays equal to 7 across a write to a disjoint element.

The consuming operation is `agree(observed, current)` with `requires observed == current && observed == 7`, after a saved read of `items[0]` on `[7, 1]` and a write to the other element. Exit 0 means the saved observation still names that element. The reject twin writes `items[0]` instead, so the observation no longer names the element.

Parent outcome: `STATE-LOCAL-VALUE-FRONTIER` (`TASKS.md`). The copy is recorded from the live element scalar in `omega-rust/psi/compilation/pipeline/04_typed-trees-to-checked-trees/src/flow/transfers/scalar_values.rs`. A later write retires only overlapping storage.

## Measured on `cfc1b266038` before the repair

`omega run --both samples/cli/facts/saved_observation_snapshot/local/main.omg` exited 200. The diagnostic was `cannot prove requires contract for call agree from main: saved == items[0] && saved == 7`. The same program with no write checks. Each side separately satisfied `== 7`; the disjoint write dropped the equality between the saved observation and the live element. `local/invalid/main.omg` failed with that same diagnostic.

## Measured after the repair

All nine positive places exit 0 in Terminal Psi and native execution (`omega run --both`). Each reject twin fails `omega --check` because the write overlaps `items[0]`, with `cannot prove requires contract` for `agree`. The other-machine twin also fails its `ensures result == items[0]` for that same overlapping write.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Requirements and evidence

- **Establishment.** The observation is the read `items[0]` of the literal `[7, 1]`. Where that read is in another machine or target body, the callee requires `items[0] == 7` and the caller proves it from the literal. The consuming `agree` requires the saved value still equal the live element and 7.
- **Identity and invalidation.** A write to `items[1]` does not overlap the observed element, so the saved copy stays equal to it. A write to `items[0]` overlaps it and the equality is unprovable.
- **Composition.** The nine places carry that same sentence. Parameter, call result, other machine, and target body pass the saved value or the array through a contract that names the element. Field, match join, const name, and a nested machine keep the read in the writing state.
- **Soundness.** This row is the array-element snapshot. Exclusive exposure and case, generic, reference, and float carriers remain open. `pass/calls/runtime_temporary_lending_order_exit` already exits 0 in both engines.

Acceptance demonstrated: a saved array-element observation across a disjoint element write, in all nine places, with overlapping-write twins. What remains open in `STATE-LOCAL-VALUE-FRONTIER` is exclusive exposure of saved observations and the other carriers named above, plus the rest of that outcome's composition obligations.
