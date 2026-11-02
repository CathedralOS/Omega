# Indexed borrowed field production

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, recursive/reference
predicate read inventories. This is another production of
[data_where_standing](../../README.md), retaining the bound and exact operation
of the [nested borrowed-field production](../nested/README.md).
`Holder.counters` stores two shared references to `Counter`; each Counter stores
a shared scalar reference. The public consumer requires only `index < 2` to
form the indexed selection, then calls `read(holder.counters[index].value)`.
The checked result equation and Counter's standing predicate must justify
`total = 70 + value` before any runtime canary guard.

| Place | Positive | Predicate-removal twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [twin](local/invalid/main.omg) |
| field | [program](field/main.omg) | [twin](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| call_result | [program](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| match_join | [program](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| const_name | [program](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| other_state | [program](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| other_machine | [program](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| target_body | [program](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

Every twin removes only Counter's default predicate. The dynamic selector,
index formation requirement, layouts, transport and runtime oracle are identical.
The const-name production retains the independent HIGH/LOW Backing constants;
those concrete callers cannot establish the arbitrary public consumer's bound.

The local and parameter places retain separate local-copy and incoming-value
routes. Field stores the Holder in Envelope; call_result receives an owned Holder
from ordinary relay. Match joins distinct original/swapped reference arrays and
executes both arms with both selector values. Other_state terminal-transfers
Holder and selector into a genuinely nested machine; other_machine forwards
both to a separate consumer. Target_body selects ordinary named implementations
and independent 17u64/29u64 tag checkers through all four hosted Build bindings.

Each entry checks high=60 and low=-17, exact results 130/53, both Counter
before=17/after=53 fields, distinct markers 19/23, and Holder prefix=31/suffix=47.
Both array orders and both selector positions are observed. The field entry
also checks the transported Envelope's Holder. The consumer checks its actual
transported Holder canaries after the arithmetic, detecting collateral copies
without supplying an arithmetic premise. Wrong selector/array order, omitted
loads, branch substitution or lost descriptor transport must fail the oracle.

Establishment, exact descriptor/value identities, complete possible source
inventories, readable access and lifetimes remain separate obligations. No
reference type or index alone authorizes its pointee.

Exact main/Build copies are pinned under
`tests/omega/{pass,fail}/facts/data_where_standing_borrowed_field_array_<place>`.
Positive pins author native and interpreter exit-zero oracles; each negative
pins the intended exact-i32 overflow diagnostic. These are required outcomes,
not passing observations.

The source freeze was observed at workbench
`d054dabcbcf02f1c3926476c206cac39a863e16d`. Retained optimized R14b producer
source `96f561907a23aea7fbb6f6254ce421a50dd8d436`, executable SHA-256
`3b83b5ee9cb90beb20e02195655e4e725bdb19b9c9eff98fc48d8007842d0aa3`,
checks all 18 frozen projects: every valid positive wrongly refuses the exact
addition, and every predicate-removal twin refuses that same overflow
obligation. These baseline source checks execute neither engine. Runtime
agreement and repaired positive checking remain pending.

Two focused rejection controls keep Counter's bound and isolate independent
obligations without the blocked addition:

- [Wrong selected equality](controls/wrong_selected_equality/invalid/main.omg)
  requires `index < 2` and returns the actual indexed read, but promises equality
  to element zero. Main selects element one from distinct values 60 and -17.
  R14b rejects the public result guarantee; a selected reference cannot donate
  a different element's identity.
- [Missing index bound](controls/missing_index_bound/invalid/main.omg) omits
  only the selector's formation requirement from its public read. Counter's
  scalar bound cannot establish an arbitrary index is within length two.
  R14b rejects that indexed selection.

The controls are pinned as
`tests/omega/fail/facts/data_where_standing_borrowed_field_array_{wrong_selected_equality,missing_index_bound}`.
Their first direct-tail-read versions stopped at an unrelated borrow-resource
lowering refusal. Those source versions and raw attempts remain historical;
the explicit local read bindings reach the intended rejection reasons above.
Baseline source hashes and raw command receipts are retained under
`build/scalar-collection-read-20261102` outside the tracked source tree.
No passing runtime or other-host qualification is claimed. The parent migration
and GUI qualification remain open.
