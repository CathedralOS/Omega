# Standing predicates after write-only field stores

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in `TASKS.md`.
Fact: **Counter.value <= 60 makes exact i32 70 + value representable without a per-call bound.**
This is another production of [the existing fact](../README.md), established
by a partial store through `&write Counter`.

`fill` receives a valid `Counter` and a replacement bounded by its ordinary
call requirement. It stores only `value`, without reading through the loan.
The independent `peer <= 60` predicate must survive; the unchanged `before`,
`peer` and `after` fields retain 31, 13 and 47. The produced `Counter` then
travels through each place before an independent consumer proves `70 + value`.
The two inputs 60 and -2147483648 must produce 130 and -2147483578. Both match
arms execute, and target selection has independent 17/29 implementation checks.

| Place | Positive | Missing value predicate |
| --- | --- | --- |
| Local | [program](local/main.omg) | [reject](local/invalid/main.omg) |
| Field | [program](field/main.omg) | [reject](field/invalid/main.omg) |
| Parameter | [program](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| Call result | [program](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| Match join | [program](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| Const name | [program](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| Nested machine | [program](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| Other machine | [program](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| Selected implementation | [program](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

Each twin removes only `Counter`'s value predicate, retains its peer predicate,
and must refuse the exact addition. Additional controls reject an
[insufficient replacement bound](controls/invalid_store/main.omg) and a
[read through write-only authority](controls/forbidden_read/main.omg).
The [predicate helper](controls/predicate_helper/main.omg) expresses both
bounds through a checked Boolean call, preserving the same operation and oracle.
Samples and exact corpus pins bind all four hosted `ProgramEntry` targets.
Pins use `tests/omega/{pass,fail}/facts/data_where_standing_write_only_*`.

## Requirements and evidence

- **Establishment:** construction supplies the initial complete default domain;
  the formed replacement and ordinary data-store proof establish the updated
  value. The destination annotation contributes no proof of its own store.
- **Identity and lifetime:** the write-only loan names existing valid storage;
  stores preserve sibling cells and their predicates without observing them.
  Read permission, field-subloan admission and displacement custody remain
  independent. See [write-only authority](../../../../../wiki/spec/terminal-psi/structural_access.md#write-only-authority).
- **Composition:** the same established fact crosses all nine places. The
  consuming addition precedes the caller's result guard; the match and target
  checks distinguish alternative execution paths.
- **Soundness:** a missing standing fact cannot be recovered from later callers;
  an insufficient store bound and an unauthorized read still reject.
- **Psi lowering:** ordinary scalar stores, explicit calls, joins and nested
  transfers retain their effects in Terminal Psi, while proof clauses erase.
- **Layout, ABI and provider settlement:** the same four-field record and
  explicit entry/provider bindings retain exact argument and field placement.
- **Instructions and storage:** native stores update the original caller home;
  the returned/copied record retains both written values and untouched canaries.
- **Encoding and execution:** qualification observes native and Terminal exits
  on macOS ARM64; four authored bindings do not claim other-host execution.

Baseline `084cb11bd22ad69524a2913c4735a37ee282b6d0` rejected all nine positives
at the write-only projection gate; all nine twins rejected the missing arithmetic
fact. The repair separates a store's address from formation of a projected loan:
stores retain the original owner's obligations, whereas a bare field loan must
not discard its containing predicates. Optimized CLI qualification observes
all nine positive native/Terminal exits as 0 and the intended twin refusals.
Final focused corpus results and actual producer identities belong to the PR.

This does not close the parent migration. In particular, the old interval store
checker still refuses a temporary 61 followed by a restoring bounded store;
the same refusal occurs with `&mut` on the baseline. General interval-window
restoration, remaining aggregate/reference transports, historical source audit,
filesystem qualification and suffix retirement retain their acceptance.
