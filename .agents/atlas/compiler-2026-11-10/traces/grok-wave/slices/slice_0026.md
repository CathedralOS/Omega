slice_0026

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0192 | emission.rs:1714-1716 | REACHABLE | `endpoint_use` maps missing declaration to `Storage` using the caller's type table while the formal may name a callee-only identity |
| A0193 | emission.rs:1855-1858 | CLOSED | Universal-dominator branch only when `entry` is `None`; both production callers pass `Some(...)` |
| A0194 | emission.rs:4546 | UNDECIDED | `filter_map` drops out-of-roster targets; did not trace whether lowering can emit such a target on checked input |
| A0195 | emission.rs:5248 | UNDECIDED | Runtime byte footprint probes only index 0; stage-05 uniformity for array qualifications not traced |
| A0196 | emission.rs:8245-8253 | REACHABLE | `BorrowedView` arm of `readable_byte_source_bound` omits capacity vs plan maximum |
| A0197 | emission.rs:9646 | CLOSED | `IntegerComparison` is a non-short-circuit leaf; operands are direct/scalar and both sides are emitted |
| A0198 | emission_cleanup.rs:1194-1196 | CLOSED | Sole caller only invokes `restore_path` when the place is already on the frontier |
| A0199 | emission_cleanup.rs:872-895,919 | UNDECIDED | Case/external terminators can carry structural arguments; effect of skipping `transfer` on acceptance not traced |

### A0192

1. Read (full): `completed_place::endpoint_use` at `emission.rs:1703-1728` (`read`).
2. Search `\bendpoint_use\(` in `omega-rust/` → 3 hits: definition `emission.rs:1703`, callers `unit_operation_frame.rs:1808`, `scalar_computations.rs:988` (`read`).
3. Read callers: call-path `unit_operation_frame.rs:1793-1837` passes `self.structural_types.declarations()`; scalar path `scalar_computations.rs:982-994` passes `self.structural_types` (`read`). Both feed `plan()` at `emission.rs:1574+` (`read` start; full `plan` read through frontier rules).
4. Input to accepting branch: after `type_identity_text` succeeds (`emission.rs:1709-1712`), `types.iter().filter(|row| row.identity == identity)` has no first row (`emission.rs:1713-1715`) — i.e. interned identity text is absent from the **caller's** declaration slice while the checked formal still has that `type_identity`.
5. Verdict **REACHABLE**: a program with a cross-unit or cross-roster call where the callee formal's type identity is checked but not present in the lowering unit's `StructuralTypeDeclaration` list would get `EndpointUse::Storage` instead of `Value`, skewing capture/`plan` for that operand without an error at this site.
6. Unsure: whether the checker always mirrors every callee formal identity into the active unit's lowered declaration table; did not read stage-05 call wiring end-to-end.

### A0193

1. Read (full): `control_flow::block_dominators` at `emission.rs:1841-1906` (`read`).
2. Search `\bblock_dominators\b` in `omega-rust/` → 3 hits: definition, `emission_cleanup.rs:1924`, `unit_composed_control.rs:6165` (`read`).
3. Read callers: `emission_cleanup.rs:1923-1924` uses `Some(entry_position)`; `unit_composed_control.rs:6165-6167` uses `Some(entry)` (`read`).
4. Input to accepting branch: `entry: Option<usize>` is `None` at `emission.rs:1855-1857`, yielding each block's dominator set equal to all blocks (`read`).
5. Verdict **CLOSED**: every traced production path passes `Some(...)`, so the universal-dominator fallback is not exercised on those paths today.
6. Unsure: none for traced callers; no third caller found.

### A0194

1. Read (full): `private_borrow_transport::forward_from` at `emission.rs:4456-4713`, especially successor build `4542-4548` (`read`).
2. Caller: `forward_private_state_borrows` → `unit_composed_control.rs:6634-6640` (`read` callsite only).
3. Input to accepting branch: terminator `target` / case target `BlockId` not found in `identities` (`identities = blocks.iter().map(|b| b.id)`, `4546`) — edge omitted from `successors` (`read`).
4. Verdict **UNDECIDED**: whether checked lowering can emit a terminator target outside the same machine's `blocks` id list.
5. Unsure: did not read all terminator builders in `unit_composed_control.rs` / `unit_arguments.rs` for target validation.

### A0195

