# Nested borrowed field production

Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.

Owning task: **CANONICALIZE-SCALAR-RANGE-CONTRACTS**, recursive/reference
predicate read inventories. This is another production of the existing
[data_where_standing](../../README.md) fact. `Holder.counter` stores a shared
reference to `Counter`, whose `value` stores a separate shared reference to
`i32`. The bound and exact operation match the [borrowed field production](../README.md).
The consuming body passes that loaded scalar descriptor through ordinary `read`.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [program](local/main.omg) | [missing predicate](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [missing predicate](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [missing predicate](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [missing predicate](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [missing predicate](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [missing predicate](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [missing predicate](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [missing predicate](other_machine/invalid/main.omg) |
| Selected target implementation | [program](target_body/main.omg) | [missing predicate](target_body/invalid/main.omg) |

Each twin removes only `Counter`'s default predicate. Public consuming bodies
owe the addition independently of `main`'s literals. Entries execute 60 and -17,
expect 130 and 53, and check the unchanged backing scalars, both records' nonzero
neighbor fields and markers. The match entry executes both selections. The
selected-target entry retains distinct provider tags. Every positive declares
exit 0 and binds all four hosted targets.

## Requirements and evidence

- **Establishment:** constructing `Counter` proves `bounded(value) == true`.
  Its standing predicate justifies the exact addition through the helper's
  checked result equation; no caller guard supplies a missing premise.
- **Identity and lifetime:** the Holder slot, Counter descriptor, scalar-pointer
  slot and scalar referent remain distinct. Reading a pointer field retains
  the actual completed descriptor and its lifetime. The receiver permits the
  slot read; its address is not the child's pointee or reborrow parent.
- **Composition:** every place transports the same record/reference/predicate.
  A match preserves every incoming origin and the selected receiver identity.
  Descriptor snapshots and current referenced contents have separate epochs.
- **Soundness:** complete source inventories, readable intermediate references,
  actual pointer-slot access, exact carrier/access and surviving writes remain
  mandatory. Distinct loaded-pointer namespaces prove no spatial separation.
- **Checked-to-Terminal Psi:** the bound erases; pointer loads, scalar reads,
  helper calls, selection and canary observations remain executable.
- **Layout, ABI, instructions and encoding:** existing reference/record layout,
  calling plans and image emission must preserve both pointer crossings and
  agree with Terminal Psi. Authored target bindings establish no foreign-host
  runtime observation.

The fixed discovery base is `539d46c0eb8aa68b72e9f3b326c27d8ed2959346`.
All nine frozen positives refused the exact i32 addition. Current repairs have
carried all nine through checking; eight places have actual macOS ARM64 native
and Terminal exit 0/0 observations. The call-result place and the saved-descriptor
control still need native transport repair. Exact source copies and oracles are
pinned under `tests/omega/{pass,fail}/facts/data_where_standing_nested_borrowed_field_*`;
complete corpus and row qualification remain pending.

This production does not close the parent task's remaining recursive/case/
collection inventories, proposition handback, filesystem qualification or
source migration. GUI qualification remains pending separately.
