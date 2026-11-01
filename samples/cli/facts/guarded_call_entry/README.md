# Guarded invocation entry after opaque callbacks

After an opaque callback, fresh `count <= slots.len` and preserved immutable
`fuel > 0` admit the nested call's `slots.len - count` and `fuel - 1`.
Every place copies this fact and consumes both obligations directly.

Owner: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in [TASKS.md](../../../../TASKS.md),
remaining guarded `CompactBinary::frame_fields` / private `parse` admission in
[Std wire](../../../../source/library/std/wire.omg). This is the invocation-entry
boundary: future callee writes cannot erase already evaluated entry subjects.
It differs from the [copied argument snapshot](../copied_call_argument_snapshot/README.md)
row, which retains an earlier scalar across effects of a later argument.

`Plan` may mutate the context. The count premise is established by a fresh guard
**after** that callback; the earlier positive fuel belongs to an immutable scalar
copy. The nested consumer has an opaque write summary. Its current entry facts
remain usable without granting reference preservation on its return.

| Place | Positive | Reject twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [weakened fuel](local/invalid/main.omg) |
| field | [source](field/main.omg) | [weakened fuel](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [weakened fuel](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [weakened fuel](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [weakened fuel](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [weakened fuel](const_name/invalid/main.omg) |
| nested machine | [source](other_state/main.omg) | [weakened fuel](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [weakened fuel](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [weakened fuel](target_body/invalid/main.omg) |

Each twin changes only the producing invocation's fuel domain from `> 0` to
`>= 0`. The guards, callback contract and consuming arithmetic remain unchanged.
It must reject the missing positive-fuel requirement at the nested `parse`
consumer after the callback. Suppliers accept any `u64` and preserve its exact
identity, so the twin cannot fail at an earlier positive-fuel precondition. Additional controls reject a stale pre-callback count and
fuel exposed mutably to a callback which zeros it.

Each positive executes both a successful callback and a callback which clears
`valid`; their observations differ from the fallback sentinel 99. Omitting the
successful callback leaves a remainder of 2 instead of 0. The match place runs
both arms: its selected fuel is 1 or 2, so the observed fuel remainder is 0 or 1.
Every caller also verifies all three arrays' contents and neighboring hex canaries.
The field carrier has its own neighboring canaries. Other places supply fuel 2,
so their two returned fields are distinct: remaining 0 and fuel-left 1.
The consumer returns both differences in an ordinary record; neither arithmetic
fact is replaced by a caller equality guard.

## Requirements and evidence

- **Establishment:** the producing invocation requires positive fuel; exact
  copies and contracted suppliers carry it. The post-callback guard supplies
  the current count/length relation. The guard does not assume the consumer's
  requirements or recover a constructor's pre-callback count.
- **Identity and lifetime:** immutable copied fuel keeps its exact value;
  the current referent uses authenticated storage origins after argument
  evaluation. Unknown descriptor histories remain unknown. The stale-context
  and mutable-fuel controls retain their invalidation fences.
- **Composition:** local, field, parameter, result, join, constant, nested
  machine, other machine and selected provider transports feed the same pair
  of obligations. Each invocation checks its own declared contract.
- **Soundness boundaries:** an opaque frame grants no post-state preservation,
  access authority, termination promise or assumption about callback contents.
- **Psi 05–07:** ordinary checking and Terminal lowering retain the two input
  facts and return both arithmetic results. Actual `omega run --both` observes
  the Terminal interpreter alongside native execution.
- **Omega 00–02:** the four hosted Build entries and the selected provider use
  normal record/view layouts and scalar, reference and aggregate-return plans.
- **Omega 03–06:** caller-visible result fields, live values across calls and
  neighboring canaries expose incorrect homes, spills and field selection.
- **Omega 07–09 and emission:** direct macOS ARM64 images must agree with
  Terminal execution and exit 0. Other-host native behavior is unverified.

The frozen original 36-line reduction rejects its private `parse` contract on
optimized producer `6c8950d4825` (SHA-256
`717c3b9f6443bd68d2a2243bdef18fc2ea8f9f716c29af7ad96688ebf24e94c3`).
The repaired producer has SHA-256
`b7bc6f1e94027fd92558acdb9a4f8539520f97432616508106b213e81ea5d95d`
(opt-level 1), built from `ddda30fe17a` plus the compiler changes checkpointed as
`fec9a6f33b2`. It executes the unchanged reduction and incoming-reference
composition with native/Terminal exits 0/0. All nine final positives execute with native/Terminal exits 0/0 on macOS ARM64,
and all nine twins reject `parse`'s positive-fuel obligation. Eight positives
refuse that contract on the retained baseline; the parameter place already
passes source checking and supplies composition coverage. The row is pinned as
`guarded_call_entry_through_<place>_{exit,rejected}` under
`tests/omega/{pass,fail}/facts/`, alongside its two safety controls. Scoped row
records and independent review are retained with the PR and owning claim; authoring syntax failures are
not baseline defects or acceptance.

WindowDemo clears both source packages and Terminal production on this producer,
then reaches ordinary package-policy review. Its accepted project policy is
unchanged. Native GUI execution, mixed-predicate restoration-window soundness,
remaining range-suffix migrations and the parent's other acceptance stay open.
