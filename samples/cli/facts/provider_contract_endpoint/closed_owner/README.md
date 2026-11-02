# Constant arguments inside a generic function

A call inside a generic function must still be checked when that call supplies
only constants. These programs call `Endpoint::upper<13>()` inside
`selected_limit<Unused>()`. The enclosing function has not yet been specialized,
so the call can still refer to the original trait method. The validator must
recognize that method and check its arguments without demanding an executable
version of the enclosing function too early.

A selected checked provider closes limit() to 7; requires value <= limit() carries that bound into exact subtraction 7 - value.

This is another production of the existing `provider_contract_endpoint` fact,
owned by **CANONICALIZE-SCALAR-RANGE-CONTRACTS**. The original generic-wrapper
customer, `tests/omega/pass/generics/folded_range_endpoint_selection_custody`,
encounters this compiler error while checking the standard library's generic
macOS window function. Its individual Objective-C calls supply constant counts.
The repair must let that unchanged customer reach its original runtime result.

| Place | Program | Provider returns six instead of seven |
| --- | --- | --- |
| Local | [positive](local/main.omg) | [rejection](local/invalid/main.omg) |
| Field | [positive](field/main.omg) | [rejection](field/invalid/main.omg) |
| Parameter | [positive](parameter/main.omg) | [rejection](parameter/invalid/main.omg) |
| Call result | [positive](call_result/main.omg) | [rejection](call_result/invalid/main.omg) |
| Match join | [positive](match_join/main.omg) | [rejection](match_join/invalid/main.omg) |
| Constant | [positive](const_name/main.omg) | [rejection](const_name/invalid/main.omg) |
| Nested machine | [positive](other_state/main.omg) | [rejection](other_state/invalid/main.omg) |
| Other machine | [positive](other_machine/main.omg) | [rejection](other_machine/invalid/main.omg) |
| Selected implementation | [positive](target_body/main.omg) | [rejection](target_body/invalid/main.omg) |

Each pair differs only in the selected provider's return value, seven versus
six. The supplied input remains seven, so the rejection must identify that it
exceeds the provider's limit. A caller guard does not establish the subtraction's
bound. Matching copies are retained in `tests/omega/{pass,fail}/facts` as
`provider_contract_endpoint_closed_owner_through_<place>_*`.

The field program also checks neighboring values 31 and 47. The match program
uses both branches, carrying seven and three, and checks subtraction results
zero and four. Taking the same branch twice or overwriting a neighbor therefore
fails at runtime. The selected-implementation program retains the existing
architecture tags, 17 and 29.

The requirement checks cover:

- **Establishment:** the selected provider establishes the limit; the input
  contract establishes the returned scalar's bound.
- **Identity and lifetime:** every call retains its original method and exact
  argument declarations. Constants do not grant storage or borrow authority.
- **Composition:** the same bound reaches the subtraction through all nine
  places, including both match branches and an ordinary returned result.
- **Soundness:** generic bodies still owe argument carriers, bounds, lifetimes,
  and supplied implementation evidence. Executable calls still need their
  concrete selected method.
- **Source to Terminal:** the checked scalar, calls, and branch choices must
  execute with the same result in the Terminal interpreter.
- **Layout and calls:** ordinary scalar and record layouts must retain the
  values and field neighbors across calls.
- **Instructions and storage:** the generated code must preserve the exact
  subtraction and the values saved across helper calls.
- **Native image:** each positive must run with native and Terminal exit zero.
  Build declarations cover all four hosted targets; execution records name the
  host actually used.

The governing rules are [generic applications and specialization](../../../../../wiki/spec/language/generics.md#applications-and-specialization)
and [exact application identity](../../../../../wiki/spec/language/generics.md#closed-applications-and-canonical-identity).
All nine frozen positives refused the call in `selected_limit` with the optimized
`3009cab5a41` producer before the compiler edit. The source, diagnostics, and
later execution records remain in the task's claim evidence. The broader
migration, temporary invariant violations, filesystem tests, and removal of the
old range parser remain unfinished.
