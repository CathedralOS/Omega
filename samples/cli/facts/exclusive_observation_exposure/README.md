# Complete non-writing frames preserve saved observations

A saved observation that items[0] is 7 stays provable across a complete, non-writing exclusive call on that element.

The consuming operation is `agree(observed, current)` with `requires observed == current && observed == 7`, after a saved read of `items[0]` on `[7, 1]` and an exclusive call borrowing `items[0]` without storing. Exit 0 means the saved observation remains valid. The reject twin changes only `expose` by adding `slot = 8`, which invalidates the live equality.

Parent outcome: `STATE-LOCAL-VALUE-FRONTIER` (`TASKS.md`). [Invocation contracts](../../../../wiki/spec/language/state_contracts.md#mutation-and-subject-identity) preserve facts under complete empty summaries. [Mutation frames](../../../../wiki/spec/language/dependent_values.md#mutation-frames) allow checked implementations to derive narrower frames than their exclusive signature ceiling. Permission to store is not an actual store; unknown effects still use their conservative ceiling.

## Baseline and acceptance

At recorded baseline `d545ec03095b`, each overlapping no-store program (formerly labeled invalid) wrongly refuses its `agree` contract. The same synthetic-store broadening also refuses the existing direct-choice and nested-machine disjoint-bound samples. Historical refusal labels were not language authority: a known complete empty body frame preserves these facts.

Acceptance is `omega run --both <place>/main.omg` exiting 0 in both engines for all nine places, and each `omega --check <place>/invalid/main.omg` rejecting the actual overlapping write with `cannot prove requires contract`. Producer-bound observations live in the owning qualification/claim record; no unexecuted target is claimed here.

## Places

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [`local/main.omg`](local/main.omg) | [`invalid_local/main.omg`](invalid_local/main.omg) |
| field | [`field/main.omg`](field/main.omg) | [`invalid_field/main.omg`](invalid_field/main.omg) |
| parameter | [`parameter/main.omg`](parameter/main.omg) | [`invalid_parameter/main.omg`](invalid_parameter/main.omg) |
| call result | [`call_result/main.omg`](call_result/main.omg) | [`invalid_call_result/main.omg`](invalid_call_result/main.omg) |
| match join | [`match_join/main.omg`](match_join/main.omg) | [`invalid_match_join/main.omg`](invalid_match_join/main.omg) |
| const name | [`const_name/main.omg`](const_name/main.omg) | [`invalid_const_name/main.omg`](invalid_const_name/main.omg) |
| a nested machine | [`other_state/main.omg`](other_state/main.omg) | [`invalid_other_state/main.omg`](invalid_other_state/main.omg) |
| another machine | [`other_machine/main.omg`](other_machine/main.omg) | [`invalid_other_machine/main.omg`](invalid_other_machine/main.omg) |
| another target body | [`target_body/main.omg`](target_body/main.omg) | [`invalid_target_body/main.omg`](invalid_target_body/main.omg) |

## Requirements and evidence

The canonical row uses array elements. Earlier sibling probes, including the
float variants, were retired in [#1841](https://github.com/CathedralOS/Omega/pull/1841).
Their producer-bound observations remain historical evidence in the source
history; they do not qualify the current Build configuration repair.

- **Establishment.** The observation is the read `items[0]` of the literal `[7, 1]`. Where that read is in another machine or target body, the callee requires `items[0] == 7` and the caller proves it from the literal. The consuming `agree` requires the saved value still equal the live element and 7. The exclusive loan does not establish the fact; the read does.
- **Identity and invalidation.** The complete non-writing call preserves the exact observed element. The twin's actual store overlaps that element and invalidates equality with its saved value. Disjoint actual writes remain covered by the choice rows.
- **Composition.** The nine places carry that same sentence. Parameter, call result, other machine, and target body pass the saved value or the array through a contract that names the element. Field, match join, const name, and a nested machine keep the read in the lending state. The positive and reject twins differ only by the store in `expose`.
- **Soundness.** The loan is ordinary exclusive access. No mathematical binding is executed. Proof retirement follows actual write footprints; no frame grants borrow authority.
- **Checked-to-Terminal Psi.** The positive places execute in the Terminal Psi interpreter and agree with native execution. The reject twins fail checking, so they have no Terminal Psi product.
- **Later realization.** Each positive binds `ProgramEntry` for `windows_x86_64`, `linux_x86_64`, `macos_arm64`, and `linux_arm64`. Current native/Terminal execution evidence is host-specific; bindings alone do not establish execution on the other hosts.

These array-element observations and overlapping-write twins do not close the rest of `STATE-LOCAL-VALUE-FRONTIER`.

## Build-owned target-body migration

The `target_body` place and its twin carry no source target prefix.
`BUILD-OWNED-TARGET-SELECTION` replaces the prefixed family with ordinary named
implementations (`X86...` and `Arm...`) of a boundary-trait requirement, selected
by explicit target-keyed Build rows. The operation, inputs, contracts and exit
oracle are unchanged. The entry wrapper and each implementation check a tag
(17 for x86-64, 29 for AArch64), so a wrong Build row traps. Observed evidence is
Linux x86-64 native and Terminal Psi agreement for the positive and source
rejection for the twin; the other hosted targets keep their authored rows,
unexecuted here.
