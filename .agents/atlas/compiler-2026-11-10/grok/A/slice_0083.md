# slice_0083

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0634 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_scalar_graph.rs:2972 | REACHABLE | Edge scalar binds not in `required_values` get `Unused` transport; narrow integer constants skip carrier range checks before materialize. |
| A0635 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_structural.rs:1044 | REACHABLE | `StoreStructuralField` to `OwnedResult` admits without a home check; `ReleaseReference` / `EstablishTrivialAffineLocal` return `Ok(true)` and emit nothing. |
| A0636 | omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/structural_inputs.rs:2385 | REACHABLE | Borrowed-view / element-view roots skip storage bounds; zero-extent runtime reach treats any span as in-bounds. |
| A0637 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/address_and_load_rewrites.rs:880 | REACHABLE | `WriteOutgoing` stores do not count as interfering; instructions with no memory row pass when classified `Private`. |
| A0638 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/copy_and_extension_rewrites.rs:1475 | REACHABLE | Redundant `ZeroExtendU32` after `Float32ToBits` is identity without proving upper 32 bits are zero. |
| A0639 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/branch_and_compare_rewrites.rs:157 | CLOSED | Extra non-flag implicit uses fail `admits_unit_flow` because `MaterializeI64` constraints have empty `implicit_uses`. |
| A0640 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/branch_and_compare_rewrites.rs:1617 | REACHABLE | `audit_dead_unit` never checks register live-out past function exit when the terminator omits the unit. |
| A0641 | omega-rust/omega/compilation/pipeline/04_selected-instructions-to-selected-instructions/src/branch_and_compare_rewrites.rs:1508 | CLOSED | `reaching_events` errors on an empty cone contribution before compare removal can proceed. |

## A0634

Digest (read): area_04.md:219 — values outside `required_values` get no transport; integer constants materialize without carrier range check.

**Unused transport.** Opened enclosing `successor` at `selection_scalar_graph.rs:2897-2996` (read). At `:2972-2990`, bindings use `SelectedValueTransport::Registers` only when `builder.required_values.contains(&semantic.parameter)`; otherwise `:2988-2989` assigns `SelectedValueTransport::Unused`. Case-payload path at `:6777-6788` mirrors this (`SelectedCasePayloadTransport::Unused`).

Caller search: `rg '\bpub\(crate\) fn successor\(' omega-rust/omega/compilation/pipeline/03_target-operations-to-selected-instructions` → 2 definitions (read): `control::successor` at `:2897`, structural-case `successor` at `:6768` (calls `super::control::successor` at `:6795`). Production calls at `:2737`, `:2831-2832`, and case-dispatch sites `:6546`, `:6634-6636` (read).

`required_values` at `selection.rs:1624-1708` (read) is backward reachability from terminators and calls; values never linked into `incoming` never enter the set. `entry` at `selection_scalar_graph.rs:6230-6233` (read) skips stack setup entirely when `!builder.required_values.contains(&parameter.value)`.

Input in plain words: a declared incoming parameter or edge-carried scalar that the analysis omits from `required_values` still completes selection with `Unused` transport and may never receive registers or stack homes.

**Integer constants.** Opened `integer_arithmetic::emit` `Constant` arm at `selection_scalar_graph.rs:3627-3655` (read). Boolean range is checked at `:3628-3631` and `:3644-3647`; IEEE float bit patterns at `:3633-3642`. Other scalar types go straight to `emit_materialize_i64` at `:3649-3654` with no width or signed-range test against `scalar_type`.

Caller search: `rg 'integer_arithmetic::emit' omega-rust` → 1 production dispatch site in `selection_scalar_graph.rs` (read).

Input in plain words: a narrow integer scalar (for example an 8-bit carrier) given a constant outside that carrier’s representable range is still materialized as a full i64 row.

Verdict **REACHABLE** for both digest sub-claims on the selection accept path.

## A0635

Digest (read): area_04.md:220 — `StoreStructuralField` with non-home `OwnedResult` passes admission; `ReleaseReference` / `EstablishTrivialAffineLocal` emit nothing.

