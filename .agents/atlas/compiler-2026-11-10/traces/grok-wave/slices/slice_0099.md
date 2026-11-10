slice_0099

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0760 | `omega-rust/omega/compilation/representations/register-homes/src/recovery/structural_calls.rs:852` | REACHABLE | Decode keeps arbitrary 32-byte commitment; admission recomputes only on the provider path, not on fixed-view-copy recovery decode. |
| A0761 | `structural_calls.rs:1476`; `structural_declarations.rs:518`; `structural_contracts.rs:404-405` | REACHABLE | Encode never writes three fields; decode zeroes or fixes them; transformed plan identity hashes the wire shape, not withheld semantics. |
| A0762 | `omega-rust/omega/compilation/representations/selected-instructions/analyses/src/fixed_precolored_split_requirements.rs:455` | REACHABLE | Ignored `use_transition` still leaves a segment; `finish` can pass with an empty `used` set while cuts exist. |
| A0763 | `omega-rust/omega/compilation/representations/selected-instructions/analyses/src/live_ranges.rs:740-849` | REACHABLE | `register_index` miss (`None` or `u32::MAX`) skips fragment work with no `LiveRangeError`. |
| A0764 | `omega-rust/omega/compilation/representations/selected-instructions/analyses/src/live_ranges.rs:1145-1149` | REACHABLE | Non-canonical duplicate ids get empty `fragments` in extra rows; analysis admits without error. |
| A0765 | `omega-rust/omega/compilation/representations/selected-instructions/analyses/src/machine_effects.rs:953` | CLOSED | Only `seal_pre_allocation_machine_effects` calls `new`, after `compute_terminal_pre_allocation_machine_effects` and `receipt(&plan)`. |
| A0766 | `omega-rust/omega/compilation/representations/selected-instructions/analyses/src/spill_choice.rs:329` | UNDECIDED | `choice: None` matches the no-pressure branch; under-reporting pressure was not traced to a caller. |
| A0767 | `omega-rust/omega/compilation/representations/selected-instructions/src/blocks.rs:346-363`; `blocks.rs:717` | UNDECIDED | Terminator fixed defs omitted from lookup bits; four `has_fixed_definition` uses look conservative; full accept path not traced. |

### A0760

1. Read `decode_native_callback_argument` (`structural_calls.rs:817-866`): line 852 `registrar_application_commitment = cursor.array()?` (read); no hash recompute in this function.
2. Read encode in `legalized-operations/.../normalized_foreign.rs:124` (read): copies bytes to wire unchanged.
3. Search `rg 'decode_native_callback_argument' /agent/repos/omega/omega-rust` → 2 hits: definition and `decode_list` at `structural_calls.rs:782` (read).
4. Search `rg 'decode_normalized_foreign_call' /agent/repos/omega/omega-rust` → production in `structural_calls.rs`, list fill in `structural_contracts.rs:209` (read); rest tests.
5. Read `FixedViewCopyPlan::decode` (`fixed_view_copy_codec.rs:66-88`): envelope and transformed identity only (read); no commitment check.
6. Read provider check `provider-admission/.../source_imports.rs:411` (read): `callback.registrar_application_commitment != application.commitment` on admission, not recovery decode.
7. Grep `registrar_application_commitment` under `pipeline/05_selected-instructions-to-register-homes` → 0 hits.

**Input to accepting branch:** Any fixed-view-copy wire blob that passes `OMGFCV` decode supplies 32 arbitrary commitment bytes per callback.

**Verdict:** REACHABLE — Smallest bad case: foreign-call callback in a decoded fixed-view-copy artifact whose commitment bytes do not match the admitted registrar application; decode accepts them and they are not recomputed on this path.

**Unsure:** Whether later register-homes stages consult commitment before executing recovered calls (not grep-visible in pipeline 05).

### A0761

1. Read `decode_result` (`structural_calls.rs:1460-1484`): `qualification_establishments: Vec::new()` (read); encode at `1443-1457` does not write establishments.
2. Read `decode_target_parameter` (`structural_declarations.rs:510-521`): `projected_qualifications: Vec::new()` (read); semantic path still decodes projections (`496`, read).
3. Read `encode_boundary_settlement` (`structural_contracts.rs:400-405`): `let _ = settlement.realization; bytes.push(1)` (read); decode at `482` forces `ClaimCompletionOnlyRealization`.
4. Read `selected_instruction_plan_identity` (`SI/plan_identity.rs:40-96`, read): identity from encoded page digests of embedded plan, not separate semantic fields.
5. Digest check 20 (read in `area_05.md:590,749`): same class of omission in legalized identity; `qualification_establishments` question narrowed.

**Input:** Round-trip through register-homes recovery codecs; withheld fields never appear on wire.

**Verdict:** REACHABLE — Smallest bad case: transformed selected plan inside a fixed-view-copy record that, if it had nonempty establishments/projections/realization semantics in memory, round-trips to empty/fixed values while envelope identity still matches the encoded bytes.

**Unsure:** Whether the compiler ever emits nonempty values for these three fields into plans that use this codec (inferred nonempty use in `unit_validation.rs` elsewhere, not read on recovery path).

### A0762

1. Read `fragment` (`fixed_precolored_split_requirements.rs:407-486`, full function read).
2. Line 455 `cuts.use_transition(&prior, site, destination_view);` — return ignored (read).
3. Read `use_transition` (`145-167`, read): returns `false` when no unique matching entry transition.
4. Read `require_transition` (`124-142`, read): errors on `false`; not used at 455.
5. Read `finish` (`169-188`, read): `self.used.iter().all(...)` — vacuously true when `used` is empty.
6. Read caller `register` (`302-404`, read): `cuts.finish(function, register)?` after all fragments.
7. Search `rg 'use_transition\(' omega-rust` → 2 hits: `require_transition` and line 455 (read).

