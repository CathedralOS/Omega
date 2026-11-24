# Packet: unit 015 gap 002 (area 10, stage 07 spill and split)

Packet complete. Every open or inferred claim in the stage 07 spill-and-split record has a disposition, and none is a demonstrated compiler bug. Of the four "unsure" items, three are guarded by an invariant upstream or downstream, and one (`before_fixed_use`) is sound by construction. The digest's reported disagreement between the two split settlement-shift rules is one rule written two ways. No code change was needed and no claim is left unresolved.

The [record](../../records/unit_015_gap_002.md) covers lines 541–5493 of `runtime_spill_and_split.rs`. That is the spill pass, which stores a value to a frame slot and reloads it before each use, and the split pass, which copies a value pinned to a fixed register into a flexible one. The baseline is `24ad2700685e6cc6062026c87ed5fd24ed415265`. The file is unchanged between that revision and `origin/main` at `a0152d26`, so all line numbers below match both. When review began, no merged triage ledger covered this range ([`07-13-psi-and-native.md`](../07-13-psi-and-native.md) has no entry for it), and no open pull request or registered claim touched the file.

Paths are relative to `source-rust/subsystems/compiler/psi-to-native/`. `S07` is `pipeline/07_selected-instructions-to-register-homes/src/allocation/` and the file is `S07/runtime_spill_and_split.rs`. The work was code reading and tracing on the owner code; no build or test was run because no claim needed a fixture to settle it.

## Summary

| # | Claim (record section; digest 10 line) | Outcome |
| ---: | --- | --- |
| 1 | Blocks without a slot summary contribute nothing to slot reuse (§4, "unsure" 1; L187, L214) | Intended: unreachable guard |
| 2 | `slot::assign` branch with no definitions (§4, "unsure" 2) | Intended: unreachable from the only caller |
| 3 | `load_slot` and `end_of_block_slot` fall back to a possibly undeclared private slot (§4; L187) | Intended: encoding refuses an undeclared slot |
| 4 | Entry fixed-view scan assumes `virtual_registers[id]` sits at index `id` (§8, "unsure" 3; L187) | Intended: invariant enforced by stage 05 |
| 5 | `before_fixed_use` renames one use without checking redefinitions or dominance (§10, "unsure" 4; L187) | Intended: sound by construction |
| 6 | Split's two settlement-shift rules disagree (§10; L74) | Intended: the same rule written two ways |
| 7 | `next_chunk` computes `byte_size - offset` unchecked (§7) | Intended: the only caller keeps `offset <= byte_size` |
| 8 | 128-unit mask path and set path for "a home survives" may disagree (§7, §8; L74, L239) | Intended: identical by construction |
| 9 | `unit_effects` trusts an empty liveness row as "nothing live" (§6) | Intended: matches the liveness producer's contract |
| 10 | `derive_runtime_spill` callers unknown (§8) | Inventory: one caller |
| 11 | Retained uses of a frame-address victim in establishing stores (§8) | Intended: the original register stays defined |
| 12 | `unreachable!()` after a transport `matches!` (§3; L241) | Intended: guarded by the same match |
| 13 | Split `charge` closure order (§10) | Matches as written |
| 14 | Inventory notes: duplicated `next_*_id` helpers, `surviving_home_exists` twice, shared settlement remap (§3, §4, §7, §10; L74) | Inventory, no correctness defect |

## Claims

### 1. Blocks without a slot summary contribute nothing to slot reuse

- **Original:** record §4 "Fail-open", third bullet, and "What I am unsure about", first bullet; digest 10 lines 187 and 214.
- **Code path:** the reuse scan in `RawCandidateSlots::scan` loops over `block_index in 0..block_count` (`:2229-2232`, `:2292-2295`), with `block_count = function.blocks.len()` (`:2179`). `representations/selected-instructions/src/blocks.rs:892-894` returns `Some(self.0.get(block_index)?.slot_summary())`. `slot_summary` (`blocks.rs:499`) always builds the summary through `OnceLock::get_or_init`.
- **Contract:** every block has a slot summary. `None` means only that the index is out of range.
- **Trace:** the loop never passes an out-of-range index, so the `continue` arm cannot run.
- **Outcome:** intended (unreachable guard). No fix.

### 2. `slot::assign` branch with no definitions

- **Original:** record §4 "Fail-open", first bullet, and "unsure" 2.
- **Code path:** `slot::assign` (`:1720`) has one caller, `derive_runtime_spill` at `:4685`, found by searching for `slot::assign`. The caller builds `definitions` at `:3650-3663` from one of three sources: a single `AfterInstruction` row, `entry_definitions` (`:2654-2663`, always one row), or `parameter_definitions`, which returns `UnsupportedUse` when its result is empty (`:4885-4887`). Later code only pushes to the list (`:3920`, `:4014`).
- **Trace:** `definitions` is never empty when `assign` runs. Even if it were, the branch at `:1745-1786` declares the private slot whenever a load is emitted.
- **Outcome:** intended (defensive and unreachable). No fix.

