slice_0083

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0634 | `selection_scalar_graph.rs:2972-2990`, `:3628-3655`, `:6777-6789` | CLOSED | Non-`required_values` get `Unused` transport by design; integer constants are range-checked in legalization before selection emits `MaterializeI64`. |
| A0635 | `selection_structural.rs:1027-1050`, `:227-230`, `:5522-5531` | CLOSED | `OwnedResult` admission matches the legalized target row; no-op rows and unused owned entry skips are intentional; `reseat` re-validates geometry. |
| A0636 | `structural_inputs.rs:110-131`, `:2385-2410`, `:2437-2440` | CLOSED | Descriptor-frame and empty-reach branches are deliberate geometry rules; production store paths re-check indices at `reseat` (`selection_structural.rs:1075-1078`). |
| A0637 | `address_and_load_rewrites.rs:880-881`, `rewrites.rs:2807`, `:1218-1228` | REACHABLE | Stored-load scan treats `WriteOutgoing` as non-interfering and lets `UnrosteredReach::Private` instructions without memory rows pass `reject_unaccounted`, so forwarding can admit past an overlapping outgoing store. |
| A0638 | `copy_and_extension_rewrites.rs:1464`, `:1475-1477` | REACHABLE | `identity` accepts `MeaningfulLow` + `Zero{width:32}` via `n <= 32` without proving high i64 lanes are zero; removal is gated only by that relation. |
| A0639 | `branch_and_compare_rewrites.rs:157-160` | CLOSED | Non-flag implicit uses skip `reaching_event` but must still satisfy `resolved_unit_surface` / `admits_unit_flow`, which requires empty `plain_uses` against an empty rewritten row. |
| A0640 | `branch_and_compare_rewrites.rs:1617-1653` | CLOSED | `audit_dead_unit` walks `block_instructions` (includes terminator per `analyses/src/rewriting.rs:41-46`) and rejects any later `implicit_use` of the unit. |
| A0641 | `branch_and_compare_rewrites.rs:1508-1560` | CLOSED | `reaching_events` returns `ConditionStateError::Use` when the entry event set is empty (`rewrites.rs:442-445`); compare removal maps that to refusal. |

### A0634

1. Read `selection_scalar_graph.rs:2897-3007` (`successor`): binding transport is `Registers` only when `builder.required_values.contains(&semantic.parameter)` (read `:2972-2990`); else `SelectedValueTransport::Unused` (read `:2988-2990`).
2. Read `:6768-6793` (case-payload `successor`): same pattern for `payload.parameter.value` (read `:6777-6788`).
3. Read `integer_arithmetic::emit` at `:3599-3655`: `LegalizedScalarInstructionKind::Constant` checks boolean and IEEE float bit patterns only at selection (read `:3627-3648`); then `emit_materialize_i64` (read `:3650-3654`).
4. Read `selection.rs:1624-1708` (`required_values`): backward closure from terminators and instruction operands populates the set (read).
5. Read `legalization_scalar_graph.rs:7620-7625`: `AbstractOperation::IntegerConstant` legalizes only if `valid_literal(*scalar_type, *value)` (read); `valid_literal` at `:7731-7746` enforces width (read).
6. Search `rg 'successor\(source, order, builder' …/selection_scalar_graph.rs` → 8 hits (read); all are terminator construction inside the same module (`:2737`, `:2831-2832`, case-dispatch successors).
7. Search `rg 'integer_arithmetic::emit' omega-rust/omega` → 1 hit, caller `selection_scalar_graph.rs:7485` (read, in block emission loop).

**Input reaching accepting branch:** Edge-binding or case-payload parameters whose `ValueId` is absent from `required_values` (computed at builder init from `value_transport::required_values`, read `selection_scalar_graph.rs:7122`).

**Verdict: CLOSED.** Skipping transport for values not in `required_values` matches the liveness/use closure. Integer constants out of carrier range cannot reach selection: legalization drops them at `:7625` with `valid_literal`.

**Unsure:** Nothing material for verdict.

### A0635

1. Read `selection_structural.rs:1009-1078` (`reseat` / `StoreStructuralField`): `OwnedResult { .. } => true` at `:1044` (read); failure if `!destination_admits` at `:1049-1050`.
2. Read `:1075-1078`: `geometry.byte_offset` and `runtime_indices_match` must match row (read).
3. Read `selection_structural.rs:134-229` (`entry`): owned inline path; at `:227-229`, `owned_pointer.is_none() && !referent_used` → `continue` (read).
4. Read `:5522-5531`: `ReleaseReference` / `EstablishTrivialAffineLocal` with no result return `Ok(true)` without emitting (read).
5. Read `legalization_source.rs:954-985`: `StoreStructuralField` legalized from matched abstract/target pair only (read).
6. Search `rg 'leaf_copy::reseat' omega-rust` → 1 hit at `selection_structural.rs:5457` (read); search `rg 'structural::operation\(' …` → 1 production call at `selection_scalar_graph.rs:7337` (read).

**Input:** Legalized `StoreStructuralField` rows; entry-time owned parameters; custody rows without results.

**Verdict: CLOSED.** `OwnedResult` is a valid store root for function-result homes; wrong destinations would mismatch projection/geometry at `:1075-1078` or fail earlier legalization. `Ok(true)` no-ops are schema-legal rows. Entry skip applies only when referent is unused (read `parameter_use::referent_used` gate at `:227`).

**Unsure:** Did not read full `legalization_reference_custody.rs:6479+` body.

### A0636

