# Nested Record Value Transport

A nested record `Envelope` preserves its inner `Header` bound `header.limit <= 50` and sibling `payload` across value transport so `header.limit + payload <= 100` in all nine places.

Every place performs the same operation: add the inner header limit (`<= 50`) and sibling payload (`<= 50`) of an `Envelope` and return the sum under `ensures result <= 100`. The oracle verifies that `envelope.header.tag == 7` remains untouched and the sum is `42` (`limit = 20, payload = 22`), exiting 0 only when both hold. Only the value transport of the nested record changes across places.

| Place | Positive Project | Reject Twin | Transport Mechanism |
| --- | --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) | envelope is copied into a local variable before reading fields |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) | an enclosing packet holds the envelope in a field before reading |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) | machine reads fields directly from its explicit parameter |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) | envelope is returned from a contracted forwarding call |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) | both match arms yield an envelope, preserving bounds across the join |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) | inner bound is governed by a named constant |
| another state | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) | successor state receives the envelope and sums its fields |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) | forwarding machine passes the envelope to another machine |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) | each of four target-scoped machine bodies preserves the bound |

## Execution

Each positive place is an application that exits 0:

```sh
omega run --both samples/cli/facts/nested_record_value/local/main.omg
omega run --both samples/cli/facts/nested_record_value/field/main.omg
omega run --both samples/cli/facts/nested_record_value/parameter/main.omg
omega run --both samples/cli/facts/nested_record_value/call_result/main.omg
omega run --both samples/cli/facts/nested_record_value/match_join/main.omg
omega run --both samples/cli/facts/nested_record_value/const_name/main.omg
omega run --both samples/cli/facts/nested_record_value/other_state/main.omg
omega run --both samples/cli/facts/nested_record_value/other_machine/main.omg
omega run --both samples/cli/facts/nested_record_value/target_body/main.omg
```

Each place has an `invalid/` twin that removes or widens the inner header bound (`header.limit <= 50`), so the sum is no longer provably in range. The controls reject with `may overflow` (or `cannot prove ensures contract` for `const_name`). Check each twin with:

```sh
omega --check samples/cli/facts/nested_record_value/<place>/invalid/main.omg
```

## Requirements and Evidence

- **Parent outcome:** `CONSERVATION-CONTRACT` (`TASKS.md`).
- **Ticket:** `5116b13df42742d9839df19a9be7fc4a`.
- **Requirement established:** Compound value preservation across value transport. Specifically, value transport (across locals, fields, parameters, call returns, branch joins, named constants, state transitions, machine calls, and target-scoped bodies) preserves nested record invariants and sibling fields without dropping path-bound facts or perturbing untouched payload fields.
- **Obligations accounted for:**
  - *Establishment:* Construction and precondition contracts introduce nested field bounds (`envelope.header.limit <= 50`) alongside sibling scalars (`envelope.payload <= 50`).
  - *Identity and lifetime:* The nested record is a value type (`[copy]`); value transport preserves exact field values and bounds without storage aliasing or invalidation.
  - *Composition:* Callee calls, successor transitions, and match joins compose exact actuals and preserve preconditions on all incoming paths.
  - *Soundness boundaries:* Reject twins demonstrate that breaking the inner bound prevents discharging `result <= 100` and reports overflow/contract failures at each place.
- **What remains open:** Reference and mutable loan transport of nested record fields (`&envelope.header.limit`), field assignment invalidation of sibling path facts, and dynamic partition conservation under `core::content`.
