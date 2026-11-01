# Captured storage bounds

Counter's declared 0..16 bounds survive a disjoint field write and captured scalar read, so an incoming pumped < 16 guard admits pumped + 1.

Parent: **CANONICALIZE-SCALAR-RANGE-CONTRACTS** in
[TASKS.md](../../../../TASKS.md). This is the captured-read prerequisite of
WindowDemo's unchanged 16-message guarded increment. It does not close the
parent's remaining record/array handback or full GUI acceptance.

`Counter.pumped` is an `i32` whose data declaration enforces `0 <= pumped <= 16`.
The loop writes disjoint `have` and `marker` fields, observes `pumped`, and carries
only `pumped < 16` into the private increment helper. The completed scalar must
retain the standing floor independently of that incoming upper guard. No
receiving goal establishes itself, and no extra nonnegative guard supplies the
missing declaration fact.

## Places

| Place | Transport into the common update | Positive | Reject |
| --- | --- | --- | --- |
| Local | Mutable local `Counter` | [source](local/main.omg) | [twin](local/invalid/main.omg) |
| Field | Borrowed `Holder.counter`, with outer canaries | [source](field/main.omg) | [twin](field/invalid/main.omg) |
| Parameter | Owned `Counter` parameter, updated and returned | [source](parameter/main.omg) | [twin](parameter/invalid/main.omg) |
| Call result | Completed constructed `Counter` result | [source](call_result/main.omg) | [twin](call_result/invalid/main.omg) |
| Match join | Two distinct `Counter` arms joined before return | [source](match_join/main.omg) | [twin](match_join/invalid/main.omg) |
| Const name | Named `[copy]` `Counter` constants copied into mutable storage | [source](const_name/main.omg) | [twin](const_name/invalid/main.omg) |
| Nested machine | `Counter` passed into the nested `nested` machine | [source](other_state/main.omg) | [twin](other_state/invalid/main.omg) |
| Another machine | Owned `Counter` relayed through two top-level machines | [source](other_machine/main.omg) | [twin](other_machine/invalid/main.omg) |
| Selected target body | `Counter` produced by a named implementation selected in Build | [source](target_body/main.omg) | [twin](target_body/invalid/main.omg) |

Each place copies the same public update body and fact sentence. Each Build binds
`ProgramEntry` for Windows x86-64, Linux x86-64, macOS arm64 and Linux arm64. The
target place selects ordinary `SelectedCounter` for those same targets.

Each reject changes **only** the direct guard from `< 16` to `<= 16`. The admitted
endpoint can then produce 17, which fits `i32` but violates the unchanged data
predicate. Its intended refusal is receiving `where` containment, not overflow.
Positive and reject runtime oracles remain identical.

## Requirements and runtime oracle

- Establishment: constructions and named constants satisfy the declaration;
  parameter/return carriers transport an already admitted `Counter`.
- Identity and lifetime: a disjoint field write must preserve the declaration's
  bounds on the exact later scalar observation. Recurrent arrivals cannot use an
  earlier iteration's values as an invariant. Borrowed updates return authority
  before the caller reads the result.
- Composition: the captured standing floor and the private caller's upper guard
  jointly justify `pumped + 1`. All producers feed that same consumer.
- Soundness: these are store-enforced field intervals, not arbitrary pending
  predicates, assumptions from the receiving goal, or new read permissions.
- Lowering and execution: both `choose` paths execute, leaving distinct markers
  11 and 13. An untouched `origin` field also distinguishes the transported
  values, including the match arms. Each counter must finish at 16 with `have`
  equal to 1; any mismatch crashes `Trap`.
- Storage: nonzero `0x13579BDF` and `0x2468ACE0` canaries surround `pumped` and are
  checked after both calls. The field place also checks the enclosing record's
  canaries. Const originals must remain unchanged. No affine cleanup effect is
  claimed for this `[copy]` carrier.
- Omega 00–09: existing record layout, reference/call plans, exact i32 increment,
  frame homes, branches and native image emission carry the unchanged operation.
  No new ABI, layout or instruction form is claimed; the resulting writes,
  transported values and untouched neighbors are observed in both engines.

The nearest rows prove different requirements:
[completed integer carrier bounds](../completed_integer_carrier_bounds/README.md)
retains primitive `u8` bounds through widening; `i32` alone cannot establish this
row's 0..16 refinement.
[Default-domain field write](../default_domain_field_write/README.md) uses literal
preserving stores; it does not exercise captured read identity plus a private
arrival guard after another field write.

## Pins and qualification

The nine positive/reject pairs are copied under
`tests/omega/pass/facts/captured_storage_bounds_through_<place>_exit` and
`tests/omega/fail/facts/captured_storage_bounds_through_<place>_rejected`.
Positive fixtures declare exit 0 for both native and Terminal engines.

The frozen local discovery candidate wrongly refuses on optimized exact
`225551dd8184c58a9cb777ef540b6e5e8c3b7448` (producer SHA256
`0fecdc1bc3a57f5a0b823d4b309207a7524f60fca0c3a578bdcac98b6327c284`).
Preliminary R2 admits that candidate and still rejects its `<=16` twin with
inferred [1,17] versus required [0,16]. Those source/Terminal-production results
are historical discovery evidence, not execution of these newly authored files.

Final behavior is qualified on macOS arm64 with optimized R5 producer SHA256
`717c3b9f6443bd68d2a2243bdef18fc2ea8f9f716c29af7ad96688ebf24e94c3`,
source `6c8950d4825fde942fde2b421f26b6192635114a`: all nine canonical `requires`
positives observe native exit 0 / Terminal exit 0, and all nine twins refuse the
intended `pump_next` receiving-`where` obligation. Both choices, origin markers,
writeback and canaries are checked in every place. Authored sources and fixture
copies remained unchanged throughout execution; the nine consumed locks were
captured and unchanged. Other hosted bindings are declarations, not cross-host
runtime qualification.

The focused corpus records retain their original R3/`9150cfa6fe03` observation
identities: nine checked positives, nine intended rejections and nine macOS
arm64 engine agreements. Their 54 fixture files are unchanged; fresh R5 public
CLI runs exercise the identical pinned program sources and oracles. Independent
consumer review accepts this reuse without relabeling historical observations.
The historical discovery sources, producers, raw streams and replay inputs remain
retained with the PR evidence.

This closes the captured-read prerequisite only. Full GUI qualification and the
parent's mixed-predicate restoration-window acceptance remain open; the new
intake retains only additional bounds independently enforced by every store.
