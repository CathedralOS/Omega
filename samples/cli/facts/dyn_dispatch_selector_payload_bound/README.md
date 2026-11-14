# Dynamic selectors through selected target code

A program must call the implementation its runtime selector names and keep the
result bound that makes later arithmetic safe. These examples call two lawful
implementations, check their distinct results, and use the same public
`result <= 88u64` guarantee to prove `88u64 - result` safe.

Owning tasks: **BUILD-OWNED-TARGET-SELECTION**, replacing the remaining target
prefixes with ordinary named implementations chosen in Build; and
**RESTORE-DYNAMIC-DESCRIPTOR-AND-TABLE-CUSTODY**, preserving dynamic selection
and the complete callable table through compilation and execution.

Fact: Dispatching a trait method through a dynamic trait carrier (dyn / trait conformance) whose contract establishes result <= 88u64 preserves the return bound across the selector and data lanes, discharging exact subtraction 88u64 - result.

`ActualEvaluator` returns 38 for input 10 and 28 otherwise. `AlternateEvaluator`
calls a helper returning 48 for input 10 and 58 otherwise. Every positive
executes both selectors with inputs 10 and 20, expecting differences 50, 60,
40 and 30. A wrong selector, dropped input, inverted branch or omitted helper
cannot satisfy those checks. The field place also checks the surrounding
17/23 fields. Runtime comparisons observe the computed results; they do not
supply the bound used to form the subtraction.

The [named-predicate production](alias_contract/README.md) expresses that same
public bound as `within_payload_bound(result, 88u64)`. Both productions carry
the same fact through all nine places:

| Place | Direct bound | Named predicate |
| --- | --- | --- |
| Local | [local](local/main.omg) / [reject](invalid_local/main.omg) | [program](alias_contract/local/main.omg) / [reject](alias_contract/local/invalid/main.omg) |
| Field | [field](field/main.omg) / [reject](invalid_field/main.omg) | [program](alias_contract/field/main.omg) / [reject](alias_contract/field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) / [reject](invalid_parameter/main.omg) | [program](alias_contract/parameter/main.omg) / [reject](alias_contract/parameter/invalid/main.omg) |
| Call result | [call_result](call_result/main.omg) / [reject](invalid_call_result/main.omg) | [program](alias_contract/call_result/main.omg) / [reject](alias_contract/call_result/invalid/main.omg) |
| Match join | [match_join](match_join/main.omg) / [reject](invalid_match_join/main.omg) | [program](alias_contract/match_join/main.omg) / [reject](alias_contract/match_join/invalid/main.omg) |
| Const name | [const_name](const_name/main.omg) / [reject](invalid_const_name/main.omg) | [program](alias_contract/const_name/main.omg) / [reject](alias_contract/const_name/invalid/main.omg) |
| Nested machine | [other_state](other_state/main.omg) / [reject](invalid_other_state/main.omg) | [program](alias_contract/other_state/main.omg) / [reject](alias_contract/other_state/invalid/main.omg) |
| Other machine | [other_machine](other_machine/main.omg) / [reject](invalid_other_machine/main.omg) | [program](alias_contract/other_machine/main.omg) / [reject](alias_contract/other_machine/invalid/main.omg) |
| Selected target implementation | [target_body](target_body/main.omg) / [reject](invalid_target_body/main.omg) | [program](alias_contract/target_body/main.omg) / [reject](alias_contract/target_body/invalid/main.omg) |

The [root](main.omg) and [named-predicate root](alias_contract/main.omg) use the
local form. Every positive binds all four hosted targets and expects exit 0
in native and Terminal Psi execution.

Each reject twin keeps the executable bodies, inputs, subtraction and runtime
checks unchanged. It weakens only the published result bound from 88 to 89 on
the requirement and both implementation members. A result permitted to be 89
cannot justify `88 - result`; rejection must name that exact-arithmetic
obligation. The harmless bodies do not make that weaker promise sufficient.

## Build selection

Both `target_body` projects call `SelectedCompute::compute`, passing the dynamic
selector and its ordinary integer input separately. Explicit Build rows choose
`X86Compute` for the two x86-64 targets and `ArmCompute` for the two ARM64 targets.
The entry supplies tag 17 or 29 respectively; each selected implementation
checks its own tag before dispatching. Selecting the other implementation
therefore traps. No source machine declaration carries a target prefix.

The requirement permits that tag-check crash, declares no service use, and
retains the same dynamic call followed by `88 - result`. The helper and both
target entries declare empty service reach with bare `reaches`, and name the
synchronous call separately with `invokes SelectedCompute`.

