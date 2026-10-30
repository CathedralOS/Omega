# Joint private runtime ranks

Owner: `NESTED-MACHINE-UNIFICATION` in [TASKS.md](../../../../../TASKS.md).

Fact: a private helper preserves the invocation rank on a tail edge, and every
complete cycle strictly descends while advancing the explicit counter once.
Each place transports the same natural input into the same `count` operation.
The caller retains the mutable counter through the cycle and reads it again
after completion. Helpers author their actual entry requirements, without
redundant `terminates by` clauses or captured runtime inputs.

The matching input performs seven increments from 3 to 10. The fallback input
performs zero increments and preserves 10. Both run in every `main`: inverting
a dispatch, selecting the fallback, omitting an increment, or repeating one
fails the exit-0 oracle. Every twin changes only `pending - 1` to `pending` and
must reject because the complete cycle then has no strict edge.

| Place | Positive | Nondecreasing twin |
| --- | --- | --- |
| local | [source](local/main.omg) | [source](local/invalid/main.omg) |
| field | [source](field/main.omg) | [source](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [source](parameter/invalid/main.omg) |
| call_result | [source](call_result/main.omg) | [source](call_result/invalid/main.omg) |
| match_join | [source](match_join/main.omg) | [source](match_join/invalid/main.omg) |
| const_name | [source](const_name/main.omg) | [source](const_name/invalid/main.omg) |
| other_state | [source](other_state/main.omg) | [source](other_state/invalid/main.omg) |
| other_machine | [source](other_machine/main.omg) | [source](other_machine/invalid/main.omg) |
| target_body | [source](target_body/main.omg) | [source](target_body/invalid/main.omg) |

[Earlier decrease](arrival_decrease/main.omg) moves the strict edge before two
preserving helper calls, with renamed and reordered parameters; its
[twin](arrival_decrease/invalid/main.omg) removes that edge. The
[top-level peer](top_level/main.omg) checks the same rank correspondence without
lexical containment. A [private promise](private_promise/main.omg) declares
`terminates` on its helper without a redundant helper rank; its
[twin](private_promise/invalid/main.omg) still rejects a preserving cycle. [Long cycle](long_cycle/main.omg) performs 100,000 increments
while retaining the caller's original borrowed counter.

Additional source controls reject a [preserving subcycle](controls/preserving_subcycle/main.omg)
even when another cycle decreases, a [non-tail recurrent call](controls/non_tail/main.omg),
a [false arrival requirement](controls/missing_arrival_contract/main.omg), and
[overlapping mutation through a live shared loan](controls/live_shared_loan/main.omg).
The last two are independent contract and ownership obligations: ranking does
not authorize them.

## Evidence and requirements

The fixed fetched base is `56b550d41aec59c4ac3e59ca3d570f1a2afb9dc5`.
The pinned `nightly-2026-09-04` producer builds that unchanged source and
rejects the unchanged nine positives with `a member lacks a supported exact
ranking witness` for `count, forward`. Earlier Homebrew observations remain
labelled separately; the submitted representation lookup remains unchanged.
Raw baseline, candidate, qualification and resource observations are retained in
this workbench's `build/joint-rank/` and identified in the publication record.

- Establishment: the authored natural rank anchors the component. Each private
  coordinate is an exact declaration parameter; every recurrent call is checked
  against its actual arguments and its own current prefix. The source rank is
  its entry snapshot, not a later mutable value or an enclosing entry fact.
- Identity and lifetime: simultaneous argument identity and the explicit mutable
  counter survive preserving and decreasing edges; the caller observes the
  original backing after completion. The live-shared-loan control still rejects.
- Composition: helpers can rename/reorder the rank; every incoming and outgoing
  recurrent occurrence participates. The graph containing only non-strict edges
  must be acyclic, so a convenient decreasing cycle cannot hide a preserving one.
- Soundness: formation, callee requirements, effects, cleanup, loans and exact
  tail-position checks remain separate. Unpromised productive components receive
  no fabricated natural rank. Other views and authored ranges retain their
  existing full-coordinate and endpoint obligations.
- Psi 05–07: admitted coordinates become the existing entry observation and
  constant-stack component rows, including witness-free helpers. Native and
  canonical Terminal execution agree; the pinned corpus independently audits
  emitted operations and custody.
- Omega 00–02: each positive binds all four hosted `ProgramEntry` targets. The
  ordinary caller and explicit borrowed argument use existing calling plans;
  only macOS ARM64 has native execution evidence here.
- Omega 03–06: the long native image exits 0 with an actual 256 KiB
  `RLIMIT_STACK`, with the caller-owned counter live for all 100,000 cycles.
  This bounds this physical execution; it is not portable WCSU certification.
- Omega 07–09: emitted macOS ARM64 images execute the observable counter oracle
  and agree with Terminal Psi. Other hosted native engines remain unmeasured.

The positives and fact-sensitive twins are pinned under
`tests/omega/{pass,fail}/facts/nested_machine_joint_rank_*`.
Remaining parent acceptance includes private coordinates in ranged/structural
views, portable composed WCSU and other-host physical limits, complete effectful
call-contract/custody composition, source migration and the Omega-written
compiler. This row does not close `NESTED-MACHINE-UNIFICATION`.

## Returned carriers and restoration

The same rank/counter fact also travels through each place's
`carried_restoration/` production. A ranked nested component returns an owned
`View` containing an exclusive reference; its ordinary caller reseats that
carrier in the exact borrowed slot. Both the seven-step and empty paths retain
the original backing, neighboring canary and carrier stamp. Each nondecreasing
twin breaks only the strict edge. The selected-implementation place uses an
ordinary named body selected by Build.

[Requirement mapping and measured observations](local/carried_restoration/README.md)
cover the complete row, normal output-loan timing, lineage controls and the
100,000-cycle physical stack observation. Corresponding fixtures are pinned as
`facts/nested_machine_joint_rank_carried_restoration_*`; composition and safety
controls are `borrows/nested_machine_carried_*`. These are macOS ARM64 execution
observations; the broader parent acceptance remains open.

## Descriptor length coordinates

The same fact also has a `structural_view/` production in each of the nine
places. An authored `Slice::Length` rank transports its descriptor through a
private helper with reordered arguments and no separate ranking clause.
[Requirements, witnesses and controls](local/structural_view/README.md) describe
the exact admitted coordinate and current-versus-entry descriptor observations.

## Optional rank ranges

Each place also carries the same fact through `ranged_value/`.
[Formation, invocation premises and changed-bound examples](local/ranged_value/README.md)
explain why optional intervals are instantiated at each invocation while the
nonnegative rank relation stays fixed. The accompanying controls retain strict
descent, arithmetic formation and ordinary call obligations.

[Projected scalar coordinates](local/projected_value/README.md) carry the same
private-cycle fact through inline record fields in all nine places, with
nested-field, moved-copy, entry-reset and wrong-subject controls.

[Descriptor field coordinates](local/descriptor_field/README.md) carry that same
private-cycle fact through slice descriptors stored in inline records across all
nine places. Nested records, moved copies and mutable payload stores exercise
composition; restoring an earlier descriptor or shortening only another field
still rejects. Call requirements observe prepared operands before callee writes.

## Borrowed record coordinates

The `borrowed_value/` child in every place carries the same count through a
readable, binding-stable formal reference to the caller's original record.
Each caller checks count zero, canary641 and counter3→10, then exercises the
empty path. Each child's `invalid/` removes only the strict decrement.
The [local compositions](local/borrowed_value/README.md) additionally cover a
moved reference carrier, a disjoint write, nested inline fields and a borrowed
slice descriptor updated through the original record. Recursive normal-exit
summaries compose windows without identifying opaque windows as exact values.
Reset, whole-record replacement, alias-write and pointer-reseat controls under
`controls/borrowed_*/` must still fail the same rank obligation. Saved entry
coordinates grant no permission to reuse invalidated current observations.

## Paired cursor and bound coordinates

Each place also carries the same private-cycle fact through `bounded_distance/`.
The [requirement mapping and controls](parameter/bounded_distance/README.md)
cover exact cursor/slice-bound transport, fixed-width natural normalization,
independent entry formation, conserved upper coordinates and clamped-view
counterfactuals. The original counter operation and empty path remain observable.

## Stored-reference coordinates

The `borrowed_value/stored_reference/` child in every place carries the same
seven/empty payload through a `Remaining` record whose `child` is `&mut Payload`.
The selected rank is `remaining.child.count`. The caller checks count zero and
guard 827 on its original payload after the temporary reference carrier dies;
the explicit counter still advances from 3 to 10 and then stays 10.
[Requirements and controls](local/borrowed_value/stored_reference/README.md)
distinguish matched entry loads from current storage observations. The local
compositions cover carrier, child and scalar aliases plus an observable disjoint
referent write. Their twins remove only descent. Separate controls reject a
reset after descent, an alias reset, decrementing another child, write-only
observation and reliance on an enclosing helper's arrival facts.

Scalar stores publish their completed value to proven identical current storage
coordinates, retaining independent owned snapshots and refreshing invalidation
dependencies. Reference production consumes checked occurrence captures;
borrowed descriptor storage cannot become an owned field extraction. Native
geometry uses the shared structural path grammar for every reference crossing,
while access, lineage and original backing remain independent obligations.
The frozen local source refuses on `54fa5d06d77` before this repair. Further
reference loads without independently retained evidence, indexed/case private
coordinates and the parent's other accepted requirements remain unfinished.

[Effectful decrement operands](local/borrowed_value/stored_reference/operand_effects/README.md)
carry that same fact across all nine places while temporarily replacing and
restoring stored references. The operand result, caller-visible audit and
unchanged spare distinguish completed captures from reloaded descriptor homes.

## Indexed scalar coordinates

The same fact's `indexed_value/` production carries a runtime selector beside
its readable fixed-array backing through all nine places. Both selectors and
the seven/empty paths are observable, with untouched-neighbor canaries.
[Entry formation, snapshot identity, read lowering and controls](local/indexed_value/README.md)
separate admitted index bounds from source syntax and declaration geometry.

## Case payload coordinates

Each place's `case_value/` carries the same private-cycle fact through an
independently classified Pending payload. The [requirement mapping and controls](local/case_value/README.md)
cover declaration-owned case formation, public result guarantees, nested payload
fields and computed construction, with strict-descent twins in every place.

[Stored-reference case ranks](local/case_value/stored_reference/README.md)
carry the same counter fact through a loaded descriptor, with aggregate/scalar
aliases, ordered descriptor replacement and snapshot controls.

[Reference-bearing case payload ranks](local/case_value/stored_reference/reference_payload/README.md)
carry the same seven/empty counter fact through two independently captured stored
references in all nine places, with common-field loans, pointer restoration and
stale-result controls.
