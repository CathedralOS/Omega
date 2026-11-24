# Triage packet: unit_022 (area 11, native stages 11-15)

This packet gives a disposition for each claim in [records/unit_022.md](../../records/unit_022.md).
That record covers lines 1-7800 of `image-emission/src/function_fragments.rs` and its three small child
files. Lines 7801-21313 belong to the gap packets (`unit_022_gap_009` onward) and are not judged here.

Baseline: `24ad2700685e6cc6062026c87ed5fd24ed415265`. Reviewed at `a0152d26`; nothing under
`15_objects-to-image/image-emission/` changed between the two. Line numbers below are unchanged from the record.
Paths are under `source-rust/subsystems/compiler/psi-to-native/pipeline/15_objects-to-image/image-emission/src/`;
`FF` is `function_fragments.rs`. Evidence is code reading; nothing was built or run.

Status: Packet complete. All 25 claims in the record have a disposition below. None is a demonstrated wrong
result, so this packet makes no code change. Two claims remain open as cleanup proposals, listed at the end.

## Claims

### F1. Empty installation call set accepts (record §1 "Fail-open risks", bullet 2; digest 11 line 203, `FF:3760`)
- Code path: `source::installed_calls::PreparedCalls::new` (`FF:3731-3790`) and `validate` (`FF:3792-3929`).
- Contract: every selected call owned by `CallSiteOwner::Installation` must belong to an
  `InstallStaticInterrupt` target operation and match its validation or work helper.
- Trace: `PreparedCalls::new` checks every installation-owned call against `targets` (`FF:3755-3759`) before the
  early return at `FF:3760`. With no targets, any installation-owned call is refused with "installation call has
  no owning operation", so the early return is reached only when `calls` is empty too. Calls with other owners
  are outside this validator's role. Each target carries `calls: [TargetInstallationCall; 2]`
  (`representations/target-operations/src/operations.rs:55`), so the "position 0 validate, others work" loop
  (`FF:3841-3851`) sees exactly two rows, and `installed_interrupts` counts exactly one of each (`FF:4593-4602`).
- Outcome: **intended** (refuses, no accept path).

### F2. Reads inside the IDT interval skip guard checks (record §1 "Fail-open risks", bullet 4; digest 11 line 203, `FF:5291-5311`)
- Code path: `installed_interrupts::publication::reads` (`FF:5263-5373`), `within` (`FF:5232-5238`),
  `Contract::derive` (`FF:4043-4160`).
- Contract: an installation load must address an operand input, a declared control word, or an authenticated
  read interval; loop-indexed intervals need the loop bound and the entry guards.
- Trace: `within` succeeds only when the address is the base plus a *constant* offset whose end fits the extent
  (`offset(address, base)` then `end <= extent`). The early skip at `FF:5302-5308` therefore exempts only
  constant-offset reads that lie entirely inside `pointers[0]` (the table operand, `FF:4051`) and its
  `table_bytes`. `Contract::derive` declares the same base and extent as `ReadKind::Table`
  (`FF:4146-4150`), and that arm requires no guard (`FF:5368`). Without the early skip such a read would reach
  `fresh_observations` (a no-op here: the only loads in `pointers[0]+c` are the operand loads it exempts,
  `FF:5492-5498`) and then the `Table` arm, with the same acceptance. Indexed gate reads
  (`table + index*16 + 5`) have no constant offset, so they fail `within` and still take the guarded `Gate`
  path (`FF:4126-4130`, `FF:5326-5361`).
- Outcome: **intended**. The early skip duplicates the declared `ReadKind::Table` interval; it accepts nothing
  that the interval roster does not already accept. Optional cleanup: drop the `within(..., table_bytes)`
  clause and let the `Table` interval decide. Not filed as a defect.

### F3. Operand-pointer loads are exempt from freshness (record §1 "Fail-open risks", bullet 5; digest 11 line 203, `FF:5522-5534`)
- Code path: `installed_interrupts::publication::fresh_observations` (`FF:5486-5545`).
- Contract: a guard must use bytes observed after the validation call, except the operand pointers that the
  validator itself received.
- Trace: the exempt list is built only from `source_loads` of `input.pointers`, `stack_length` and
  `arrival_length` (`FF:5492-5498`); every other `Load` needs an observation that `validate` precedes
  (`FF:5527-5535`), and root intervals need an owned observation after `validate` (`FF:5512-5521`).
- Outcome: **intended** (record classified it as stated).

### F4. Non-canonical spans fall back to the full scan (record §1 "Fail-open risks", bullet 1; "Stated limits" bullet 4; `FF:514-525`, `FF:330-333`)
- Code path: `attribution::PhysicalSpans::prepare` and `validate_with_ordinals`.
- Trace: `prepare` returns `None` for overlapping or unordered spans (`FF:522-526`), and the caller then uses
  `interval_is_exact` (`FF:330-333`). Both paths must return `true` or the row is refused (`FF:334-337`).
- Outcome: **intended** (comment `FF:514-515`). The fallback changes the algorithm, not the verdict.

