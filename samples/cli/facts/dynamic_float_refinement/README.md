# Dynamic floating-point return guarantees

A dynamic call returns `0.25` under a public `f64` contract that allows values
from `0.0` through `0.5`. The implementation proves the narrower exact value;
callers use the public contract. Every program checks the observed result and
traps if it differs from `0.25`.

This row belongs to `BUILD-OWNED-TARGET-SELECTION` in [TASKS.md](../../../../TASKS.md).
Its remaining migration replaces the four identical target-prefixed `invoke`
bodies with `SelectedFloatInvocation`, an ordinary named implementation of
`FloatInvocation`. Four explicit Build rows select it. The dynamic selector
remains an actual `dyn Fraction` argument, and the result remains `f64`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [invalid](local/invalid/main.omg) |
| field | [program](field/main.omg) | [invalid](field/invalid/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid](parameter/invalid/main.omg) |
| call result | [program](call_result/main.omg) | [invalid](call_result/invalid/main.omg) |
| match join | [program](match_join/main.omg) | [invalid](match_join/invalid/main.omg) |
| named constant | [program](const_name/main.omg) | [invalid](const_name/invalid/main.omg) |
| nested machine | [program](other_state/main.omg) | [invalid](other_state/invalid/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid](other_machine/invalid/main.omg) |
| selected implementation | [program](target_body/main.omg) | [invalid](target_body/invalid/main.omg) |

Each twin changes only Quarter's guarantee and return value from `0.25` to
`0.75`. Its body still proves its own guarantee, but that guarantee violates the
public upper bound. Rejection must name `ensures result <= 0.5`.
The match example executes both choices and carries a different route marker
with each selector; selecting the wrong arm changes its result to `0.125`,
which the caller detects. The named constant stores the selector itself.

The [named-predicate production](alias_contract/README.md) carries the same
public interval into a caller precondition. The already ordinary
[input predicate](input_alias/README.md) and
[input alternatives](input_alternatives/README.md) productions are unchanged.

The applicable requirements are:

- Construction and refinement: Quarter proves `result >= 0.25 && result <= 0.25`,
  which must imply both public bounds. A later runtime comparison supplies no
  proof premise.
- Identity and lifetime: storing, copying, passing and returning a selector keeps
  the chosen implementation. It grants no access to an implied data object.
- Composition: the call preserves the public result contract. The named-predicate
  production must pass that result directly to its constrained consumer.
- Soundness: the `0.75` twins refuse the public upper bound even when their
  implementation is not the selected target provider.
- Terminal Psi: the artifact retains the dynamic selector separately from its
  explicit arguments and executes the same floating-point call.
- Provider and calling plans: Build chooses the ordinary wrapper for each target;
  the selected implementation preserves the declared parameter and result types.
- Native lowering and image emission: both engines must observe the authored
  result on a matching host. Compilation or cross-target installation alone
  does not count as native execution.

The governing contracts are [dynamic dispatch](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md),
[named implementations](../../../../wiki/spec/language/conformances.md),
[numeric values](../../../../wiki/spec/language/numeric_values.md) and
[Build-owned target selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection).

Pins are `tests/omega/{pass,fail}/facts/dynamic_float_refinement_through_<place>_*`.
Every positive, including [the root program](main.omg), binds all four hosted
targets and declares exit zero. Run a place with `omega run --both <place>/main.omg`.

The migrated programs have not yet been qualified. Historical Linux baseline
[run 41475899137](https://github.com/CathedralOS/Omega/actions/runs/41475899137)
checked both nine-place productions and their eighteen intended refusals, then
observed eighteen native/Terminal exit-zero pairs. That run used recording
checkout `c98fe475c9a7` and the verified optimized producer `1f3c4da0217e`;
it is not a run of the migrated source or current main. Current scoped checking,
execution, independent review and landing remain open. Broader dynamic dispatch
and target-selection acceptance stays on the task board.