1. Read (full): `require_unrestricted_byte_region` at `emission.rs:5212-5253` (`read`); caller `require_unrestricted_logical_footprint` at `5274-5276` (`read` snippet).
2. Input to accepting branch: `ByteStoreIndex::Runtime(_)` after `structural_memberships_disjoint_from_write` passes — only `FixedIndex(0)` is pushed and checked (`emission.rs:5238-5249`, `read`).
3. Verdict **UNDECIDED**: non-uniform per-index qualifications on the same array path not traced in stage 05.
4. Unsure: language/checker invariant that runtime-indexed byte regions are uniform across elements.

### A0196

1. Read (full): `readable_byte_source_bound` at `emission.rs:8227-8256` (`read`); use sites `8151-8155`, `8174-8184` (`read`); `emit_concatenation` `7922` capacity obligation on **result** (`read`).
2. Input to accepting branch: `BorrowedView` shape with any allowed borrow access, regardless of `maximum_length` (`emission.rs:8245-8253`, `read`).
3. Verdict **REACHABLE**: byte value plan can treat a whole borrowed-view source as matching a plan capacity at `8153` without comparing view capacity to `*capacity`, while concat only records a result-side obligation (`7922`).
4. Unsure: whether downstream interpreters enforce source length separately (not traced in stage 06).

### A0197

1. Read (full): `contains_short_circuit` at `emission.rs:9631-9654` and `validate_short_circuit_expression` at `9666-9699` (`read`).
2. Read `emit_boolean_expression` for `IntegerComparison` at `operation_emission.rs:119-129` — both operands emitted unconditionally (`read`).
3. Read stage-05 mirror `checked_boolean_contains_short_circuit` at `05/.../execution_terminal_unit.rs:3493-3520` — `IntegerComparison` treated as non-short-circuit (`read`).
4. Input to accepting branch: any `LoweredBooleanReturnExpression::IntegerComparison { .. }` classified as non-short-circuit at `emission.rs:9646` (`read`).
5. Verdict **CLOSED**: `IntegerComparison` is not an `And`/`Or` carrier; operands are `LoweredDirectExpression`, not nested boolean short-circuit forms; classification matches checking and emission behavior.
6. Unsure: none.

### A0198

1. Read (full): `restore_path` at `emission_cleanup.rs:1193-1212` (`read`).
2. Search `\brestore_path\(` in `omega-rust/` → 2 hits: definition and `emission_cleanup.rs:832` (`read`).
3. Read caller loop `emission_cleanup.rs:807-833`: `StoreStructuralField` only calls `restore_path` when `frontier.iter().any(|root| root.place == *destination)` (`813-814`, `read`).
4. Input to silent branch: `place` not in `frontier` at `1194-1196` (`read`) — not passed from the sole caller given guard `813`.
5. Verdict **CLOSED**: production path to `832` requires the place on the frontier first.
6. Unsure: none.

### A0199

1. Read (full): terminator handling in `projected_transfers::complete` at `emission_cleanup.rs:838-920` (`read`); `transfer` at `1023-1033` (`read`).
2. Read composed `StructuralCase` emission `unit_composed_control.rs:8200-8207` — case edges start with empty `structural_arguments` (`read`); `ExternalCompletion` terminator can carry `structural_arguments` from `operation_emission.rs:1720-1736` (`read`).
3. Read order: `forward_private_state_borrows` at `6634` before `affine_cleanup::complete` at `11206` (`read`) — case edges may gain arguments before `complete()`.
4. Input: non-empty `structural_arguments` on `StructuralCase` cases or `ExternalCompletion` without `transfer` (`872-895`, `919`, `read`).
5. Verdict **UNDECIDED**: whether non-empty lists on those terminators during `projected_transfers::complete` can change whether an unsound program lowers successfully.
6. Unsure: post-`forward_private_state_borrows` case-edge roster vs custody-transfer semantics; join logic at `932-936` not fully traced to acceptance.

### What I did not read

- Exhaustive terminator/target builders across `unit_arguments.rs` and remainder of `unit_composed_control.rs`.
- Stage-05 rules for array qualification uniformity and byte runtime-index stores.
- Terminal / validation enforcement of byte concatenation source lengths beyond stage-06 emission.
- Full `affine_cleanup` join and discard paths after skipped `transfer` on external/case edges.

SLICE COMPLETE