### F5. Descriptor replay returns early with no descriptor operations (record §1 "Fail-open risks", bullet 3; `FF:3369-3377`)
- Code path: `source::descriptor_tables::DescriptorReplay::new` (`FF:3330-3400`), `dedicated_descriptor_owner` (`FF:3618-3633`).
- Trace: before the early return, every descriptor hardware instruction (`SegmentRead`, `TaskRegisterRead`,
  `SegmentReload`, `TaskRegisterActivate`, `GlobalDescriptorTable`) must name exactly one source operation
  (`FF:3353-3355`) that is either a descriptor operation or an installation (`FF:3629-3632` refuses others).
  Installation-owned hardware is skipped here and then refused by the installation body allow-list, which has no
  `RegisterAccess` arm (`FF:4620-4655`).
- Outcome: **intended** (no accept path for an unowned hardware instruction).

### F6. Port spans without a physical origin (record §1 "Fail-open risks", bullet 6; `FF:2500`)
- Code path: `port_effects::append_function_effects`.
- Trace: `effects.len() - start != spans.len()` refuses with "port event has an unowned selected span"
  (`FF:2500-2502`).
- Outcome: **intended** (refuses).

### F7. Byte-literal pool absent on both sides (record §1 "Fail-open risks", bullet 7; `FF:1528-1534`)
- Code path: `byte_literals::validate`.
- Trace: `(None, None)` is accepted only when `text_bytes.len() == pool_text_offset`; a mismatched roster
  (`Some`/`None`) is refused (`FF:1535-1536`).
- Outcome: **intended**.

### F8. `CurrentImageSelections::prepare_for` returns `None` (record §2, "Fail-open"; `current_image_selections.rs:15-20`, `FF:1765-1768`)
- Trace: `prepare_for` and the consumer use the same `ImageAddressOrigins::indexes` predicate, so a missing
  selection is accepted only when no address needs one.
- Outcome: **intended** (record classified it as traced).

### F9. `image_data.rs` and `internal_calls::argument_extent` (record §3 and §4, "Fail-open")
- Trace: `ObjectImageData::prepare` only propagates errors. `argument_extent` checks
  `fragment.bytes.get(start..end) == Some(call.bytes)` for calls and requires disjoint transfers that meet the
  copy ends (`internal_calls.rs:61-114`).
- Outcome: **intended** (record reported none).

### F10. Internal-call records do not check the result kind (record §4, "Cases by shape")
- Code path: `internal_calls::place_calls` (`internal_calls.rs:120-170`).
- Trace: the function joins existing plans to spans. Each plan's source type is `InternalUnitCallSource`
  (`representations/machine-code/src/internal_call_plans.rs:61`), so the result kind is fixed by the plan
  producer, not here. Owners other than `Operation`, `InvokeNoNormal` and `CleanupAction` are refused
  (`internal_calls.rs:130-138`).
- Outcome: **intended** at this layer. The plan producer was not traced in this packet.

### F11. `place_calls` callers (record §4, "Unused code", suspected)
- Grep `place_calls` over `image-emission/src`: definition `internal_calls.rs:120`; callers `FF:20250`
  (`structural::validation`) and `FF:20906` (`structural::populate`).
- Outcome: **resolved**: two callers, both in gap-packet ranges.

### F12. Most `ObjectFunction` vectors start empty (record §1 "Inputs and outputs"; "Five things" 1; `FF:2880-2897`, `:2908`)
- Trace: `structural::populate` (`FF:20885`) fills them; the gap_011 record reads that function, and
  `validation.rs:710-725` checks the empty-record list.
- Outcome: **intended**; the population claim belongs to `unit_022_gap_011`.

### F13. Stack linkage 8 on x86-64, 0 on AArch64; default alignment 16 (record §1 "Inputs and outputs"; `FF:2812-2816`)
- Contract: an x86-64 `call` pushes an 8-byte return address; AArch64 `bl` writes the link register and pushes
  nothing. Both hosted ABIs require 16-byte stack alignment at calls.
- Outcome: **intended**.

### F14. Fixed descriptor operand roles and byte counts (record §1 "Cases by shape" bullets 1-2; `FF:3452-3537`)
- Contract: GDT entries are 8 bytes, the x86-64 task-state segment is 104 bytes and a GDTR/IDTR pseudo-descriptor
  is 10 bytes (2-byte limit + 8-byte base). Other register operations are refused (`FF:3507`).
- Outcome: **intended** (hardware-defined sizes; unsupported operations refuse).

### F15. Fixed installation call roster (record §1 "Cases by shape" bullets 3 and 5; `FF:3841-3851`, `FF:4593-4602`, `FF:4651-4655`)
- Trace: `TargetUnitOperation::InstallStaticInterrupt` carries `calls: [TargetInstallationCall; 2]`
  (`target-operations/src/operations.rs:55`). Unlisted instruction kinds are refused.
- Outcome: **intended** (see F1).

