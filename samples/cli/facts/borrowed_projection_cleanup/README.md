# Borrowed projection cleanup

Seed.code == 7 survives each place into borrowed anonymous storage and its exact normal cleanup, including scalar copies through returned descriptors.

This CML4 row carries the same code fact into cleanup hook prerequisites.
Each place keeps the original projected `Pair` argument to a Unit call, including
its unselected linear sibling. Two additional compositions carry that fact into
`CapturedReceipt::drop`: a scalar read through a returned `CapturedView`, and a
copied descriptor staged through a fixed array into a scalar-returning call.
The counter is zero while the call reads and one after normal completion;
`verify_captured` checks the value and that once-only cleanup in both engines.
It does not supply the hook's static code prerequisite.

Each invalid twin changes only the transported code to 6, keeping both hooks'
code-7 prerequisites. The pinned invalid oracles require the new
`CapturedReceipt::drop` refusal as well as automatic-cleanup rejection. The root
retains ordered construction, explicit/implicit loans, returned aggregates and
fixed-array projections alongside these descriptor compositions.

## Places

The nine places carry `Seed.code == 7` into the borrowed projection of an anonymous Pair child and verify deterministic cleanup:

| Place | Positive | Reject twin | Transport |
| --- | --- | --- | --- |
| root | [main.omg](main.omg) | — | original witness: ordered effectful construction, explicit/implicit loans, returned aggregate and fixed-array projection |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) | local `Seed` binding supplies code 7 |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) | record field (`Holder.seed`) supplies code 7 |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) | callee machine parameter receives code 7 |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) | helper call returns code 7 |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) | match branches converge on code 7 |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) | named constant supplies code 7 |
| a nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) | nested-machine parameter carries code 7 |
| another machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) | intermediate machine forwards code 7 |
| another target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) | per-target machine body produces code 7 |

## Requirements and evidence

Canonical authority: [source ownership](../../../../wiki/spec/language/ownership.md#construction-and-disposal-order),
[normal continuation ownership](../../../../wiki/spec/terminal-psi/ownership.md#roots-continuations-and-order),
and [loan closure](../../../../wiki/spec/terminal-psi/loans.md#reborrow-lineage-and-access).

| Requirement | Witness and limit |
| --- | --- |
| Establishment | The transported code enters the anonymous value before borrowing; the hook must prove code 7 at its actual death. |
| Identity and lifetime | Returned descriptors keep original backing. A copied scalar detaches from its carrier; independently live references keep their loans. Counter observations distinguish premature, missing and duplicate hooks. |
| Composition | Both descriptor productions occur at the consuming site in all nine places, retaining the original Unit-call/linear-sibling control. |
| Soundness | All nine code-6 twins reject at the unchanged cleanup prerequisites. Separate borrow controls retain a live descendant and reject early backing access. |
| Psi 05–07 | Checked completion contexts and ordered actions survive into Terminal Psi; actual execution must return 0. |
| Omega 00–02 | Each public project binds all four hosted entries. The measured host realizes pointer, record and fixed-array geometry and calling plans. Foreign entry bindings alone are not execution evidence. |
| Omega 03–06 | Host execution exercises descriptor carriers, live backing storage and hook calls through instruction selection and frame realization. |
| Omega 07–09 and emission | Native exit 0 must agree with Terminal exit 0 on the measured host. This does not claim Windows/Linux execution from macOS. |

## Measured run record

Producer `00e629d9481e09e3db2be7896a087109e664450e`, opt-level 1,
`macos_arm64`, integrated on published base `7a96cd9a433`:

- Root plus all nine public places run with native exit 0 and Terminal exit 0;
  all nine public invalids require and match `CapturedReceipt::drop clause`.
- The nine mirrored corpus places and the two retained descriptor-completion
  fixtures run at 0 in both engines. At baseline `d25dcf177ece`, the added
  direct composition refused for missing exact context, and the larger existing
  call compositions refused disposal while a wrapper reference remained live.
- The affected source comparison covers 128 fixtures without lost outcomes.
  The target comparison covers 65: 63 native executions and 62 actual
  native/Terminal agreements, including one existing 42/42 control. Two are
  checked-only; one foreign-`abs` fixture still has no trusted Terminal host
  implementation on both baseline and candidate. The unchanged source
  expected-fragment gap also remains outside this repair.
- Changing only the runtime counter oracle from 1 to 2 produces native signal 5
  and Terminal `Trap`, so missing or duplicate cleanup cannot satisfy the oracle.
- All ten accepted project locks stay stable during the final public replay.
  Parent acceptance was prepared after its nested projects; no accepted lock was
  borrowed into another checkout.

Earlier Unit-call records at `cc2f948d148` and `9ff26a87021` are historical
coverage of that production, not execution of these new compositions.

## Remaining scope

CML4's cross-host qualification and suspension acceptance stay open. A separate
retained linear-`Pair` scalar-supplier composition still produces native exit 0
but Terminal `AffineFrontierMismatch` at block 8 on this producer, as it did at
baseline. This repair does not claim that residual-claim composition is complete.

```sh
omega run --both samples/cli/facts/borrowed_projection_cleanup/local/main.omg
omega --check samples/cli/facts/borrowed_projection_cleanup/local/invalid/main.omg
python3 tools/sample_native_gate.py --facts --filter borrowed_projection_cleanup --cli <omega>
python3 tools/corpus_gate.py --both --filter borrowed_projection_cleanup_through_
```