**Input:** Live range where shared candidates empty at a legality span boundary (`439-441`, read); `entry_transitions` lack a matching authentication for the cut.

**Verdict:** REACHABLE — Smallest bad case: fixed-precolored interval with an unauthenticated domain break at an internal cut site; segment is recorded even when `use_transition` fails and `finish` may still return `Ok` if nothing was marked used.

**Unsure:** Whether downstream segment-home analysis always fails on such requirements (not read past `analyze_fixed_precolored_split_requirements` export).

### A0763

1. Read `register_fragments` (`live_ranges.rs:720-867`, full function read).
2. Lines 740-742, 773-775, 816-818, 837-839, 847-849: guard `register_index.get(...)` and `idx != u32::MAX` (read); else skip.
3. Read `compute_function` index build (`1047-1056`, read): `vec![u32::MAX; max_reg_id + 1]`; only declared ids mapped.
4. Read `analyze_live_ranges` (`1629-1635`, read): `require_liveness_of` checks identities only (`1609-1618`, read); no operand/register cross-check.
5. Search `rg 'compute_function' live_ranges` — single compute path into `admit` (read).

**Input:** Liveness operand or block live set naming a virtual register id with no `virtual_registers` row (or unmapped index).

**Verdict:** REACHABLE — Smallest bad case: selected function whose liveness mentions a virtual register id that is absent from the function’s `virtual_registers` table; live-range analysis yields no fragments for that id and returns `Ok`.

**Unsure:** Whether the selected-instruction producer always keeps operands and register tables consistent (not read).

### A0764

1. Read duplicate detection (`live_ranges.rs:1048-1055`, read): `has_duplicate_register_ids = true` on second same id.
2. Read row build (`1108-1166`, read): `fragments` taken only when `pos == canonical_idx`; else `Vec::new()` (1145-1149).
3. No error branch on `has_duplicate_register_ids` before `Ok(FunctionLiveRanges)` (1172, read).

**Input:** `virtual_registers` table with two rows sharing the same `VirtualRegisterId`.

**Verdict:** REACHABLE — Smallest bad case: duplicate virtual-register declarations where the non-canonical row keeps occurrences but empty fragments; analysis completes without error.

**Unsure:** Whether encoded selected plans can contain duplicate ids without failing an earlier validator (not read).

### A0765

1. Read `ValidatedPreAllocationMachineEffects::new` (`machine_effects.rs:953-961`, read): stores plan and receipt without cross-check.
2. Search `rg '\bValidatedPreAllocationMachineEffects::new\b' omega-rust` → 1 hit at `881` (read).
3. Read `seal_pre_allocation_machine_effects` (`877-882`, read) and `analyze_pre_allocation_machine_effects` (`903-922`, read).
4. Read `receipt` (`999-1005`, read): copies identity fields from plan.
5. Read `compute_terminal_pre_allocation_machine_effects` (`142-209`, partial read): root and catalog validation before building plan.

**Input on production path:** Plan from compute plus receipt derived from same plan.

**Verdict:** CLOSED — Compilation acceptance goes through `analyze_pre_allocation_machine_effects` → compute → seal; `new` does not bypass compute.

**Unsure:** External test or harness calls to pub `new` (not searched outside `omega-rust`).

### A0766

1. Read `compute_function` tail (`spill_choice.rs:206-332`, read): `choice: None` when every register gets `first_free` (`206-216` continue loop).
2. Read `choose_spill_victims` (`510-533`, read): always seals computed plan.
3. Did not read `first_free` body or downstream consumers of empty spill choice (not read).

**Input:** Allocation legality scan where simplified per-register `common_candidates` always yield a free view.

**Verdict:** UNDECIDED — `choice: None` is the documented no-spill outcome; whether that model can under-report real pressure needs `first_free` and recovery-classification callers.

### A0767

1. Read terminator operand loop (`blocks.rs:346-363`, read): sets `fixed_views` / `referenced_register_bits`, not `fixed_definition_bits` (contrast body loop `314-316`, read).
2. Read `has_fixed_definition` (`717-721`, read): consults `fixed_definition_bits` only.
3. Search `rg 'has_fixed_definition' omega-rust` → 6 hits (1 def, 3 tests, `assignment.rs:1537`, `runtime_spill_and_split.rs:4951`) — read assignment and runtime_split branches.
4. `assignment.rs:1536-1540` (read): marks split inadmissible when `!has_fixed_definition`.
5. `runtime_spill_and_split.rs:4951` (read): errors when `!has_fixed_definition`.

**Input:** Virtual register whose only non-Use fixed-view operand is on the block terminator.

**Verdict:** UNDECIDED — Lookup omission is confirmed; traced consumers reject or disallow splits when the bit is false (conservative), not traced for an accept path that should have seen a terminator fixed def.

### What I did not read

- Full `compute_terminal_pre_allocation_machine_effects` and `compute_instruction` in `machine_effects.rs` beyond line 209.
- `first_free` and pressure recovery after empty spill choice (`spill_choice.rs`, `recovery_classification.rs`).
- Selected-instruction plan validation before liveness for operand/register consistency.
- Fixed-view-copy pipeline stages after `FixedViewCopyPlan::decode` beyond `seal_validation` and `apply_fixed_view_copy` headers.
- Full `CutIndex::boundary` and downstream fixed-precolored segment homes.
- `materialize_definition_bits` and terminator `MaterializeI64` handling beyond cited lines.

SLICE COMPLETE
