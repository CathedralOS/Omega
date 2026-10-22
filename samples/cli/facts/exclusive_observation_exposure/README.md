# Exclusive exposure of a saved array element

A saved observation that items[0] is 7 stays provable across an exclusive loan of a different element.

The consuming operation is `agree(observed, current)` with `requires observed == current && observed == 7`, after a saved read of `items[0]` on `[7, 1]` and an exclusive loan of the other element that does not store. Exit 0 means the saved observation still names that element. The reject twin loans `items[0]` instead and does not write it, so the observation is no longer valid.

Parent outcome: `STATE-LOCAL-VALUE-FRONTIER` (`TASKS.md`). An exclusive loan permits a later store of the lent referent, so a complete empty store list does not preserve a saved observation of that referent. A loan the body already overlaps keeps the narrower footprint: lending or writing one array element does not become a write of the whole array.

## Measured on `d6a282e29da` before the repair

Without the compiler edit, `omega run --both samples/cli/facts/exclusive_observation_exposure/local/main.omg` exited 0 in both engines. `omega --check samples/cli/facts/exclusive_observation_exposure/local/invalid/main.omg` also exited 0. There was no diagnostic: the saved fact stayed provable after an exclusive loan of `items[0]` that only reads.

## Measured after the repair

All nine positive places exit 0 in Terminal Psi and native execution (`omega run --both`). Each reject twin fails `omega --check` with `cannot prove requires contract`. The other-machine twin also fails its `ensures result == items[0]` for that same overlapping loan.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Requirements and evidence

- **Establishment.** The observation is the read `items[0]` of the literal `[7, 1]`. Where that read is in another machine or target body, the callee requires `items[0] == 7` and the caller proves it from the literal. The consuming `agree` requires the saved value still equal the live element and 7. The exclusive loan does not establish the fact; the read does.
- **Identity and invalidation.** An exclusive loan of `items[1]` does not overlap the observed element, so the saved copy stays equal to it. An exclusive loan of `items[0]` overlaps it even when the loan body does not store, and the equality is unprovable.
- **Composition.** The nine places carry that same sentence. Parameter, call result, other machine, and target body pass the saved value or the array through a contract that names the element. Field, match join, const name, and another state keep the read in the lending state. The positive and reject twins differ only in which element is lent.
- **Soundness.** The loan is ordinary exclusive access. No mathematical binding is executed. Proof retirement follows the lent referent.
- **Checked-to-Terminal Psi.** The positive places execute in the Terminal Psi interpreter and agree with native execution. The reject twins fail checking, so they have no Terminal Psi product.
- **Later realization.** Each positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64` and exits 0 natively on this Windows x86_64 host. The other three targets are bound and not executed here.

Acceptance demonstrated: exclusive exposure of a saved array-element observation, in all nine places, with overlapping-loan twins. What remains open in `STATE-LOCAL-VALUE-FRONTIER` is case, generic, reference, and float carriers, plus the rest of that outcome's composition obligations.