Opened `reseat` at `selection_structural.rs:1009-1059` (read). `destination_admits` at `:1027-1048` requires parameter/block-parameter roots to appear in the signature or block parameters; `StructuralWindowRoot::OwnedResult { .. } => true` at `:1044` has no home check.

Caller search: `rg '\breseat\(' omega-rust` → 2 matches (read): definition `:1009`, dispatch `selection_structural.rs:5457` (`Kind::StoreStructuralField`).

Opened structural entry skip at `:227-230` (read): when `owned_pointer` is absent and `referent_used` is false, transport is skipped (`continue`).

Opened emit dispatch at `:5522-5531` (read): `ReleaseReference` and `EstablishTrivialAffineLocal` return `Ok(true)` without pushing instructions when `row.result.is_none()`.

Input in plain words: a `StoreStructuralField` that writes through an `OwnedResult` window not owned by this function is admitted, and reference-release rows can complete selection as no-ops.

Verdict **REACHABLE**.

## A0636

Digest (read): area_04.md:221 — offset past borrowed view descriptor shape; empty reach returns true; element-view root skips size bound.

Opened `runtime_reach` / `runtime_span_within_storage` at `structural_inputs.rs:110-137` (read). `extent == Some(0)` yields `RuntimeReach::Empty` at `:111-112`; `:131` returns `true` for `Empty` without comparing `offset`/`bytes` to storage.

Opened `scalar_access_within_storage` at `:2385-2410` (read). `descriptor_frame` is true for `ElementView` or borrowed-view byte sequence at `:2392-2399`; `:2401-2402` short-circuits to `true` without calling `runtime_span_within_storage`.

Opened `field_read` at `:2412-2440` (read). For element-view roots, `root` is `None` at `:2431`; `:2437-2440` accepts reads when `root.is_none()` without a byte-size cap.

Caller search: `rg '\bscalar_access_within_storage\(' omega-rust` → 6 matches, all in `structural_inputs.rs` (read). `field_read` is used from `selection_inputs.rs` geometry (read at grep hits `:3509`, `:3517`).

Input in plain words: a borrowed-view descriptor frame or element-view root with a zero-extent index chain, or an access whose end offset exceeds declared storage, still yields `Some` geometry instead of rejection.

Verdict **REACHABLE**.

## A0637

Digest (read): area_04.md:224 — outgoing-argument stores overlapping forwarded places; no memory-access row accepted when `Private`.

Opened `interferes` at `address_and_load_rewrites.rs:842-882` (read). `SelectedMemoryAccessRole::WriteOutgoing { .. }` and `AddressOutgoing` fall through to `_ => false` at `:880-881`, so outgoing writes never set `interfered`.

Opened load-forwarding walk at `:520-545` (read). `candidate_accesses` at `:1175-1191` sets `interfered` only via `interferes`; when `interfered` is false the walk can select a forward through stores that only touch outgoing slots.

Opened `reject_unaccounted` at `:1218-1227` (read) with `unrostered_reach` at `rewrites.rs:2742-2807` (read). Instructions with no `memory_accesses` row call `reject_unaccounted` at `:543-544`; `UnrosteredReach::Private` returns `Ok(())`, allowing the candidate. Default for unlisted kinds is `Unaccounted` at `:2807`; many arithmetic/load kinds map to `Private` at `:2803`.

Input in plain words: a store to an outgoing-argument slot that overlaps the memory being forwarded is ignored for aliasing, and a `Private`-classified instruction between load and use needs no memory-access roster row.

Verdict **REACHABLE** (unsound load-forwarding admit path, not a selection reject).

## A0638

Digest (read): area_04.md:225 — 32-bit zero extension removed after `Float32ToBits` / `LoadPacked` without proving high bits zero; `:1464` `as u64` truncation.

Opened `producer_normalization` at `copy_and_extension_rewrites.rs:1450-1467` (read). `Float32ToBits` maps to `Normalization::MeaningfulLow { width: 32 }` at `:1451-1453`. `MaterializeI64` patterns use `value as u64` at `:1462-1465`.

Opened `identity` at `:1472-1490` (read). `(Extension::Zero { width: 32 }, Normalization::MeaningfulLow { width: n })` is true when `n <= 32` at `:1475-1477` with no test that non-low bits are zero.