Ordinary named implementations and Build rows follow [provider selection](../../../../wiki/spec/build/provider_selection.md#build-owned-target-selection)
and [multi-target compilation](../../../../wiki/spec/build/configuration.md#multi-target-compilation).
This follows [service reach and direct invocation](../../../../wiki/spec/language/effects.md).
The caller's selector remains independent of Build's target choice, under
[dynamic dispatch](../../../../wiki/spec/terminal-psi/dynamic_dispatch.md).

## Requirements checked by these programs

- **Establishment:** checked implementation members prove the public result
  bound. The named predicate reduces to the same integer comparison. No caller
  equality check establishes a missing guarantee.
- **Identity and lifetime:** the exact selector and explicit input remain
  independent values across locals, fields, calls and nested machines. Selecting
  code creates no hidden data receiver or borrowed instance.
- **Composition:** every call uses its own selector, input and public result
  promise. Both match branches execute and select different implementations.
  The weaker-bound twins test the consuming subtraction directly.
- **Soundness:** public contracts and ordinary conformance refinement govern
  the call. Erased predicate reasoning grants no access, storage or ownership.
- **Checked to Terminal Psi:** the portable program retains the selector,
  integer argument, indirect call and result. The parent production keeps its
  existing `ControlFlowCleanup` selection; AlternateEvaluator's separate helper
  must survive with the dynamic table's callable dependencies.
- **Providers, layout and calling convention:** Build records four explicit
  target choices. The dynamic selector occupies its own pointer-sized value;
  the integer arguments and result retain their ordinary calling convention.
- **Instructions and stack storage:** indirect calls preserve selector and data
  arguments, then perform the same integer subtraction. Field neighbors check
  that the selector projection uses the intended slot.
- **Encoding and execution:** the emitted executable must produce the authored
  result and agree with Terminal Psi. Source checking or successful emission
  alone does not establish this requirement.

## Validation and remaining scope

The fixed starting revision is `2da2c5b57f1966e0989001b12d805d6d477113e4`.
The first hosted run uses documentation-only checkpoint `25686539ece3` with
unchanged compiler, library and authored program inputs. Earlier passing target
prefixes are migration inputs, not a newly discovered compiler defect.
The unchanged baseline passes all 36 corpus fixtures and 38 sample projects on
Linux x86-64. Migration checkpoint `8a297d8025c2` exposed a separate checking
error: the two target places reject because the compiler mistakes their first
ordinary dynamic-valued argument for the outer call's selector. The other
sixteen corpus positives execute with native/Terminal exit 0, and all eighteen
bound controls reject. Neither target place executes, and the sample leg does
not run after that failure.

The shared call classifier now reads the requirement's declaring owner first.
A boundary call forwards every authored argument; only the later local trait
call consumes its separate selector. The [wrong-trait control](controls/wrong_trait/main.omg)
keeps a genuine attempt to call an unrelated trait invalid. This fixes the
existing call rules rather than changing the selector or public result bound.

Repair revision `dbde2a0eb0ce5a77198bc4ee3f26d5253cf4a56c` passes the complete
focused selection on [Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41538946198)
and [macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41538950836).
On each host, source checking accepts twenty programs and rejects all twenty-one
controls for their intended reasons. All eighteen fact fixtures execute with
native and Terminal exit 0. The two existing dynamic-call regression fixtures
also agree between engines: one exits 0 and the other exits 70 as authored.
All thirty-nine sample projects meet their oracles: twenty execute successfully
and nineteen reject as intended. The eighteen weaker-bound twins reject at the
exact-arithmetic obligation on the subtraction; the unrelated-trait control
rejects because its selector does not inherit the called trait.

The migration selection comprises 36 corpus fixtures and all 38 original sample
projects: both roots, eighteen positive places and eighteen reject twins.
The compiler repair adds a wrong-trait control and four existing dynamic-call
regressions, bringing its focused selection to 41 fixtures and 39 sample projects. Each
sample Main/Build pair is copied to its corresponding fixture under
`tests/omega/{pass,fail}/facts/dyn_dispatch_selector_payload_bound_through_*`
or `dyn_dispatch_payload_alias_through_*`. Reject diagnostics and raw execution
records retain their actual producer revision and host in the claim evidence.
Both producers use verified optimization level 1. The earlier Source and combined
execution records name the measured repair revision; later documentation,
explanatory comments and record updates do not claim a new execution.
Windows x86-64 and Linux ARM64 execution remain unverified.

The later correction to service declarations is measured separately at
`811f0cc440abdab14a264b198d85a8f012f830d0` on
[Linux x86-64](https://github.com/CathedralOS/Omega/actions/runs/41571522743) and
[macOS ARM64](https://github.com/CathedralOS/Omega/actions/runs/41571525529).
On each host, both valid target programs check and exit 0 in native and Terminal
execution; both bound-89 controls report the intended subtraction error in
both provider bodies. All four affected ordinary projects meet their oracles.
The verified producers use optimization level 1. All bounds, bodies, inputs, target tags,
Build rows and expected results remain unchanged by this correction.

This follow-up measures only the four affected target projects. Other places
retain the earlier observations above. Four owned checking records and two owned
combined-execution records per host now name the measured correction source;
every unrelated record remains unchanged. The
[published source archive](https://github.com/CathedralOS/Omega/pull/1613#issuecomment-6594622141)
retains that exact source. Later documentation and record updates do not claim
another execution. The former wider service ceiling was a valid upper bound,
so this source correction is not presented as a compiler defect.

This migration does not close either parent task. Remaining source families,
provider admission and layout/callback requirements stay on the target-selection
board item; broader dynamic operational contracts and cross-host realization
remain on the dynamic-selection item.
