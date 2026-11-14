# Dynamic floating-point return guarantees

A dynamic call returns `0.25` under a public `f64` contract that allows values
from `0.0` through `0.5`. The implementation proves the narrower exact value;
callers use the public contract. Every program checks the observed result and
traps if it differs from `0.25`.

This row belongs to `BUILD-OWNED-TARGET-SELECTION` in [TASKS.md](../../../../TASKS.md).
Four explicit Build rows select `SelectedFloatInvocation`, an ordinary named
implementation of `FloatInvocation`, in place of the former target-prefixed
`invoke` bodies. The dynamic selector
remains an actual `dyn Fraction` argument, and the result remains `f64`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [program](local/main.omg) | [invalid](invalid_local/main.omg) |
| field | [program](field/main.omg) | [invalid](invalid_field/main.omg) |
| parameter | [program](parameter/main.omg) | [invalid](invalid_parameter/main.omg) |
| call result | [program](call_result/main.omg) | [invalid](invalid_call_result/main.omg) |
| match join | [program](match_join/main.omg) | [invalid](invalid_match_join/main.omg) |
| named constant | [program](const_name/main.omg) | [invalid](invalid_const_name/main.omg) |
| nested machine | [program](other_state/main.omg) | [invalid](invalid_other_state/main.omg) |
| another machine | [program](other_machine/main.omg) | [invalid](invalid_other_machine/main.omg) |
| selected implementation | [program](target_body/main.omg) | [invalid](invalid_target_body/main.omg) |

Each twin changes only Quarter's guarantee and return value from `0.25` to
`0.75`. Its body still proves its own guarantee, but that guarantee violates the
public upper bound. Rejection must name `ensures result <= 0.5`.
The match example executes both choices and carries a different route marker
with each selector; selecting the wrong arm changes its result to `0.125`,
which the caller detects. The named constant stores the selector itself.
The existing nested `controls/outside_interval` projects use the same migrated
source and rejection metadata as their adjacent invalid twins.

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

Linux x86-64 qualification is complete for both nine-place rows. Exact source
`51ca56236d0f` has eighteen accepted corpus positives, eighteen intended corpus
refusals and eighteen actual native/Terminal exit-zero pairs. The ordinary
project checks cover eighteen passes and twenty-seven intended refusals,
including the nine existing nested controls. The two complementary runs are
[41754435705](https://github.com/CathedralOS/Omega/actions/runs/41754435705) and
[41756155191](https://github.com/CathedralOS/Omega/actions/runs/41756155191).

The nested controls were then migrated to match their adjacent invalid twins.
[Run 41756934771](https://github.com/CathedralOS/Omega/actions/runs/41756934771)
checks all nine corrected controls for their intended refusal and runs both
root programs with native/Terminal exit-zero agreement at `0f3013a5e471`.
Its verified CLI binary has the same digest as the earlier Linux producer.
No positive, pinned fixture, compiler or library input changed in that edit.
The records keep the source revision at which each observation was made.

The retained macOS ARM64 reports also complete both rows at `51ca56236d0f`:
eighteen accepted positives, eighteen intended corpus refusals, eighteen
actual native/Terminal exit-zero pairs, forty-five ordinary projects
(eighteen passes and twenty-seven intended refusals), and both root programs.
[Run 41756122501](https://github.com/CathedralOS/Omega/actions/runs/41756122501)
hit its one-hour limit and the workflow is canceled. Its retained pilot summary
and all six stages had completed successfully. Preparing the optimized producer
took 2,632 seconds; the workflow is not relabeled successful, and no additional
execution was invented or needed to record these observed results.

The source migration landed in [PR #1486](https://github.com/CathedralOS/Omega/pull/1486)
at `fa33c81d5ef2`. Fetched-main verification confirmed the reviewed source,
all 532 authored input hashes and both recorded files, preserving unrelated
parent records. The macOS observations keep their actual earlier source stamp;
no execution at that publication or a later metadata commit is inferred.
Windows x86-64 and Linux ARM64 now have matching-host qualification for both
rows at published source `9566f10e0d29`. Each host checked eighteen positives
and eighteen intended corpus refusals, then observed eighteen native/Terminal
exit-zero pairs. Each also ran the forty-five ordinary projects: eighteen
exit-zero pairs and twenty-seven intended refusals, including the migrated
nested controls. Both root programs completed with native/Terminal exit zero.
The runs are [Windows 41779728390](https://github.com/CathedralOS/Omega/actions/runs/41779728390)
and [Linux ARM64 41779730966](https://github.com/CathedralOS/Omega/actions/runs/41779730966).
Their audits verify the optimized compiler profiles, all 532 frozen authored
inputs and all 707 distinct authored/copied input hashes per host.

Both nine-place rows now have observed native/Terminal agreement on all four
hosted targets. Programs, Build files, oracles, compiler and library inputs are
unchanged in this record update. Each result keeps its actual observed source;
the earlier Linux x86-64 and macOS results retain their earlier stamps.
Broader dynamic dispatch and target-selection acceptance stays on the task board.

<details><summary>Historical baseline and retained source</summary>

[Baseline run 41475899137](https://github.com/CathedralOS/Omega/actions/runs/41475899137)
checked both original nine-place productions and their eighteen intended
refusals, then observed eighteen native/Terminal exit-zero pairs. The recording
checkout was `c98fe475c9a7`; the actual verified optimized producer was
`1f3c4da0217e`. It is not a run of the migrated source or current main.

The earlier measured core source is retained as a verified Git bundle in
[PR #1486](https://github.com/CathedralOS/Omega/pull/1486#issuecomment-6620126120),
with all thirteen posted parts read back and decoded. Raw reports, producer
profiles and authored input hashes are retained in
`build/dynamic-float-selected-20261104`. No execution is attributed to a later
metadata-only commit.

</details>