### 3. `load_slot` falls back to the private slot

- **Original:** record §4 "Fail-open", fifth bullet; digest 10 line 187.
- **Code path:** `Assignment::load_slot` and `end_of_block_slot` (`:1652-1664`) return `self.private` for a missing row. The rewrite reads them at `:1194` and `:1322`, and declares the private slot only when `assignment.declares_private` is set (`:1045`).
- **Contract:** `assign` fills a load row for every recorded use position (`:2061-2078`). A window that emits a load either picks a candidate slot or sets `declares_private` (`:2043-2049`). If the rewrite ever disagreed and reloaded from an undeclared slot, the frame address would name a `LocalStorageSlotId::Spill` missing from `local_storage_slots`.
- **Trace:** encoding admission resolves every frame address through `slot_region` (`representations/machine-code/encoding-admission/src/frame_address.rs:190-220`). That function requires a unique `local_storage_slots` entry and a unique placed slot. Otherwise it returns `ArtifactMismatch`, and `local_slot_region` (`:39-60`) does the same. A missing slot is therefore refused, never encoded at a wrong offset.
- **Outcome:** intended (the fallback fails closed downstream). Whether a disagreement can actually arise was not reproduced. No fixture separates the two decisions, and none is needed for soundness.

### 4. The entry fixed-view scan assumes positional register IDs

- **Original:** record §8 "Fail-open", last bullet, and "unsure" 3; digest 10 line 187.
- **Code path:** `:4209-4220`. Once a register ID is 256 or more, the loop scans `virtual_registers[id..]` and stops.
- **Contract:** `SelectedFunction::ids_are_positions` (`representations/selected-instructions/src/control_flow.rs:80-97`) requires `virtual_registers[i].id == i`. Stage 05 refuses a function that fails it: "selected block or virtual register ID differs from its position" (`pipeline/05_target-operations-to-selected-instructions/src/instruction_selection/selection_scalar_graph.rs:7813-7817`). Stage 07 rewrites append registers with `next_register_id = virtual_registers.len()` (`:3362-3364`, `:5228`), and the recovery codec checks the same predicate (`representations/register-homes/src/recovery/fixed_view_copy_codec.rs:718`).
- **Trace:** with positional IDs the tail slice starts at the current register, which the loop has already examined. The loop is therefore equivalent to a full scan and cannot skip a fixed view.
- **Outcome:** intended (stage 05 enforces the invariant). The early break saves no work and could be simplified, but it is not a correctness defect.

### 5. `before_fixed_use` renames one use

- **Original:** record §10 "Fail-open", first and second bullets, and "unsure" 4; digest 10 lines 51 and 187.
- **Code path:** `ValidatedRuntimeSplit::before_fixed_use` (`:5244-5348`). It is called from the allocator's recovery loop at `S07/assignment.rs:1784`.
- **Trace:** the function inserts the copy `fresh = CopyI64 register` directly before the chosen consumer, at the same instruction index (`:5332-5335`), and renames only that consumer operand. The copy therefore reads `register` at exactly the point where the consumer read it, so a redefinition elsewhere cannot change what the consumer sees. `fresh` has one definition and one use, adjacent in the same block, so no new dominance requirement exists. The original consumer already named `register`, so `register` already reaches that point. The chosen operand must be an untied `Use` (`:5301-5306`), and the copy row comes from `admission::copy_row` (`:5114`). A fixed-view use that occurs only in an edge-transfer block is not found, and the function refuses with `UnsupportedUse` (`:5298`).
- **Outcome:** intended (sound by construction). No fix.

### 6. The two split settlement-shift rules

- **Original:** record §10 "Same job elsewhere"; digest 10 line 74 ("split's two different rules").
- **Code path:** `before_fixed_use` inserts at `instruction_index` and shifts settlements with `instruction_index >= insertion` (`:5338-5346`). `split_selected_runtime_value` inserts at `definition_index + 1` (`:5419-5420`) and shifts with `shifted_position`, which moves positions `> definition_index` (`:5486-5492`, call at `:5424-5426`).
- **Trace:** for integers, `p > definition_index` holds exactly when `p >= definition_index + 1`, the insertion index. Both rules shift every settlement at or after the inserted instruction.
- **Outcome:** intended. The digest's "different rule" is one rule written two ways. Sharing one helper would be a tidy-up, not a repair.

