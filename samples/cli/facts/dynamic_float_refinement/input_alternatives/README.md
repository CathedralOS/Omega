# Public IEEE input alternatives

Inline exact IEEE value `0.25` guarantee refines public inclusive f64 interval
`[0, 0.5]`; the dyn call preserves the public interval for a precondition and observes `0.25`.

Owning outcome: **RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY** in
[TASKS.md](../../../../../TASKS.md), public requirement-contract implication.

Difference from [the input-alias production](../input_alias/README.md): the
caller's input predicate admits either the exact interval `[0.125, 0.125]` or
`[0.25, 0.25]`. Both alternatives independently imply the public `[0, 0.5]`
requirement. Every place invokes both inputs; both still return the quarter.
The selected implementation's private result guarantee cannot supply the input
premise or strengthen the caller's public result contract.

At fixed base `eb9acb01452251ddb0c338fa400fb9cd476c8bc4`, optimized CLI
`68e71955520e14f15e686f4cb7c867d04d15246c178d5f22cfc3d29f89098ec7`
refuses both the equality-alternative and interval-alternative probes at
`cannot prove requires contract for call dyn Fraction::get from probe`.
The unchanged single-interval input alias checks on that same producer.

The checker retains these alternatives as bounded private entailment worlds.
Each world must imply the requested bound on the same completed operand and
IEEE format. Checked mathematical definitions and executed Boolean connectives
share that query, while leaves retain their authenticated selected comparisons.
Unknown leaves preserve a possible world; neither false order nor inequality
grants a non-NaN bound. Private worlds never establish operation formation or
enter the surrounding proof context. Exhausted search declines the proof.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [positive](local/main.omg) | [widened input](local/invalid/main.omg) |
| field | [positive](field/main.omg) | [widened input](field/invalid/main.omg) |
| parameter | [positive](parameter/main.omg) | [widened input](parameter/invalid/main.omg) |
| call_result | [positive](call_result/main.omg) | [widened input](call_result/invalid/main.omg) |
| match_join | [positive](match_join/main.omg) | [widened input](match_join/invalid/main.omg) |
| const_name | [positive](const_name/main.omg) | [widened input](const_name/invalid/main.omg) |
| other_state | [positive](other_state/main.omg) | [widened input](other_state/invalid/main.omg) |
| other_machine | [positive](other_machine/main.omg) | [widened input](other_machine/invalid/main.omg) |
| target_body | [positive](target_body/main.omg) | [widened input](target_body/invalid/main.omg) |

Every twin widens only the second interval's upper endpoint from `0.25` to
`0.75`. Both actual entry inputs remain valid, while the public selector call
must reject the newly admitted outside inputs. Source and Build inputs are
byte-identical to their `tests/omega/{pass,fail}/facts/` pins. All four hosted
entries are authored; matching-host execution remains a separate observation.

The match place retains distinct marker results 11 and 13 and executes both
selector arms with both inputs. The field place verifies neighbors 17 and 29.
The target place retains ordinary named implementations selected by four
explicit target-keyed Build rows. Each consuming guard traps on a wrong quarter,
marker or neighbor; no guard re-establishes a missing static contract.

[Equality alternatives](controls/equal/main.omg) exercise the same prefix rule.
[The NaN branch](controls/nan_branch/main.omg) admits `input != input` and must
reject. [False ordered comparisons](controls/false_order_branch/main.omg)
admit NaN even though finite inputs lie in the public interval; integer
complements must not prove this call. [The f32 production](controls/f32_rounded/main.omg)
uses an endpoint exactly halfway above `0.25`, which lands back to `0.25` in
f32. [Its strict-bound control](controls/f32_strict/main.omg) requires the input
strictly below that endpoint and must reject the admitted `0.25`.

- **Establishment:** every admitted prefix alternative implies the public literal bounds; implementations independently establish their quarter-valued result.
- **Identity and lifetime:** exact completed operands and IEEE formats survive definition unfolding, selector forwarding and storage; no branch supplies a new observation authority.
- **Composition:** local, field, parameter, call result, both joins, constant, nested machine, forwarding machine and Build-selected target carry the same selector and contract.
- **Soundness:** one outside alternative, NaN, false order, format rounding and strict endpoints retain their own refusal boundaries; wanted conclusions do not form operands.
- **Psi 05–07:** proof worlds erase; ordinary float operands and separate selector lanes produce executable Terminal Psi.
- **Omega 00–02:** existing row, float argument/result, record and calling plans realize the same public shape on each authored hosted entry.
- **Omega 03–06:** selected calls, float registers, live selectors and neighbor storage must preserve the observed quarter and marker.
- **Omega 07–09 and emission:** image encoding and dispatch relocations must execute with matching native and Terminal crash/exit oracles.

The wider parent remains open for its other proof mechanisms, operational
envelopes, Product compiler parity and complete table/other-host qualification.
