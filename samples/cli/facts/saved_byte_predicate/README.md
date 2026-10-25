# Saved byte-predicate currentness

A saved byte predicate certifies the same bytes after a disjoint write, but not after an overlapping write.

Parent: **R5**, caller-visible mutation summaries and their current-content consumers. This row establishes the saved-verdict invalidation obligation in [byte views](../../../../wiki/spec/terminal-psi/byte_views.md#evaluated-content-predicates). R5 remains open for finite write cycles, parameter permutations, convergence and its other remaining acceptance.

`ascii_only(view)` executes once. A separate buffer receives byte 255, and the saved decision must still justify `inspect(view)`'s ASCII requirement. `inspect` returns the observed byte; the program traps unless it is 65. Each reject twin changes only the store target to `view[0]`, making that saved decision insufficient. The Boolean snapshot itself remains a value after the write.

The nine places vary the observed byte storage's carrier or executing body: local array, record field, borrowed parameter, array call result, joined array result, named constant initializer, nested machine, another machine and target body. Every place executes the same saved-observation/write/contract-consumption operation. All four hosted entry bindings are authored. This host executed Windows x86-64.

| Place | Positive | Overlapping twin |
|---|---|---|
| local | [source](local/main.omg) | [reject](local/invalid/main.omg) |
| field | [source](field/main.omg) | [reject](field/invalid/main.omg) |
| parameter | [source](parameter/main.omg) | [reject](parameter/invalid/main.omg) |
| call result | [source](call_result/main.omg) | [reject](call_result/invalid/main.omg) |
| match join | [source](match_join/main.omg) | [reject](match_join/invalid/main.omg) |
| const name | [source](const_name/main.omg) | [reject](const_name/invalid/main.omg) |
| a nested machine | [source](other_state/main.omg) | [reject](other_state/invalid/main.omg) |
| another machine | [source](other_machine/main.omg) | [reject](other_machine/invalid/main.omg) |
| target body | [source](target_body/main.omg) | [reject](target_body/invalid/main.omg) |

## Baseline and repair

At `3ddc7d1455f182d54645df101870ea9b441b9019`, `omega run --both` of `local/main.omg` exited 200. Lowering `probe` failed with `InvalidUnitMachinePlan` reason `reference alias changes its referent carrier`. A borrowed destination was treated as an alias whenever its access was a loan, and any carrier change was refused. A shared reborrow of a mutable byte view is a different type identity with the same pointer and extent. That reborrow stays an alias when the borrowed-referent presentation is unchanged and the source access can supply the destination ceiling. Addressing inline backing, and any other carrier change, still uses the shared descriptor producer.

On Windows x86-64, all nine positives then return native exit 0 and Terminal exit 0. All nine twins reject with `cannot prove requires contract for call inspect`. The reachable lane-38 control remains `tests/omega/fail/slices/lane38_saved_predicate_reachable`.

## Requirements and evidence

- **Establishment:** Evaluating the primitive and selecting its true result establishes the relation to that exact view's current bytes. No caller equality guard repairs a missing relation.
- **Identity and lifetime:** Complete intervening write frames retire overlapping content relations. Disjoint buffers and fields survive. The saved Boolean is not overwritten by content invalidation. Coarse collection footprints remain conservative.
- **Composition:** The same consuming contract and write target distinction occur in all nine places. Sources are pinned byte-identically under `tests/omega/{pass,fail}/facts/saved_byte_predicate_through_*`.
- **Soundness:** The selected scalar producer, source custody, access, bounds and loan checks remain required. Backing footprints grant no exact identity, access or new callable purity. Unknown origins remain opaque.
- **Psi 05–07:** Actual `omega run --both` carries the checked Boolean, view and selected transition through Terminal emission and interpretation. The runtime result tests the observed byte.
- **Omega 00–02:** Each hosted Build companion supplies ProgramEntry; the ordinary view/scalar calling plans and provider settlement carry `inspect` and its result.
- **Omega 03–06:** Native execution exercises the selected predicate operations, live saved Boolean, mutation, calls, register homes and activation frames.
- **Omega 07–09 and emission:** The Windows x86-64 image executes with the authored result and agrees with Terminal. The other three hosted targets are authored and were not executed on this host.

This completes the bounded saved-byte-predicate currentness repair, not all R5 or exact disjointness between windows within one coarsened collection.