### 7. `next_chunk` subtraction

- **Original:** record §7, helpers bullet.
- **Code path:** `next_chunk` (`:3286-3291`) has one caller, at `:4608`. `offset` starts at 0 (`:4555`). It grows only by a `width` that `next_chunk` returned, which is at most `byte_size - offset` (`:4653`), or it is set to `byte_size` by the `CopyBytes` arm (`:4594`). Once `offset == byte_size`, the next call returns `None` and the caller refuses with `UnsupportedUse` (`:4608-4610`).
- **Outcome:** intended (`offset <= byte_size` holds by induction). No fix.

### 8. 128-unit mask path and set path

- **Original:** record §7 "Same job elsewhere" and "Stated limits", and §8 "Fail-open" fourth bullet; digest 10 lines 74 and 239.
- **Code path:** the mask path (`:4330-4356`) runs only when every unit of every physical view is below 128 (`:4177-4182`) and no implicit unit is 128 or more (`:4263`). Under those conditions it excludes exactly the same sets as `surviving_home_exists` (`:3293-3323`): implicit-use units, pinned view units and reserved units. Reserved units of 128 or more appear in no view, so leaving them out of `reserved_mask` (`:4282-4287`) changes nothing. Both paths look up views by `views.get(id)` and check `view.id == view_id`.
- **Outcome:** intended (the two paths agree by construction). The duplication remains in the inventory.

### 9. `unit_effects` and empty liveness rows

- **Original:** record §6 "Fail-open".
- **Code path:** `preserve_live_units` (`:3012`) skips a definition or use whose liveness row has no instructions (`:3073-3097`, `:3119-3136`). `compute_function_units_filtered` (`pipeline/06_selected-instructions-to-selected-instructions/analyses/src/liveness.rs:622-670`) returns an empty row in two cases. One is a block that is not in `needed_blocks`. `preserve_live_units` marks every definition and use block as needed (`:3044-3059`). The other is a block where no filtered unit is live at exit and none is used implicitly inside it (`liveness.rs:662-667`). In that case no clobbered unit is live anywhere in the block.
- **Outcome:** intended. An empty row means that no clobbered unit is live in the block.

### 10. `derive_runtime_spill` callers

- **Original:** record §8 "Unused or one-caller".
- **Trace:** `grep -rn -F derive_runtime_spill source-rust` finds the definition (`:3486`), one import (`:954`) and one call (`:1029`, inside `spill_selected_runtime_value_with_crossing_status`).
- **Outcome:** inventory (one caller).

### 11. Retained uses in establishing stores

- **Original:** record §8 "Fail-open", second bullet.
- **Code path:** when the victim is defined by `FrameAddress`, the spill gathers the `(operation, place)` pairs from that instruction's `AddressLocal` accesses (`:3689-3707`) and the `WritePlace` stores for the same pairs (`:3708-3724`). A use in one of those stores is accepted only as operand 0 of `Store` or `ZeroBytes`, and is recorded in `retained_uses` (`:3940-3952`). The rewrite then leaves that operand unchanged (`:1174-1184`).
- **Trace:** the original definition stays in place (spill inserts stores after it and never removes it), so the retained operand still reads a defined register. The allocator gives it an ordinary live range from that definition to the store. This weakens how much pressure the spill relieves, but it does not change the program.
- **Outcome:** intended. No comment states the reason. The likely purpose is to keep the address-establishing store visible to stable-address checks, but this was inferred and not verified.

### 12. `unreachable!()` after a transport match

- **Original:** record §3 "Cases by shape"; digest 10 line 241.
- **Code path:** `:1353-1357` and `:1431-1435` destructure `Registers` transports. The enclosing branch is entered only after `matches!` checks the same transport, as the record and digest both trace.
- **Outcome:** intended (unreachable by routing; this confirms the digest).

### 13. Split `charge` closures

- **Original:** record §10 "Fail-open", third bullet.
- **Code path:** `charge` (`:5202-5220`) passes `|| IdentityOverflow` and then `|| WorkBudgetExceeded` to `charge_steps`. The record's reading matches the code.
- **Outcome:** matches as written.

### 14. Inventory notes

- **Original:** record §3, §4, §7 and §10 "Same job elsewhere"; digest 10 line 74.
- The `next_instruction_id` and `next_register_id` copies, the two `surviving_home_exists` forms and the shared settlement remap are duplication with the same behavior (see claims 6 and 8). The resemblance between slot reuse and the analyses crate's liveness was a suspicion in the record and was not compared further.
- **Outcome:** inventory. No correctness defect. These do not reach the [code repair queue](../../../../../OWNER_CODE_CONCERNS.md) threshold on their own.

## Unresolved claims

None.
