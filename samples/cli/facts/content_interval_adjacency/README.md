# Adjacent interval content

Adjacent intervals preserve separated content when forwarded. This is the
numeric prerequisite exposed by Cathedral's vacant-memory accounting, not a
grant of physical storage or permission to merge unrelated roots.

The callable requires `embed(left.base) + embed(left.length) == embed(right.base)`
and returns both owned claims beneath a pair. Its postcondition conserves their
separated interval content. The captured equality must not erase the sum's
nonnegative operands before the interval ordering proof consumes them.

| Place | Positive | Reject twin |
| --- | --- | --- |
| Local | [local](local/main.omg) | [overlap allowed](local/invalid/main.omg) |
| Field | [field](field/main.omg) | [overlap allowed](field/invalid/main.omg) |
| Parameter | [parameter](parameter/main.omg) | [overlap allowed](parameter/invalid/main.omg) |
| Call result | [call result](call_result/main.omg) | [overlap allowed](call_result/invalid/main.omg) |
| Match join | [match join](match_join/main.omg) | [overlap allowed](match_join/invalid/main.omg) |
| Const name | [const name](const_name/main.omg) | [overlap allowed](const_name/invalid/main.omg) |
| Another state | [state](other_state/main.omg) | [overlap allowed](other_state/invalid/main.omg) |
| Another machine | [machine](other_machine/main.omg) | [overlap allowed](other_machine/invalid/main.omg) |
| Target body | [target body](target_body/main.omg) | [overlap allowed](target_body/invalid/main.omg) |

Each twin changes only the adjacency requirement from `==` to `>=`, allowing
overlap. It must reject the separated-content postcondition. The named state's
arrival contract repeats the machine's requirement; states do not have result
postconditions. Positive fixtures and reject twins are mirrored under
`tests/omega/{pass,fail}/facts/content_interval_adjacency/`.

The acceptance command for each positive is
`omega run samples/cli/facts/content_interval_adjacency/<place>/main.omg --both`.
Every run must report native and Terminal exit zero. Runtime checks compare all
four returned fields, exercise both match arms, empty intervals on either side,
and an endpoint at `u64::MAX`. All four hosted targets have authored entry
bindings; execution evidence is host-specific, not a claim that all ran here.

## Requirement coverage

1. **Establishment:** the callable's adjacency premise and nonnegative embedded
   widths establish interval ordering before the separation operation.
2. **Identity and lifetime:** two linear claims move into distinct returned
   fields and are each consumed once. No backing, Stable access or new authority
   follows from the numeric geometry.
3. **Composition:** the nine places carry the same content equation; constructor
   fields on a match edge retain their own types and destination paths.
4. **Soundness boundaries:** overlap-permitting twins reject. Ownership controls
   under `tests/omega/fail/ownership/selected_constructor_*` challenge duplicate
   sources, prefix overlap, reuse, a live borrow and an omitted linear move on
   one arm. The neighboring positive fixtures swap linear fields and combine
   transferred fields with fresh construction. Returns through a wrapper and
   two simultaneously live results check that caller-local identities do not
   alias callee-local births.
5. **Psi 05–07:** native and Terminal execution consume the checked ownership
   transfers and erased proof facts. Runtime comparisons catch wrong payloads;
   these samples make no claim about `dyn`, boundary calls or borrowed restoration.
6. **Omega 00–02:** the Build companion selects the entry; ordinary structural
   argument/result layout and calling plans transport both regions. There is no
   foreign provider or physical memory acquisition here.
7. **Omega 03–06:** selected constructors and nested carrier calls exercise
   instruction selection, activation storage and return transport. No standalone
   performance or bounded-stack guarantee is asserted.
8. **Omega 07–09 and emission:** the emitted host image must execute with the same
   zero result as Terminal Psi; branch placement, relocations and image emission
   are exercised only as needed by these programs.

## Remaining customer work

Cathedral's verified handoff retains its image/context and two vacant children.
Stable access, resident initialization, readback, retirement and an allocator
remain unfinished. A separately checked generic merge additionally needs the
source-level compatible-lineage contract raised in
[OWNER_QUESTIONS Q1](../../../../OWNER_QUESTIONS.md#q1-how-does-a-checked-callable-require-compatible-extent-lineage).
Numeric adjacency alone must never authorize that merge.
