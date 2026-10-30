# Private arithmetic through effectful invocations

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../../TASKS.md).
This is another production of the [private arithmetic fact](../README.md),
with the same bound, operation and sentence:

> Every private arrival carries a positive value, so the helper can form value - 1.

The helper explicitly receives `&mut Storage` and writes `touched = 5` before
subtracting. The caller checks that write, its untouched `canary = 33`, both
returned values and branch tags, and the zero-input fallback. Each invalid twin
changes only the helper's scalar actual to zero.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [main.omg](local/main.omg) | [invalid/main.omg](local/invalid/main.omg) |
| field | [main.omg](field/main.omg) | [invalid/main.omg](field/invalid/main.omg) |
| parameter | [main.omg](parameter/main.omg) | [invalid/main.omg](parameter/invalid/main.omg) |
| call_result | [main.omg](call_result/main.omg) | [invalid/main.omg](call_result/invalid/main.omg) |
| match_join | [main.omg](match_join/main.omg) | [invalid/main.omg](match_join/invalid/main.omg) |
| const_name | [main.omg](const_name/main.omg) | [invalid/main.omg](const_name/invalid/main.omg) |
| other_state | [main.omg](other_state/main.omg) | [invalid/main.omg](other_state/invalid/main.omg) |
| other_machine | [main.omg](other_machine/main.omg) | [invalid/main.omg](other_machine/invalid/main.omg) |
| target_body | [main.omg](target_body/main.omg) | [invalid/main.omg](target_body/invalid/main.omg) |

The published baseline `baf1b6ea0633681bcc2cf34d563d748a47fadb36` refuses
all nine valid positives and all nine controls at arithmetic formation. Repaired
producer `d871d8aaf7a` runs the nine positives with native/Terminal exit-zero
agreement on macOS ARM64; all nine twins still reject for subtraction formation.
An initial generated const-name setup omitted the new explicit output actual;
that setup and its original failed run remain in evidence, separately from the
corrected baseline and repaired observations. No engine result was relabeled.

Identical fixtures are pinned under `tests/omega/{pass,fail}/facts/` with prefix
`nested_machine_private_arithmetic_mutable_`. Additional overlapping-write and
local-alias reject controls preserve the arithmetic obligation after replacing
the referenced count with zero. All positive places bind the four hosted targets;
this record claims native execution only on macOS ARM64.

The evidence establishes these specification requirements:

- Establishment: every actual arrival proves the scalar positive; the callee
  assumes neither an enclosing contract nor its own result. Zero rejects.
- Identity and lifetime: the completed scalar actual survives a write to separate
  storage. Explicit borrows supply access authority; the scalar fact supplies none.
  The caller observes the write and the neighboring storage canary.
- Composition: the same actual and operation survive all nine places, including
  three-level ordinary invocation and a separate relay. Both tag paths execute.
- Soundness: only a runtime visitor tracking storage preservation can observe an
  effectful checked prefix. Theorem/case-only traversal retains denotational
  admission, and the shared closed-arrival/escape/cycle restrictions remain.
- Checked-to-Terminal Psi: both-engine execution preserves the explicit borrow,
  ordered write, selected helper and returned record. No proof-only or affine
  cleanup construct is introduced by this row.
- Provider/layout/calling plans: the authored hosted entries, storage/result
  records and explicit reference/scalar arguments use ordinary plans. This row
  does not claim a new ABI placement lane.
- Instructions and frames: native results observe disjoint caller-owned storage
  and returned fields after ordinary/terminal invocations. No recursion or new
  stack-bound claim is made.
- Encoding and emission: direct macOS ARM64 execution agrees with Terminal Psi
  for all nine places, checking storage, tag paths and returned results.

This production establishes the independent scalar snapshot. The
[borrowed-field production](../borrowed/README.md) exercises live referent
correspondence and disjoint writes. Product compiler source migration, remaining
invocation obligations and other-host qualification remain on the parent task.