1. Read `structural_inputs.rs:110-137` (`runtime_reach`, `runtime_span_within_storage`): `RuntimeReach::Empty` ⇒ span check returns `true` (read `:131`, `:130-137`).
2. Read `:2385-2409` (`scalar_access_within_storage`): `descriptor_frame` OR `runtime_span_within_storage` against `shape(root)` (read); descriptor_frame at `:2392-2400`.
3. Read `:2412-2440` (`field_read`): element-view root sets `root = None` (read `:2431-2432`); size guard at `:2437-2440` skipped when `root.is_none()`.
4. Read `selection_structural.rs:1075-1078` on store path (cross-check).

**Input:** Structural geometry queries with borrowed-view/element-view roots, empty runtime element lists, or mixed field reads on view roots.

**Verdict: CLOSED for compilation acceptance.** Helpers return `Option`; callers like `reseat` reject mismatch. Descriptor-frame bypass is intentional for view-descriptor shapes; empty reach is the zero-extent case.

**Unsure:** Did not enumerate every `field_read` caller beyond `selection_inputs.rs:3517` (read one).

### A0637

1. Read `address_and_load_rewrites.rs:842-882` (`interferes`): `WriteOutgoing` / `AddressOutgoing` ⇒ `false` (read `:880-881`).
2. Read `:529-545` (backward scan): on `!has_row`, call `reject_unaccounted` (read `:543-544`).
3. Read `:1218-1227` (`reject_unaccounted`): `UnrosteredReach::Private` ⇒ `Ok(())` (read `:1222`); `Unaccounted` errors (read `:1224-1226`).
4. Read `rewrites.rs:2803-2807` (`unrostered_reach`): default `_ => UnrosteredReach::Unaccounted` (read).

**Input:** Stored-load forwarding walk encountering outgoing-argument stores that alias the forwarded slot, or barrier instructions classified `Private` without memory-access rows.

**Verdict: REACHABLE.** Smallest failure shape: a load is forwarded across an earlier `WriteOutgoing` to the same `(place, offset)` because `interferes` returns false; combined with `Private` no-row candidates clearing the scan, the rewrite admits unsound forwarding on an otherwise legal selected program.

**Unsure:** Did not read full `admit` from `:416` to completion.

### A0638

1. Read `copy_and_extension_rewrites.rs:1428-1490` (`producer_normalization`, `identity`, `Pattern` via `as u64` at `:1462-1465`) (read).
2. Read `:1472-1477`: `Zero{width:32}` + `MeaningfulLow{n}` ⇒ `n <= 32` (read).
3. Read `:1579-1582`: removal requires `identity(form, normalization)` (read).

**Input:** `ZeroExtendU32` (or similar) whose sole producer is `Float32ToBits` / `LoadPacked` normalized as `MeaningfulLow`.

**Verdict: REACHABLE.** A defining instruction can leave non-zero bits above the meaningful low width while still satisfying `n <= 32`; the extension fold admits and removes the zero-extend without a bit-level proof in this file.

**Unsure:** Did not verify hardware/selection invariants that clear high lanes after `Float32ToBits`.

### A0639

1. Read `branch_and_compare_rewrites.rs:156-175`: non-flag units go to `plain_uses` without `reaching_event` (read `:157-159`).
2. Read `:201-207`: `rule.admits_unit_flow` (read).
3. Read `rewrites.rs:281-297` (`resolved_unit_surface`): all `plain_uses` must appear in `rewritten.implicit_uses` (read `:288-290`).
4. Read `branch_and_compare_rewrites.rs:130`: rewritten `MaterializeI64` row must have empty `implicit_uses` (read).

**Input:** Boolean materialization with extra non-flag implicit units in the consumer instruction.

**Verdict: CLOSED.** Any non-empty `plain_uses` fails `resolved_unit_surface` against an empty rewritten row, so admission errors at `:207` despite no per-plain `reaching_event`.

**Unsure:** None.

### A0640

1. Read `branch_and_compare_rewrites.rs:1617-1652` (`audit_dead_unit`) (read).
2. Read `analyses/src/rewriting.rs:41-46` (`block_instructions` chains terminator) (read).
3. Read `:1687-1688`: called for each compare implicit def/clobber unit before dead-compare removal (read).

**Input:** Compare instruction whose flag units might still be referenced after removal point.

**Verdict: CLOSED.** Sub-question answered: `block_instructions` includes the terminator. Any later `implicit_use` of the unit returns `DeadCompareError::UnsupportedUse` (`:1632-1633`).

**Unsure:** Did not read liveness analysis; audit is implicit-unit based only.

### A0641

1. Read `branch_and_compare_rewrites.rs:1508-1519`: loop calls `reaching_events` (read).
2. Read `rewrites.rs:442-445`: empty or unknown entry events ⇒ `Err(ConditionStateError::Use)` (read).
3. Read `branch_and_compare_rewrites.rs:1519`: mapped to `refusals.unsupported_use` (read).

**Input:** Compare removal with a unit whose reaching event set would be empty.

**Verdict: CLOSED.** Reader note confirmed: empty `reaching_events` is an error, not acceptance.

**Unsure:** None.

### What I did not read

- Full `legalization_reference_custody.rs` `StoreStructuralField` custody walk past `:6514`.
- Complete `address_and_load_rewrites.rs` `admit` after `:580`.
- Full `redundant_compare` / `admit_shadowed_compare` modules beyond cited spans.
- All `field_read` call sites outside `selection_inputs.rs:3517`.
- Stage-02 abstract-operation proofs for structural bounds.

SLICE COMPLETE