Opened `copy_removal::admit` at `:1579-1582` (read): removal proceeds when `identity(form, normalization)` holds.

Input in plain words: a virtual register defined by `Float32ToBits` followed by `ZeroExtendU32` can drop the extension even if the i64 carrier’s bits above 32 are not known zero.

Verdict **REACHABLE**.

## A0639

Digest (read): area_04.md:226 — extra non-flag implicit use on a materialization reaches `resolved_unit_surface` unfiltered (boolean fold path at `:157-160` vs branch path at `:799-805`).

Opened `constant_boolean::admit` at `branch_and_compare_rewrites.rs:156-160` (read): non-flag units append to `plain_uses` without the branch fold’s `jump_surface` filter (`:799-802`).

Opened constraint requirements at `:123-133` (read): rewritten `MaterializeI64` row must have **empty** `implicit_uses`, `implicit_defs`, and `clobbers`.

Opened `resolved_unit_surface` at `rewrites.rs:281-297` (read) and `admits_unit_flow` at `branch_and_compare_rewrites.rs:413-424` (read). At `:201-207`, failed surface returns `ConstantBooleanError::UnsupportedUse`.

Caller search: `rg '\bresolved_unit_surface\(' omega-rust` → 3 matches (read): definition `rewrites.rs:281`, uses `branch_and_compare_rewrites.rs:422` and `:1070`.

Any non-empty `plain_uses` cannot satisfy `plain_uses.iter().all(|unit| rewritten.implicit_uses.contains(unit))` when `rewritten.implicit_uses` is empty.

Verdict **CLOSED** for unsound constant-boolean admission: extra non-flag implicit uses are collected unfiltered but always fail before rewrite applies.

## A0640

Digest (read): area_04.md:227 — flag unit live out of the function is not considered; `block_instructions` includes the terminator (confirmed).

Opened `audit_dead_unit` at `branch_and_compare_rewrites.rs:1617-1652` (read). Walk starts at `(block_index, compare_index + 1)` and follows successors at `:1645-1649` with `(target, 0)`. Stops when a block ends without seeing `implicit_uses` on the unit; there is no scan of caller-save or ABI live-out beyond the function’s blocks.

Opened `block_instructions` at `rewriting.rs:41-46` (read): chains block instructions with `terminator_instruction`, so terminator operands are included when the walk reaches them inside a block.

Caller search: `rg '\baudit_dead_unit\(' omega-rust` → 2 matches (read): definition `:1617`, sole call `dead_compare::admit` at `:1687-1688`. The shadowed compare removal path at `:1508-1554` does not call `audit_dead_unit` (read).

Input in plain words: a compare defines a flag register that remains live into the return sequence without being listed on the terminator’s implicit uses can pass `audit_dead_unit` while still being live out of the function.

Verdict **REACHABLE** for dead-compare removal admission.

## A0641

Digest (read): area_04.md:228 — empty `reaching_events` feared to allow compare removal; digest records empty set is an error.

Opened shadowed admit loop at `branch_and_compare_rewrites.rs:1508-1554` (read). `reaching_events` at `rewrites.rs:384-454` (read). At `:442-445`, `entry_events[block_index].is_empty()` returns `Err(ConditionStateError::Use)` before an empty set is returned.

At `:1519`, `.map_err(|_| refusals.unsupported_use)?` turns that into refusal.

Caller search: `rg '\breaching_events\(' omega-rust` → 3 matches (read): definition `rewrites.rs:384`, callers `rewrites.rs:367` (`reaching_event`) and `branch_and_compare_rewrites.rs:1510`.

Input in plain words: the compare-removal path cannot proceed when no reaching definition exists for a flag unit; the empty-set case is rejected, not accepted.

Verdict **CLOSED**.

## What I did not read

- Full `reseat` body after `:1059` and all `leaf_copy` helpers.
- Entire load-forwarding merge loop after `address_and_load_rewrites.rs:585`.
- `agree_compare_removal_row` body at `branch_and_compare_rewrites.rs:1788` and full `admit_shadowed_compare` agreement hooks.
- Corpus or native execution behavior beyond cited Rust paths.
- `provider-admission` or stage-02 guarantees that might reject programs before these sites run.

SLICE COMPLETE
