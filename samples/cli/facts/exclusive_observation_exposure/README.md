# Complete non-writing frames preserve saved observations

A saved observation that items[0] is 7 stays provable across a complete, non-writing exclusive call on that element.

The consuming operation is `agree(observed, current)` with `requires observed == current && observed == 7`, after a saved read of `items[0]` on `[7, 1]` and an exclusive call borrowing `items[0]` without storing. Exit 0 means the saved observation remains valid. The reject twin changes only `expose` by adding `slot = 8`, which invalidates the live equality.

Parent outcome: `STATE-LOCAL-VALUE-FRONTIER` (`TASKS.md`). [Invocation contracts](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity) preserve facts under complete empty summaries. [Mutation frames](../../../../wiki/spec/language/dependent_values.md#mutation-frames) allow checked implementations to derive narrower frames than their exclusive signature ceiling. Permission to store is not an actual store; unknown effects still use their conservative ceiling.

## Baseline and acceptance

On current main `d545ec03095b`, each overlapping no-store program (formerly labeled invalid) wrongly refuses its `agree` contract. The same synthetic-store broadening also refuses the existing direct-choice and nested-machine disjoint-bound samples. Historical refusal labels were not language authority: a known complete empty body frame preserves these facts.

Acceptance is `omega run --both <place>/main.omg` exiting 0 in both engines for all nine places, and each `omega --check <place>/invalid/main.omg` rejecting the actual overlapping write with `cannot prove requires contract`. Producer-bound observations live in the owning qualification/claim record; no unexecuted target is claimed here.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`local/invalid/main.omg`](local/invalid/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`field/invalid/main.omg`](field/invalid/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`parameter/invalid/main.omg`](parameter/invalid/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`call_result/invalid/main.omg`](call_result/invalid/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`match_join/invalid/main.omg`](match_join/invalid/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`const_name/invalid/main.omg`](const_name/invalid/main.omg) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`other_state/invalid/main.omg`](other_state/invalid/main.omg) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`other_machine/invalid/main.omg`](other_machine/invalid/main.omg) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`target_body/invalid/main.omg`](target_body/invalid/main.omg) |

## Requirements and evidence

The [`case_payload`](case_payload/main.omg) and [`generic`](generic/main.omg)
mechanism probes preserve the saved case payload or generic field through the
same non-writing call. Their respective [`case twin`](case_payload/invalid/main.omg)
and [`generic twin`](generic/invalid/main.omg) add only the overlapping store.
These focused probes do not close the parent outcome's remaining composition.

The [`reference`](reference/main.omg) probe and the six [`float`](float/main.omg)
places (local, field, parameter, match join, const name, a nested machine) use that
same complete non-writing call on the observed referent. Each twin adds only
the overlapping store. On `8bd3d5732111`, these non-writing same-place sources
wrongly refuse their saved equality; their former invalid labels do not justify
treating permission as a store. Producer-bound baseline observations retain the
original sources and diagnostics.

Float call result, other machine, and target body carry a saved observation
that the kept float is 7 across an exclusive loan of the other float. The
callee reads that saved record. Each twin stores through the loan of the kept
float. Fresh optimized `60453171ca5f23df9feff9c236a4667d47f4456f` executes
all nine float positives with native and Terminal exit 0 and rejects all nine
actual overlapping-write twins. The earlier equality refusals are historical,
not the current frontier; no additional float repair was needed. The
call-result, other-machine and target-body fixture pairs are now pinned under
`tests/omega/{pass,fail}/facts/exclusive_observation_exposure_through_float_*`.
Their source and Build companions match these sample pairs byte-for-byte.
This pinning observation adds no new matching-host runtime qualification.

- **Establishment.** The observation is the read `items[0]` of the literal `[7, 1]`. Where that read is in another machine or target body, the callee requires `items[0] == 7` and the caller proves it from the literal. The consuming `agree` requires the saved value still equal the live element and 7. The exclusive loan does not establish the fact; the read does.
- **Identity and invalidation.** The complete non-writing call preserves the exact observed element. The twin's actual store overlaps that element and invalidates equality with its saved value. Disjoint actual writes remain covered by the choice rows.
- **Composition.** The nine places carry that same sentence. Parameter, call result, other machine, and target body pass the saved value or the array through a contract that names the element. Field, match join, const name, and a nested machine keep the read in the lending state. The positive and reject twins differ only by the store in `expose`.
- **Soundness.** The loan is ordinary exclusive access. No mathematical binding is executed. Proof retirement follows actual write footprints; no frame grants borrow authority.
- **Checked-to-Terminal Psi.** The positive places execute in the Terminal Psi interpreter and agree with native execution. The reject twins fail checking, so they have no Terminal Psi product.
- **Later realization.** Each positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`. Current native/Terminal execution evidence is host-specific; bindings alone do not establish execution on the other hosts.

This row covers complete non-writing frames over saved array-element observations, with actual overlapping-write twins. Case, generic and reference probes and the float places do not close the rest of `STATE-LOCAL-VALUE-FRONTIER`.