### F16. Fixed control-allocation offsets (record §1 "Cases by shape" bullets 4 and 6; "Five things" 3; `FF:4055-4072`, `FF:5064`, `FF:5297-5301`, `FF:6499-6543`)
- Contract: the stage 05 producer's layout. `selection_interrupts.rs:1437-1445` defines `CONTROL_BYTES` 128,
  `DESCRIPTOR_OFFSET` 48, gate 64, arrival 72, stack 80, match count 88, timer root 96, and
  `PAIR_BASE`/`PAIR_END` 112/120 (`:939-940`). The verifier's literals match those values.
- Outcome: **intended** (the verifier re-derives the layout independently). Open proposal U2 below.

### F17. Gate placement, gate numbers 32..=255, stack alignment (record §1 "Cases by shape" bullets 7-8; `FF:5628-5692`, `FF:4211-4219`, `FF:6165-6166`)
- Contract: an x86-64 interrupt gate is 16 bytes with the handler offset split into bits 0-15 and 48-95
  (hence `source < 16 ? source : source + 32`); vectors 0-31 are reserved for CPU exceptions. The producer applies
  the same alignment rule (`selection_interrupts.rs:1178-1179`).
- Outcome: **intended**.

### F18. Fixed root-word sets, cleanup suffix and the 18-variant `instruction_operation` list (record §1 "Cases by shape" bullets 9-11; `FF:6974-7037`, `FF:7135-7150`, `FF:7670`)
- Trace: each list refuses or returns `None`/`false` for unlisted shapes; no accept-on-missing path was found in
  lines 1-7800. `aggregate_results` continues into the gap_009 range, which owns its callers.
- Outcome: **intended** in this range.

### F19. Two interval-exactness checks (record §1 "Same job elsewhere" bullet 1; "Five things" 4; digest 11 line 108; `FF:591`, `FF:704`)
- Outcome: **intended** (comment `FF:514-515`: the fast path covers canonical spans and the full scan keeps the
  error order for other shapes). Duplicate of the digest 11 §"Same job" entry.

### F20. Four matches on the same `Control` variants (record §1 "Same job elsewhere" bullet 2; digest 11 line 108; `FF:170-263`, `:372-440`, `:536-577`, `:662-701`)
- Outcome: **still open** as a cleanup proposal (U1). No wrong result was found; the four matches agree at this
  revision.

### F21. Unique-row helpers, mirrored descriptor matchers, pool validators (record §1 "Same job elsewhere" bullets 3-5; digest 11 line 108)
- Trace: `unique` (`FF:6052`) and `unique_index`/`find` (`FF:3713-3729`) are private to their modules and differ in
  shape (single row vs. sorted index). `descriptor_tables::{source_operation,target_operation}` read two different
  representations. The four pool validators each rebuild a different pool.
- Outcome: **intended**; duplicate of the digest 11 §"Same job" entry. No repair proposed.

### F22. Possible shared logic with `installation_admission::selected` (record §1 "Same job elsewhere" bullet 6; "Unsure" 3)
- Trace: `FF:3935`, `:3946` and `:6050` import `control`, guard types and `selected_constant_operand` from
  `installation_admission::selected` instead of copying them.
- Outcome: **intended** for the imported items. Whether the remaining symbolic model duplicates that crate was not
  traced here; that crate belongs to its own unit.

### F23. Symbol names compared as strings (record §1 "How it decides" bullets 4-5; "Five things" 5; `FF:1548`, `:2138`, `:2144-2149`, `:2717-2718`)
- Trace: each comparison checks an emitted name against the name that `object_file` derives from the identity
  (`normalized_import_symbol_name(locator)`, `private_function_symbol_name(identity)`). Checking the spelling is
  the point of these validators.
- Outcome: **duplicate** of digest 11 "Spelling of symbol and section names" (line 52), which owns the area-wide
  question. No defect in this range.

### F24. Empty `impl ObjectArtifact {}` and stated limits (record §1 "Unused or one-caller code" bullet 3; "Stated limits"; `FF:3059-3060`, `:7096-7099`, `:4465-4467`, `:2766-2768`)
- Trace: the empty impl block has no items and no effect. The three stated limits refuse (`ReplayUnavailable`,
  incomplete installation view) or document audit inputs.
- Outcome: **intended** for the limits; the empty block is dead code included in U1.

### F25. `prepare_object_for_image` and `selected_constant_operand` callers (record §1 "Unused or one-caller code" bullets 1-2)
- Outcome: **intended**: one production caller (`native-artifact/src/fragment_object_publication.rs`) and a
  forwarding chain to `installation_admission::selected_constant_operand`. Matches the crate map.

## Unresolved claims

- **U1 (F20, F24):** consolidate the four `Control` matches in `attribution` and drop the empty
  `impl ObjectArtifact {}` (`FF:3059-3060`). Proposed fix: one helper that maps an instruction's `Control` to its
  semantic site, used by `produce_with_ordinals`, `validate_with_ordinals`, `PhysicalSpans::prepare` and
  `supports`. Code quality only; no wrong result.
- **U2 (F2, F16):** the verifier repeats stage 05's control-word offsets as literals and keeps a redundant
  table-interval skip in `publication::reads`. Proposed fix: name the offsets once in the shared
  `installation_admission` vocabulary and remove the `within(..., table_bytes)` clause. Code quality only.

Packet complete.
