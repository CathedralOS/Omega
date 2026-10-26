# Layout extent contracts

Parent: **CHECKED-PACKAGE-REUSE** in [TASKS.md](../../../../TASKS.md).
The requirement is to retain the checked array extent, captured scalar value and
selected body through invocation proof, private target closure and native publication.

A fixed-array length retains its layout-derived extent through value transport
and satisfies the invoked length-eight contract.

Every standard place copies that sentence and calls the length-eight contract
before its runtime observation. The policy gives `View` eight bytes; the array's
symbolic extent is `extent() = size_of<View>()`. Every invalid twin changes only
the policy extent from eight to four. No caller guard establishes the fact.
The frozen `39d10d6` baseline refused the valid nonindexed call because it lost
this fact; later target-family transport exposed unclosed sibling-body recipes.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [local/main.omg](local/main.omg) | [local/invalid/main.omg](local/invalid/main.omg) |
| field | [field/main.omg](field/main.omg) | [field/invalid/main.omg](field/invalid/main.omg) |
| parameter | [parameter/main.omg](parameter/main.omg) | [parameter/invalid/main.omg](parameter/invalid/main.omg) |
| call result | [call_result/main.omg](call_result/main.omg) | [call_result/invalid/main.omg](call_result/invalid/main.omg) |
| match join | [match_join/main.omg](match_join/main.omg) | [match_join/invalid/main.omg](match_join/invalid/main.omg) |
| const name | [const_name/main.omg](const_name/main.omg) | [const_name/invalid/main.omg](const_name/invalid/main.omg) |
| nested machine | [other_state/main.omg](other_state/main.omg) | [other_state/invalid/main.omg](other_state/invalid/main.omg) |
| other machine | [other_machine/main.omg](other_machine/main.omg) | [other_machine/invalid/main.omg](other_machine/invalid/main.omg) |
| target body | [target_body/main.omg](target_body/main.omg) | [target_body/invalid/main.omg](target_body/invalid/main.omg) |

Each positive binds all four hosted `ProgramEntry` targets and declares
`Expected exit: 0`. Acceptance is `omega run --both <place>/main.omg`, with
native exit 0 and Terminal interpreter exit 0. Each standard twin must reject the
`requires contract for call` obligation `observed == 8`. The corresponding
`tests/omega/{pass,fail}/facts/layout_extent_contract_through_*` fixtures pin
the same authored programs and Build bindings.

[Composition](composition/main.omg) and its [twin](composition/invalid/main.omg)
put a match and arithmetic inside target-selected results.
[Snapshot](field/snapshot/main.omg) retains a copied record's length after the
original changes; the stale live-field fixture must reject.
[Selector](local/selector/main.omg) requires its index computation to execute
exactly once. The empty-outer fixture accepts extent four beneath a zero-length
outer array: declaration geometry does not manufacture element-storage custody.
These are focused controls, not additional transport rows.

## Requirements and evidence

- **Establishment:** validated layout and the exact array application establish
  the extent; `consume` requires eight before any runtime equality branch.
  The four-byte twins reach that same obligation and reject.
- **Identity and lifetime:** immutable captured values retain their producer and
  storage version. Snapshot and stale-field controls distinguish an owned copy
  from changed live storage; references do not become scalar snapshots.
- **Composition:** arguments, normal results, joins, fields, constants and nested
  machines carry the same fact. Target choices retain exact selected bodies;
  ordinary match joins retain every incoming requirement.
- **Soundness:** every authored target body is checked. Private closure retains
  complete qualified observation assignments and exact actuals. Choosing Mac
  does not excuse an invalid Linux body. Erased helpers grant no new authority.
- **Psi 05–07:** checked sequence measurement preserves symbolic extents and
  selector evaluation order. Terminal publication reads recorded type/application/
  consumer associations and retains the receiving conditions after helper erasure.
- **Omega 00–02:** primary geometry and private body geometry use their admitted
  contexts. Supplemental proof values remain qualified; actual storage alone
  receives Storage conditions. Existing layout and calling plans realize the
  array homes, explicit parameters and results.
- **Omega 03–06:** native lowering executes the same length and scalar calls,
  with ordinary register and frame allocation. This row makes no new claim
  about argument-count, instruction-width or activation-bound limits.
- **Omega 07–09 and emission:** authored exit zero agrees in native and Terminal
  execution on the tested host. Build bindings and checking cover all four hosted
  targets; executions on other hosts remain unverified.

Producer-bound observations and independent replay are retained in PR #221's
qualification records. This row does not close persisted checked/native reuse,
parent timing targets, query-only geometry coverage or whole-series comparison.
