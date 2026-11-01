# Coupled requirement establishes a constructed default domain

Entry facts left > 0 and value + left == 60 establish Counter value <= 60 for the constructed value + 1, making value + 195 exact in u8.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, carrying propositions through
construction. Range suffixes moved bounds into `requires`; a coupled clause
such as `value + left == 60` is the canonical way to state a relation that no
per-parameter interval expresses, so construction must read it.

## Acceptance

`Counter` declares the default domain `requires self.value <= 60`. Each place
constructs `Counter { value: value + 1 }` (or a value carried from it) inside
a machine whose entry facts are `left > 0 && left <= 60 && value <= 60 &&
value + left == 60`. Neither `value <= 60` nor `left > 0` alone bounds
`value + 1`; only the equation does. `total` then computes `value + 195` in
exact u8 from the default domain alone.

`main` calls `next(0, 60)` and `next(59, 1)` and checks both constructed
values and totals (1/196 and 60/255; `match_join` selects `value + 2` on the
`left > 30` arm, so 2/197). A wrong field, a swapped operand or a lost
increment changes an observed value and traps.

Every twin relaxes only the constructing machine's own guard `left > 0` to
`left >= 0`, which admits `value = 60, left = 0`, and must reject with `data
\`Counter\` literal cannot prove the default domain`.

## Places

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| local | [source](local/main.omg) | [twin](local/invalid/main.omg) | `value + 1` is bound to a local before construction. |
| field | [source](field/main.omg) | [twin](field/invalid/main.omg) | `advance` receives `Budget { canary: 9, value, left, tail: 11 }`; its requires and the construction read the fields. |
| parameter | [source](parameter/main.omg) | [twin](parameter/invalid/main.omg) | The construction reads the parameters directly. |
| call_result | [source](call_result/main.omg) | [twin](call_result/invalid/main.omg) | `increment` returns `value + 1` through `ensures result == value + 1`. |
| match_join | [source](match_join/main.omg) | [twin](match_join/invalid/main.omg) | The `left > 30` arm yields `value + 2`; the fallback `value + 1` needs `left > 0`. |
| const_name | [source](const_name/main.omg) | [twin](const_name/invalid/main.omg) | `LIMIT` names 60 in the coupled clause. |
| other_state | [source](other_state/main.omg) | [twin](other_state/invalid/main.omg) | A nested `grow` with no requires constructs from the parent's arrival facts. |
| other_machine | [source](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) | A top-level `grow` owns the construction and its own coupled requires; only its guard changes. |
| target_body | [source](target_body/main.omg) | [twin](target_body/invalid/main.omg) | `Advance::next` is a boundary requirement; Build selects `X86Advance` or `ArmAdvance` per target. The twin relaxes both implementations' guard. |

Pins: `tests/omega/pass/facts/coupled_requires_construction_<place>_exit/` and
`tests/omega/fail/facts/coupled_requires_construction_<place>_rejected/`,
byte-identical to these sources.

## Baseline and repair

At base `b9a2381a9f2`, every positive refused `data \`Counter\` literal cannot
prove the default domain`, while the same entry facts proved a scalar `ensures
result <= 60` for `value + 1`. The structural judge stores the equation as the
rewrite `value + left -> 60`. Its integer-premise index excluded rewrites whose
stored side is a scalar operation, and the equation branch of premise
projection only read mathematical terms, so the relation never reached the
polynomial engine. The repair projects such an equation's sides as operation
trees in their own carrier, each node keeping its formation obligation, and
indexes those rewrites as integer premises. A transition guard
`value + left == 60` and a signed `value - left == -60` coupling reach
construction through the same path.

## Requirements

- **Establishment:** the constructing machine's entry facts or its arrival
  facts establish the domain; the twins show the equation alone is
  insufficient without `left > 0`. No caller guard follows construction.
- **Identity and lifetime:** the facts name the exact entry values; the
  constructed Counter is a fresh copy value with no storage version to
  invalidate.
- **Composition:** calls substitute exact actuals (`advance`, `grow`,
  `increment`, `Advance::next`); the match join retains each arm's own bound;
  the nested `grow` receives its parent's arrival facts.
- **Soundness:** the equation's own `value + left` keeps its u8 formation
  obligation, discharged by `value <= 60` and `left <= 60`. Wrapping
  equations and unbounded operands still refuse.
- **Psi 05–07:** construction, calls, the match and transitions lower to
  Terminal Psi; predicate evidence is erased.
- **Omega 00–09:** the Windows x86-64 host executes all nine positives and
  pins natively and in Terminal Psi with exit 0/0. The other three hosted
  entries are authored, not executed here.
