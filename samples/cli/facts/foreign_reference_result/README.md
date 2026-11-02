# Foreign reference result

A foreign reference result retains the captured input backing and access through ordinary transport, so its provider write is visible and a guest write through the result changes that input while preserving its neighbors.

Parent: [TERMINAL-BUILD-SEED](../../../../TASKS.md).

The ordinary C `memset` binding publishes the precondition `count == 8`.
That bounds its write to the first `u64` of a three-word `Cell`; all bit patterns
are valid. It always returns its valid destination under this full requirement.
The provider fills that word with byte 37, producing `0x2525252525252525`.
The consumer first observes this write, then writes 41 and 73 through the returned
reference. Both independent caller cells must finish at 73, and four neighboring
canaries must stay unchanged. The match place executes both arms and distinguishes
marker writes 11 and 13. The selected implementation contains the runtime oracle;
the unselected machine named `main` traps.

The nearest row, [foreign_scalar_record_result](../foreign_scalar_record_result),
returns owned scalar payloads. This row returns an alias of existing mutable
storage, so copying payload bits cannot satisfy it. Its reject twin removes only
`mut` from the boundary input and rejects because that input cannot supply the
returned reference's access. [build_activation_borrow](../build_activation_borrow)
transports checked guest-call loans; this row crosses an opaque native host call
and must correlate the actual returned address with an admitted captured endpoint.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [source](local/main.omg) | [control](local/invalid/main.omg) |
| Field | [source](field/main.omg) | [control](field/invalid/main.omg) |
| Parameter | [source](parameter/main.omg) | [control](parameter/invalid/main.omg) |
| Call result | [source](call_result/main.omg) | [control](call_result/invalid/main.omg) |
| Match join | [source](match_join/main.omg) | [control](match_join/invalid/main.omg) |
| Const name | [source](const_name/main.omg) | [control](const_name/invalid/main.omg) |
| Nested machine | [source](other_state/main.omg) | [control](other_state/invalid/main.omg) |
| Other machine | [source](other_machine/main.omg) | [control](other_machine/invalid/main.omg) |
| Selected implementation | [source](target_body/main.omg) | [control](target_body/invalid/main.omg) |

## Requirements and evidence

- Establishment: an exact mutable input and the public returned-lifetime contract
  establish the permitted region. The actual C result supplies the address;
  neither the origin list nor a numeric host identity may choose one.
- Identity and lifetime: the result must retain the original captured allocation,
  access and parent custody after host completion and through ordinary calls.
  A distinct invocation's selector cannot borrow this invocation's storage.
- Composition: all nine places transport the same returned loan and consume the
  same provider write and guest write obligations. The paired source fixtures
  live under `tests/omega/{pass,fail}/facts/foreign_reference_result_through_*`.
- Soundness boundaries: each one-token shared-input control must reject before
  effects. Host admission, exact capture identity, access attenuation, typed
  endpoint bounds and source-region checks remain independent obligations.
- Psi 06–08: checking must retain the public result regions; lowering consumes
  those plans mechanically. Canonical encoding, receiving validation and
  interpreter completion must retain the descriptor and original backing.
- Omega 00–02: ordinary four-target Build rows and the exact C calling plans
  carry the pointer result, 32-bit fill value and 64-bit bounded count. Native
  layout and host original-storage mappings supply typed endpoint geometry.
- Omega 03–06: omitting the C call loses the first observed fill; copying the
  referent or using the wrong home loses the caller write; wrong offsets damage
  the independent canaries. Both match choices are runtime counterfactuals.
- Omega 07–09: direct native images and loaded Terminal artifacts must both
  execute the oracle on matching hosts. An interpreter decline, even beside a
  wrapper exit 0, is not execution agreement.

## Baseline and remaining acceptance

At fixed source `f243fb583a50596be3287aafefdd864d3d3eaa58`, all nine positives
check and all nine shared-input controls reject. The bounded root probe executes
natively with exit 0, including the observable provider write. `--both` reports
`interp: DECLINED (native-provider-adapter: native value ABI needs plain scalar or finite record/array storage)`;
its wrapper exit 0 is retained as a red Terminal baseline.

Raw profiles, source digests, commands and outputs are retained in the continuing
claim's `build/opaque-host-reference/fill-{baseline,row-baseline-checks}.json`.
The row is unfinished until all nine native/Terminal agreements and the reject
controls qualify. Matching Windows, macOS and Linux ARM64 execution remains
unmeasured at this checkpoint. General host-owned allocations, unbounded native
addresses and unrelated resource/qualification issuance are not supplied by this
captured-input carrier.
