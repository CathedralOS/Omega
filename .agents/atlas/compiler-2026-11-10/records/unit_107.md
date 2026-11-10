# Unit 107 notes

Files covered (all under `omega-rust/psi/foundation/`, checkout worker-23 at 457adf4a11f):
arena/src/paged.rs; build-time-values/src/lib.rs; compiler-storage/derive/src/lib.rs; compiler-storage/src/lib.rs;
compiler-storage/src/values.rs; diagnostics/src/lib.rs; evaluation-accounting/src/invocation.rs; evaluation-accounting/src/lib.rs;
evaluation-accounting/tests/accounts.rs; evaluation-accounting/tests/cached_usage.rs; extents/src/extent.rs; extents/src/lib.rs;
extents/src/loans_and_claims.rs; extents/src/mapping.rs; filesystem-authority/src/filesystem.rs; filesystem-authority/src/filesystem_sponsor.rs;
filesystem-authority/src/lib.rs; flow-effects/src/lib.rs; language-core/src/atomic.rs; language-core/src/inline_assembly.rs;
language-core/src/lib.rs; language-core/src/source_semantics.rs; language-semantics/src/const_value.rs; language-semantics/src/content.rs.

(Written in sections; section order follows the unit file. A section marked PENDING was not yet written when this note was last saved.)

---

## 1. arena/src/paged.rs (2609 lines; tests start 2005)

**Purpose.** Multi-page and concurrent arena containers built on the dense `Arena`/`Handle` of the arena crate: a set of generation-numbered pages behind one handle type (`PagedArenaSet`), a lock-free-publication concurrent page directory (`ConcurrentPagedArena` -> `SealedPagedArena`), a CAS-based slot arena with generation counters (`GenerationalPagedArena`), and two interners (`OwnedInterner`, `OwnedTextInterner`). Header comment paged.rs:1-13.

**Shape.**
- `ArenaPage<T>` (paged.rs:24) enum `Owned(Arena<T>) | Shared{arena: Arc<Arena<T>>, copy: fn}`; `DerefMut` (48-71) detaches a shared page (clone if `Arc::strong_count > 1`, else unwrap) -> copy-on-write. `share()` (74) only shares non-empty owned pages. Has `compiler_storage::Record` (130-160) with a bool tag for owned/shared.
- `PagedArenaFrontier<T>` (163): `{generation, next_arena_index}`; `contains` (170). Default (1,1).
- `PagedArenaSet<T: Default>` (196): `first_generation, first_page, additional_pages: Vec<(u32, ArenaPage)>, page_positions: Vec<u32> (one-based position at generation-page_base), page_base, current_generation, active_count, dummy: T`. Main entries: `share_pages` 234, `adopt` 287, `take_additional_pages` 301, `adopt_additional_pages` 317, `take_pages_from` 340, `begin_page` 361, `append` 397, `append_to_span` 403 (relocates a span to the current page if its page is not current: 407-413), `insert_many` 420, `get` 462, `get_mut` 468, `span` 482, `frontier` 640, `span_since` 553, `rewind_appended` 535, `revision` 578, `append_epoch` 584, `for_each_from` 609, `insert_page` 720 (keeps `page_positions` directory).
- `ConcurrentPagedArena<T>` (859): `directory: PageDirectory<T>` (segments of 256 `OnceLock<Vec<T>>`, 857/1186), `first_page_identity`, atomics `next_page/published_pages/published_values`. `reserve_page` 906 (CAS `try_update` on `next_page`), `ConcurrentArenaPage::{push, insert_many, publish}` 959/965/999 (publish sets the OnceLock page, panics if set twice 1011-1013), `seal` 928 (errors `IncompletePagePublication` if reserved != published), `SealedPagedArena` 1068 (`get`, `try_get`, `span`, `get_mut`, `page`, `into_pages`, `begin_concurrent_extension` 1077).
- `FreeStack` (1286): tagged (ABA-counter) Treiber stack over `AtomicU32` next array. `GenerationalPagedArena<T>` (1365): `pages: Box<[ArcSwapOption<Page<T>>]>`, `slot_metadata: AtomicU64` (state in low byte, generation in high 32 bits; SLOT_EMPTY/ACTIVE/FREED 1359-1361), PAGE_SIZE = 16 (1363). `alloc` 1419, `free` 1451, `get` 1486 returning `SlotRef` (dummy-backed when invalid), `reclaim_empty_pages` 1579 (body is `0`).
- `OwnedInterner<T>` (1701): `Mutex<T>` with `with`/`with_mut` (1741/1746); poison is ignored (`unwrap_or_else(into_inner)`). `OwnedTextInterner` (1752): 25 pages of `OnceLock<Box<str>>` (sizes 256<<page) plus 24 generations of an open-addressed `AtomicU64` index (probe 1907, `text_tag` 1947 is a custom 64->32 bit hash); `intern` 1787 (lock-free read probe, then `minted` mutex to mint), `slot` 1847, `text` 1837.

**Inputs/outputs.** Reads `crate::{Arena, ArenaSpanInserter, Handle, HandleSpan}`, `arc_swap`, `compiler_storage`. `PagedArenaSet` and `OwnedTextInterner`/`GenerationalPagedArena` carry mutable counters; `PagedArenaSet` mutators keep `active_count` in sync by diffing `Arena::len` before/after (e.g. 414-416, 421-423).

**How it decides.** All lookups are keyed on handle identity = (generation -> page, arena_index -> slot). `PagedArenaSet::page_for_generation` 679 compares to `first_generation` else `page_position` 696 (directory index by `generation - page_base`). `GenerationalPagedArena::get` 1486-1512 accepts only if slot state == ACTIVE and stored generation == handle generation. `OwnedTextInterner::probe` 1907 keys on a 32-bit tag then full string compare (`interner.text(slot) == Some(text)`, 1916): content-identity, not pointer.

**Cases by shape.** None of the "N shapes, refuse the rest" kind. Bounded by size: `GenerationalPagedArena::new` asserts max_capacity >= 16 (1374); `OwnedTextInterner` has fixed 25 text pages and 24 index generations (1759-1762); `slot < u32::MAX` assert 1803.

**Same job elsewhere.** The paged `Arena`+`Handle` abstraction lives in arena/src/lib.rs (not in my unit). `PagedArenaSet::free` (389) and `GenerationalPagedArena::free` (1451) are two different free-list/generation designs; the former delegates to `Arena::free` (not read). Dummy-fallback reads exist in three places in this file: `PagedArenaSet::get` 462-466, `SealedPagedArena::get` 1093-1095, `GenerationalPagedArena::get` 1486-1512.

**Unused / one-caller.** (grep over `omega-rust`, count mode, `Grep` tool)
- `GenerationalPagedArena`: 11 lines in paged.rs, 2 in arena/src/lib.rs (re-export), 0 elsewhere -> no users outside the arena crate (grepped). `reclaim_empty_pages` is `0` and a test is named "page_reclamation_is_disabled_until_quiescence_exists" (2542).
- `ConcurrentPagedArena|ConcurrentArenaPage|SealedPagedArena`: 26 in paged.rs, 3 in arena lib.rs, 4 in `psi/compilation/pipeline/01_tokens-to-syntax-trees/src/parser.rs` (grepped).
- `OwnedInterner`: 12 in paged.rs, 2 in arena lib.rs, 3 in `typed-trees/src/lib.rs`.
- `OwnedTextInterner`: 15 in paged.rs, 2 in arena lib.rs, 1 in `typed-trees/src/type_identity.rs`.
- `PagedArenaSet`: 26 in paged.rs, 2 arena lib.rs, ~101 across 20 other files (checked-trees, syntax-trees, step 05 checks, step 06 shared... via the count grep).

**Stated limits.** 1579-1581 `reclaim_empty_pages` returns 0; test name 2542 says reclamation "disabled until quiescence exists". `PagedArenaSet::for_each_from` (609-624): with more than one page it visits only the current page, skipping rows on earlier pages (`prefix_len` is only used to offset `start`), no comment states this.

**Fail-open risks.**
- `PagedArenaSet::get` returns `&self.dummy` (a `T::default()`) when the handle's page is missing (462-466), and `get_mut` returns `&mut self.dummy` (468-476) so a write through a stale/foreign handle is silently absorbed. Same in `SealedPagedArena::get` (1093-1095), `GenerationalPagedArena::get` (1486-1512, returns `SlotRef` with `dummy=true`; `is_dummy()` available). Wrong program getting through: a caller that does not check `is_valid`/`try_get` reads a default value for a dangling handle. Status: "open" for callers (not traced); tests 2461-2471 assert the dummy behaviour on purpose.
- `OwnedInterner::eq` always `true` (1726-1730) and `OwnedTextInterner::eq` always `true` (1967-1971): two containers holding different text compare equal, so any derived `PartialEq` on a struct that contains one ignores that field. Status: "open" (no comment states the intent here). Debug prints only the type name (1734, 1975).
- `OwnedInterner` lock poison is ignored in every access (1719, 1742, 1747): "stated" only by code shape, no comment.
- `PagedArenaSet::free` of an already freed/foreign handle returns false without error (389-395): benign.
- `ArenaPage::DerefMut` `unreachable!` branches (55-69): panics, not accepts.

---

## 2. build-time-values/src/lib.rs (91 lines)

**Purpose.** One enum `BuildTimeValue` (28-46): `Unit, LayoutTarget(LayoutTargetFacts), Int(i64), Bool, Float(f64), UndeterminedNaN(FloatFormat), Text(Vec<u8>), Struct{type_name: String, fields: Vec<(String, Value)>}, Case{variant: String, payload}, Array(Vec)` - the immutable value exchanged across compile-time argument/result boundaries, plus size accounting `retained_cell_count` (49) and `retained_text_byte_count` (71).

**Inputs/outputs.** Depends on `numerics`, `layout_plans`, `compiler_storage` (header 19-24). Pure; returns `Option<u64>` (None on overflow).

**How it decides.** Struct fields and case variants are identified by **String** names (`type_name`, field name, `variant`), not by handle; comment 19-24 says "labels remain pure result descriptors that issue no nominal identity". Cell counting: every value is 1 cell plus children (68). `LayoutTarget` counts 0 children cells and `facts.profile.len()` text bytes (74).

**Cases by shape.** None.

**Same job elsewhere.** Retained-size accounting by cells/bytes also exists in `evaluation-accounting` (charges are passed these numbers, see section 8). The checked interpreter evaluation.rs uses these names (grep below).

**Unused / one-caller.** `retained_cell_count|retained_text_byte_count|charge_pure_usage|reclaim_empty_pages` count grep gave: build-time-values lib.rs 8 (def+recursion), checked-interpreter evaluation... see: `psi/interpreter/checked-interpreter/src/interpreter.rs` 2, `build-settings/src/calling_policy_execution.rs` 2, `build_evaluation.rs` 1 (combined pattern; cannot split per name).

**Stated limits.** `Float` and `UndeterminedNaN` comment 35: "Evaluation may transport this meaning, but cannot publish its bits."

**Fail-open risks.** None seen (None on overflow is fail-closed for callers that unwrap to a refusal; not traced).

---

## 3. compiler-storage/derive/src/lib.rs (144 lines)

**Purpose.** `#[derive(Record)]` proc-macro (29). Structs: fields in declaration order; enums: `u32` tag = variant declaration index then fields; `#[record(skip)]` fields omitted on write and `Default::default()` on read (38-53, 55-84). Generic type params get a `Record` bound (96-102); unions rejected (131-136). Every write/read is wrapped in `output.record(..)`/`input.record(..)` (140-141) so nesting depth is bounded.

**How it decides.** Variant tags are positional (118): reordering enum variants changes the wire format (comment 10-17 says records are produced/read by the same binary). Unknown tag on read -> error "unknown <Name> variant" (128). Only `skip` is accepted inside `record(...)` (47).

**Fail-open risks.** `skip` fields are rebuilt from `Default` on read (69-70): a skipped field that is semantic would silently reset. Comment 47 says skip is "for derived, nonsemantic state". Status: stated.

---

## 4. compiler-storage/src/lib.rs (672 lines; tests from 426)

**Purpose.** Bounded, versionless binary graph storage: trait `Record {write, read}` (37), `encode`/`decode` (42/48), `RecordLimits` (56: bytes 64 MiB, allocation 512 MiB, elements 4 Mi, depth 128), `RecordWriter` (92), `RecordReader` (194), `RecordError{position, reason}` (75).

**Shape.** Writer: `extend` 111 (byte limit), `sequence` 124 (element budget + u64 LE length), `record` 149 (depth), `shared_identity` 162 (map `(TypeId, pointer addr, length) -> identity`, charges one element on first sight; this is the Arc DAG identity). Reader: `take` 229, `allocate` 242, `sequence<T>` 250 (charges elements and `len*size_of::<T>()` allocation before any Vec is built), `begin_shared` 295 (identities must be consecutive 1..), `finish_shared` 308, `shared` 325 (returns clone if present and of the right type, else error "forward, cyclic, or has another type"), `remap_generations` 338 / `define_generation` 352 / `generation` 402 (per-domain `TypeId` keyed maps of original->restored generation with payload equality check).

**Inputs/outputs.** `decode` reads bytes, builds the value, then `finish()` rejects trailing bytes (51, 280-286). `RecordReader::<()>` with `Context` default.

**How it decides.** Shared objects keyed by raw pointer address + length + TypeId at write time (167); at read time by sequence number. `define_generation` (352-400): same `(domain TypeId, original generation)` must carry an equal payload (`downcast_ref::<Payload>() != Some(payload)` -> error 368-370).

**Fail-open risks.**
- `RecordReader::generation::<T>(original)` returns `Ok(original)` unchanged when no remapping domain is registered for `T` (402-408). Wrong input reaching it: a decode that forgot `remap_generations::<T>()` silently keeps un-remapped generations rather than erroring. Status: "traced" in test only (595-597: `Semantic` domain intentionally passes through; a registered domain errors for unknown generations 597, 670). Whether every real decode registers its domains: open (not traced; callers are outside the unit).
- Writer `record()` decrements depth, runs, then increments even on error (153-159): fine.
- Decoded Arc values are cloned out of `shared_values` boxes (`finish_shared` 321: stores `value.clone()`), so shared `Arc` identity is by `Arc::clone`, not by content.

**Stated limits.** Header 17-22: "Decoding produces structural candidates only; source, checking, and realization owners must still bind the exact producer and input digests" - i.e. the format has no version/digest of its own (read: no magic/version bytes anywhere in `encode`, 42-46).

**Unused/one-caller.** `remap_generations`, `define_generation`, `generation` - not grepped across callers (see "Not grepped" at end).

---

## 5. compiler-storage/src/values.rs (540 lines)

**Purpose.** `Record` impls for primitives, `usize/isize` (as u64/i64, 30-51), `bool` (strict 0/1, 58-62), `char`, `()`, `PhantomData`, `Option`, `Result`, `Vec`, `[T; N]` (length check 166), `String`/`Cow<str>`/`Box<str>` (UTF-8 validated on read 186), `Box<T>`, `Box<[T]>`, `Arc<T>`, `Arc<str>`, `Arc<[T]>`, tuples up to 8 (310-325), `BTreeMap/BTreeSet/HashMap/HashSet` (duplicate key/entry rejected 342, 362, 385, 405), `Range`, `Duration` (nanos < 1e9 checked 431), `OsString` (Unix tag 0 / Windows tag 1, cross-host mismatch refused 464, 472), `PathBuf`, `Cow<[T]>`, all `NonZero*` (zero refused 514), `Mutex<T>` (write fails if poisoned 533-534; read makes a fresh Mutex 538).

**How it decides.** Arc forms write `(identity u64, first bool, payload-if-first)`; read: `first == false` -> `input.shared::<Self>(identity)`; `true` -> `begin_shared`, read payload, `finish_shared` (239-250, 264-273, 292-307). Allocation is charged before reading (e.g. 246, 271, 299).

**Fail-open risks.** `HashMap`/`HashSet` writes iterate in hash order (373, 396): encoding is not canonical for hash containers (read: no sorting). Not an accept/keep risk but affects byte-stability; status: open. Fixed-width integers are written via `to_le_bytes` for f32/f64 too (28), NaN payload bits preserved (test 535-546).

---

## 6. diagnostics/src/lib.rs (444 lines)

**Purpose.** `Diagnostic{severity, message, exceeded_provision, source_span, resolved_location}` (94), severity `Error|Warning` (166), `ResolvedSourceLocation{package, path, line, column}` (103), `ExceededProvision{resource: &'static str, limit, requested, coordinate}` (111), `CoverageForm` (119) with the finite resource vocabulary `coverage_machine_headers`... (130-153), renderers `write_diagnostics` (311), `write_bounded_diagnostics` (249, max 16 diagnostics, 4096 message bytes, escaped via `source::display_literal_bytes`, 34-35), `write_location` (282), joiners `comma_join_display` 340, `bracketed_display_names` 351, `join_messages` 362, helper `error_in_context` 83, trait `PhaseSnapshot` (87), macros `debug_display!`, `diagnostic_display!`, `string_diagnostic!`, `failed_display!` (37-81).

**How it decides.** `ExceededProvision::read` (227-246) decodes `resource` string by `CoverageForm::from_resource` (a string compare against the seven names, 130-141) and then demands `limit == 0 && requested == 1` (via `form.exceeded`, 155-162, 239) - i.e. only coverage-form provisions can round-trip; any other resource string is rejected (235-237). `Diagnostic::is_error` keys on severity (201). `write_location` prefers `resolved_location` over `source_span` (283, 300).

**Cases by shape.** Seven `CoverageForm` variants only (119-127).

**Unused/one-caller (grep, count mode, all of omega-rust).**
- macros `debug_display!|diagnostic_display!|string_diagnostic!|failed_display!`: 92 lines across 62 files (used widely in `psi/compilation/proof/admission`, `omega/compilation/pipeline/*`, `backend/*`).
- `PhaseSnapshot`: 6 lines: diagnostics lib.rs 2, `typed-trees/src/lib.rs` 2, `syntax-trees/src/inspection.rs` 2 (so only two implementing/using crates).
- `write_bounded_diagnostics|error_in_context|bracketed_display_names`: 28 lines across 9 files (diagnostics lib.rs 9, typed-trees values.rs 2, syntax-trees values_and_types.rs 2, symbol-resolved-trees values.rs 2, package-manager candidate error.rs 2, build_execution source_resolution.rs 1, retained_terminal.rs 2, native-compiler request.rs 2, provider-admission source_evaluated_bindings.rs 6). Combined pattern; cannot split per name.

**Stated limits.** 168 comment: Warning is "Surfaced to the user without failing the build (Decision-12 relaxation for non-proof deadness)".

**Fail-open risks.** `ExceededProvision::read` refuses unknown resources (fail-closed). `write_bounded_diagnostics` truncates silently to 16 diagnostics but prints "[truncated; N additional diagnostics omitted]" (272-278). None seen.

---

## 7. evaluation-accounting/src/invocation.rs (538 lines)

**Purpose.** Per-invocation resource account over a shared aggregate sponsor. Every two-account transaction locks sponsor then invocation (consumption 175-195; comment 1-8) and a failed charge leaves both unchanged (checks computed first, stores after: e.g. 207-225, 291-342).

**Shape.** `EvaluationMeasurementIdentity{CheckedTree, TerminalPsi{work_schedule, storage_schedule}}` (83) with `is_current` (92: markers must be 1) and `is_supported(schema, schedule)` (102: CheckedTree = schema 7/schedule 1; TerminalPsi = schema 8/work marker 1/storage marker 1). `TerminalStorageScheduleIdentity` (66, CURRENT = 1, zero refused 70). `EvaluationAccountingError` (24). `BuildEvaluationSponsor::begin_invocation` (149: refuses work budget 0, non-current measurement, measurement != sponsor's). `EvaluationInvocationAccount::{charge_work 207, charge_build_log_bytes 227, charge_filesystem_operation_attempt 245, reserve_storage 269, reserve_site 277, increase_site 291, release_storage 344, charge_result_custody 369, snapshot 408, owns_usage 451}`. `EvaluationStorageLease` (469): `absorb 476, truncate_cells 493, grow 499, shrink 519`, `Drop` releases (534-538).

**How it decides.** Budget checks by numeric comparison against limits (e.g. 215-217: aggregate > sponsor work_budget OR invocation > invocation budget). Identity checks by `Arc::ptr_eq` on the account (`same_invocation` 199, `owns_usage` 145/451). Per-resource asymmetries (read): build-log (235), filesystem attempts (253-259) and result custody (394-396) compare only the **aggregate** to the session ceiling; the invocation counters are updated but not compared. Live storage (322-326) compares both aggregate and invocation to the same session maximum.

**Fail-open risks.**
- `release_storage` (344-367) does `debug_assert!` then unchecked subtraction; in release builds an over-release would wrap/panic (arithmetic overflow behaviour depends on profile), not silently accept. Status: open.
- `EvaluationStorageLease::truncate_cells` (493-497) releases `cells` only with `payload_bytes` 0; clamped by `min`. fine.
- `lock().map_err(AccountUnavailable)` on charges (184-193) vs `unwrap_or_else(into_inner)` on release (350-356): charges fail closed on poison, releases proceed.
- `is_current`/`is_supported` hard-code schedule markers 1 and schema versions 7/8 (98, 104, 109-112): a new schedule marker is refused as unsupported (fail-closed).

**Stated limits.** None beyond header.

---

## 8. evaluation-accounting/src/lib.rs (757 lines)

**Purpose.** Aggregate sponsor `BuildEvaluationSponsor` (160) with limits `BuildEvaluationSponsorLimits` (39; all eight limits must be nonzero, 61-71; schema constant 7 at 36), consumption struct (145), leases for live cells/text bytes/filesystem handles (536-584), and the usage record `BuildEvaluationUsage` (587, 22 fields) with `validate` (643) and error enum `BuildEvaluationUsageError` (614).

**How it decides.** Legacy sponsor-level charges (`charge_work_unit` 257, `reserve_live_cell` 359, `charge_result_custody` 328, `increase_live_payload_bytes` 393) call `require_checked_tree` (195) and refuse when the sponsor's measurement is not `CheckedTree`; `charge_build_log_bytes` 272, `charge_filesystem_operation_attempt` 289 and `reserve_live_filesystem_handle` 304 do not call it. `charge_pure_usage` (431-514) validates the usage, requires `usage.measurement == self.measurement()` (438), requires `filesystem_operation_attempts == 0` (441), then adds work, log, result cells/bytes and **peak** live cells/bytes (not current) to the aggregate via `charged_total` (517) and republishes session peaks. `validate` (643-756): measurement supported, nonzero invocation budget, `work_units <= invocation_work_budget`, sponsor-field tuple is all-None (and the three session peaks must then be 0) or all-Some with every ceiling respected; any other mix is `IncompleteSponsorIdentity` (751).

**Cases by shape.** The nine-tuple `match` in `validate` (662-753) enumerates one arm per "which ceiling is zero" (681-704) before the general arm.

**Same job elsewhere.** `invocation.rs` charge paths and `lib.rs` sponsor paths both implement counter+ceiling checks (invocation.rs:207-267 vs lib.rs:257-302): the sponsor-level version uses `String` errors and `>=`/`checked_add`, the invocation version uses `EvaluationAccountingError`. Alias getters `maximum_live_cells/maximum_live_text_bytes/maximum_result_cells/maximum_result_text_bytes` (120-134) and `peak_live_cells/peak_live_text_bytes` (249-255) duplicate the `*_storage_cells/*_payload_bytes` names.

**Unused/one-caller (grep, count mode).** The six alias names `maximum_live_cells|maximum_live_text_bytes|maximum_result_cells|maximum_result_text_bytes|peak_live_cells|peak_live_text_bytes`: 73 lines across 14 files (e.g. checked-interpreter evaluation.rs 20, provider_plan/evaluated_bindings.rs 12, review-evidence restricted_build.rs 8), so the aliases are used.

**Fail-open risks.**
- `charge_pure_usage` charges `peak_live_*` of the replayed evaluation against the *current* live counters (476-489) but writes back only session peaks (511-512), not live counters: by design (tests 32-53).
- `BuildEvaluationUsage::validate` with all sponsor fields `None` accepts any invocation-level numbers (673-680) as long as the session peaks are 0: this is the "standalone compilation" case (comment 592).
- `reserve_live_filesystem_handle` and the two filesystem/build-log sponsor charges work for either measurement (no `require_checked_tree`): open (not traced).

---

## 9. evaluation-accounting/tests/accounts.rs (257 lines) and tests/cached_usage.rs (144 lines)

**Purpose.** Source-free counter tests (accounts.rs:1-2 says "no test here claims that an interpreter charges its actual storage sites"). Cover: engine-substitution refusal without charge (27-50), rejected combined `reserve_site` atomic (53-67), two invocations contending for one allowance via barriers (70-102), Arc-aliased lease release (105-116), grow/shrink atomicity (119-133), lease `absorb`/`truncate_cells` (136-155), sibling-lease mismatch (158-176), `owns_usage` ptr identity (179-190), result custody not refunded on lease drop (193-207), host charges (210-222), unsupported storage schedule 2 (225-245), `u64::MAX` overflow preservation (248-257). cached_usage.rs: replay of recorded Pure usage (32-53), every refused axis leaves counters unchanged (55-75), invalid/effectful/other-measurement usage cannot spend (77-94), live reservations counted (96-105), overflow (107-121), concurrent replay (123-144).

**Notes.** Test at accounts.rs:42-45 shows a Terminal sponsor refuses legacy `charge_work_unit`/`reserve_live_cell`/`reserve_live_text_bytes`/`charge_result_custody`. No test in these files asserts the invocation-level (non-aggregate) build-log or filesystem ceiling (read; the limits fixture `limits()` 11-13 sets log 8 and attempts 2 identically for aggregate).

---

## 10. extents/src/extent.rs (459 lines)

**Purpose.** The linear `Extent` carrier (69): `{base, length, address_space, rights, provenance, era, origin: ExtentRootOrigin, lineage: Lineage{root, path: Vec<SplitBranch>}, sharing: ExtentSharingMode}`; operations return the unconsumed carrier inside an error on failure (comment 1-6): `split_at` 130, `partition_owned` 184 (before/selected/after), `attenuate` 236, `merge` 246, `loan` 278, `loan_mut` 282; `OwnedExtentPartition::rejoin` 320.

**How it decides.** Merge (validate_merge 336-394) keys on **identity fields**: equal address_space, rights, provenance, era, sharing (337-341); equal `lineage.root` (347); equal `origin` (352); paths must be non-root siblings (same parent path, different last branch, 357-367); geometry must abut with the Lower child first (373-388). Attenuation: `rights.contains(&rights)` (237). `loan_mut` refuses `PeerShared` (287-291). `split_at` requires 0 < lower < length (131).

**Cases by shape.** `partition_owned` handles four (offset==0?, end==length?) layouts (213-228).

**Fail-open risks.** `Extent::end()` uses unchecked `self.base + self.length` (91); construction validates `base.checked_add(length)` (`validate_range` 9-17, used elsewhere) - status: traced only to the validator in this file. `merge` result length `lower.length + upper.length` (264) is guarded by the checked add at 389-392. `OwnedExtentPartition::rejoin` uses `.expect` (325, 330) (panic, not accept).

---

## 11. extents/src/lib.rs (1334 lines; tests 909-1334)

**Purpose.** Crate root of the extent (address-range authority) calculus. Holds, inline, the modules `identities` (39-154), `loans` (155-305), `roots::{root_grants, root_origins}` (307-857), re-exports (858-907), and tests (909-1333). Comments 1-28: Extents are linear capability carriers; "Fresh root extents enter solely through admitted provider issuances or program-local origins".

**Shape.**
- `normalized_extent_identity!` macro (44-68): newtype over `u64`, `from_normalized_identity` refuses 0 (58-60), `Record` read refuses 0 (49-55). 21 ids instantiated at 71-91, 121 (e.g. `AddressSpaceId`, `ExtentLineageId`, `MappingEraId`, `ExtentRightId`, `ResidentClaimId`).
- `NormalizedIdentitySet<Id>` (123) over `BTreeSet`; `contains(required)` = `required` is a subset (144-146); `ExtentRights = NormalizedIdentitySet<ExtentRightId>` (153).
- `ExtentContentInterpretation{compatibility_fingerprint, commitment: [u8;32]}` (96).
- `ExtentLoan<'a>` (176) over `LoanBacking::{Shared(&Extent), Exclusive(&mut Extent)}` (171); `validate_subrange` (286-304) checks `offset+length` overflow and `end <= extent.length` but not `length > 0` (zero-length loans allowed; test 1195-1211 asserts this).
- `ExtentRootGrant` (321: origin, lineage, address_space, rights, provenance, era, sharing). `from_admitted_provider` 414 and `from_established_program_local` 439 (marked `#[doc(hidden)]`, `pub const`) construct grants; `mint` 466, `mint_validated` 523, `mint_provider_existing_content` 479 (refuses non-provider origin 498-506), `admitting_peer_shared_backing` 433 (sets `PeerShared`). Errors hand the grant back (550-580).
- `ExtentProviderIssuance` (593: 8 ids + `ExtentProviderInvocation` 5 ids); `from_normalized_identities([u64;13])` 653 (every id nonzero via the id constructors). `ExtentProgramLocalOrigin` (753: 8 raw `u64` fields); `from_normalized_identities([u64;8])` 765 checks nonzero for each by loop with labels (778-789). `ExtentRootOrigin{ProgramLocal, ProviderIssued}` (836).

**How it decides.** Origin classification by enum variant (836-855). Existing-content minting is permitted only for `ProviderIssued` (498). Everything is keyed on newtype identity values; there is no check inside the crate that an `ExtentProviderIssuance` was actually admitted - `from_admitted_provider` is a public const constructor taking ids (691-713) and the "admitted" part is by name only (read; stated in header 3-5 only as intent).

**Cases by shape.** Provider issuance is a fixed array of 13 identities and program-local origin a fixed array of 8 (654, 766).

**Same job elsewhere.** `validate_range` (extent.rs:9) is reached through `ValidatedExtentGeometry::check` (399) and also directly by `OrderingEvent::from_admitted_provider` (loans_and_claims.rs:1273). `ExtentLoan` accessors (209-272) each match on both backings.

**Unused/one-caller (grep, count mode).** `use extents|extents::`: 76 lines across 18 files outside this crate (access-plans 52, terminal-interpreter 5, checks/validation/typed-tree steps, omega/terminal-runtime `allocation/backing.rs` 1, executable-installation 6, compiler tests 1). `from_established_program_local|ExtentRootGrant::|from_normalized_identities([`: 24 lines (access-plans tests 3, extents 15, omega terminal-runtime `backing.rs` 3, compiler tests `terminal_extent_view.rs` 3).

**Fail-open risks.**
- `ExtentRootGrant::from_established_program_local` and `ExtentProviderIssuance::from_admitted_provider` are public const constructors with no proof argument: any crate that depends on `extents` can mint authority by calling them (read 414-456, 691-713). The crate comment says roots enter "solely" through admitted issuances; enforcement is by caller convention (the doc(hidden) attribute at 438). Status: "open" (callers outside unit not traced beyond the counts above).
- Zero-length loans are accepted (286-304; test 1195-1211). A zero-length external-loan range later has to match exactly. Not a fail-open for authority conservation (length 0 carries none), but `ExtentRootGrant::mint` refuses length 0 (`validate_range` extent.rs:10).

---

## 12. extents/src/loans_and_claims.rs (1635 lines)

**Purpose.** Three sibling modules (header 1-6): `activation_claims` (8-613), `external_loans` (615-1186), `ordering_events` (1188-1635).

### 12a. activation_claims (8-613; tests 9-219)
**Shape.** `ActivationClaimLedger{next_identity, live: Vec<ActivationClaim>}` (398) a LIFO stack of claims; `establish` 426, `release` 460 (only the newest claim may be released, 464-476), `split_claim` 479, `merge_claims` 535 (only the two newest live claims, 542), `drain(self)` 587 (error if any live claim), `composed_demand` 601. `ActivationClaimBranch(Vec<u32>)` (239) with `covers` (prefix test 252) and `mutually_exclusive` (256). `compose_claim_bounds` (285): each row's bound is aligned up (`align_up` 273, power-of-two alignment required), then for each row sums the contributions of all rows whose branch `covers` that row's branch and takes the maximum (299-311). It is quadratic in row count (nested loop 300-308, read). A `None` bound row is refused ("leaves the activation's stack demand open", 291-296). `validate_committed_within_bound` (323) is `committed <= bound`.
**How it decides.** Identity = `ActivationClaimId(u64)` counter (`push_established` 605); lineage = `Lineage{root: ExtentLineageId, path}` reused from extent.rs; merge requires same lineage root, Lower/Upper siblings with equal parent, same `site` and `branch` (555-568). Split requires 0 < committed_at < committed, 0 < bound_at < bound, committed_at <= bound_at and the remainder also within bound (487-500).
**Fail-open risks.** `merge_claims` sums `lower.bound + upper.bound` and `committed` with plain `+` (570-571), no overflow check; they came from a split of one claim so the sum is the original parent's, so overflow is unreachable via ledger APIs (inferred; traced to `split_claim` 520-521 which subtracts). `ActivationClaimProvenance` has a single variant `ActivationStorage` (262-264).
**Callers.** `ActivationClaimLedger|compose_claim_bounds|validate_committed_within_bound`: 27 lines in 4 files: loans_and_claims.rs 20, lib.rs 3, `terminal-interpreter/src/storage_primitives.rs` 3, `terminal-psi/semantics/src/storage.rs` 1 (grepped).

### 12b. external_loans (615-1186; tests 983-1185)
**Shape.** `ExternalLoanGrant` (686: borrower, direction DeviceReads|DeviceWrites, address_space, provenance, required_rights, completion: set of fact ids), `begin_external_loan(loan, identity, grant, reach_receipt) -> ExternalLoan` (805), `ExternalLoan::complete(receipt) -> ExternalLoanCompletion` (755). Receipts `ExternalReachReceipt` (640) and `ExternalCompletionReceipt` (894) are constructed by `from_admitted_provider` from the loan itself plus a caller-supplied `confined_to_range: bool` (663) / `borrow_released: bool` (913).
**How it decides.** Start (811-827): address_space equality, provenance equality, `loan.rights().contains(required_rights)`, DeviceReads needs a Shared loan, DeviceWrites needs an Exclusive loan. Reach (837-872): receipt must be present, name same loan identity, borrower, direction, address space, provenance, era, lineage, rights (exact equality `!=`, 862), exact base/length, and `confined_to_range == true`. Completion (759-785): same field-by-field equality chain plus `borrow_released` and `established_facts.contains(&self.completion)`. All are chains of `if/else if` over identity values; first mismatch message wins.
**Cases by shape.** Two directions, two reach mechanisms (`AdmittedBorrowerContract`, `HardwareIsolation`, 634-637); the mechanism value is recorded but not consulted in any check in this file (read, 888 stores it; 837-872 never matches on it).
**Fail-open risks.** `confined_to_range` and `borrow_released` are plain booleans supplied by whoever calls `from_admitted_provider`: a receipt with `true` is accepted without evidence in this crate (read 663, 913, 868, 779). Status: "stated" only by the name `from_admitted_provider`. No users outside this crate (grep: `begin_external_loan|ExternalLoanGrant|ExternalReachReceipt` = 34 lines in loans_and_claims.rs and 2 in lib.rs, none elsewhere).

### 12c. ordering_events (1188-1635; tests 1451-1634)
**Shape.** `OrderingEvent` (1251: identity, role `DeviceOrderingRole` {Publication, CacheMaintenance, Notification, Completion, Acquisition}, device, scope, request: ExternalLoanId, authority: `OrderingAuthority{address_space, provenance, era, lineage}`, `coordinates: BTreeMap<OrderingCoordinateKind, OrderingRange{mapping, base, length}>`). `from_admitted_provider` (1262) refuses a duplicate coordinate kind (1274), an invalid range (`validate_range`, 1273) and an empty coordinate set (1281). `admit_role_coverage(requirement, events)` (1371): every event must match the requirement's device, scope, request, authority; its role must be in `requirement.roles`; roles must not repeat; then `covered == requirement.roles` (1412). `admit_acquisition_after_completion` (1429): roles must be Acquisition/Completion and request, device, scope, authority equal.
**How it decides.** By identity equality only. The `coordinates` (mapping/base/length) are never compared in either admission function (read 1371-1449); `same_payload_different_role` (1325) compares them but its callers are only the test at 1506 (grep: no use outside this file).
**Fail-open risks.** `admit_role_coverage` with an empty `requirement.roles` and an empty event list returns `Ok` (covered == roles == empty, 1412) - vacuous admission; open (callers outside crate: none, grep below). `admit_acquisition_after_completion` does not check that the completion occurred before acquisition in any order sense (no sequence value exists); name says "after" but only identity matches are verified (read 1433-1448).
**Callers.** `admit_role_coverage|admit_acquisition_after_completion|OrderingCoverageRequirement|OrderingEvent`: 40 lines in 2 files (loans_and_claims.rs 37, lib.rs 3): zero users outside the extents crate (grepped).

---

## 13. extents/src/mapping.rs (1487 lines; tests 911-1487)

**Purpose.** Two-phase address translation: a `MappingGrant` (35) authorises mapping a source (owned `Extent` or borrowed `ExtentLoan`) onto a destination `Extent`; `PendingMap` -> receipt -> `MappedExtent` -> `PendingUnmap` -> receipt -> `UnmappedExtents`, with an optional peer-write-revocation step for shared-borrowed sources.

**Shape.** `MappingGrant{identity, source_mode (Owned|BorrowedShared|BorrowedExclusive), source_space, destination_space, required_source_rights, required_destination_rights, mapped_rights, mapped_provenance, mapped_era, map_obligations, unmap_obligations, peer_revocation_obligations}` (35-48); `map_owned` 645, `map_borrowed` 681, shared `map_with_source` 708; `MappingEvidence` (181: grant + source/mapped facts + destination restoration); `MappingTransitionReceipt<FactId>{mapping: MappingEvidence, established: bool, established_facts}` (222) with `verify` (241) and three typedefs 297-301; `MappedExtent` (310) with `loan` 508, `loan_mut` 512, `stable_loan` 525, `begin_peer_write_revocation` 544, `begin_unmap` 559, `range_receipt_context` 477.

**Inputs/outputs.** Consumes `Extent`/`ExtentLoan` by value; every failure returns the inputs in an error (e.g. 652-659, 733-739). Output: `PendingMap` whose `mapped` extent is the destination with `rights/provenance/era` replaced by the grant's `mapped_*` values (746-751); `UnmapCompletionError`/`UnmappedExtents` restore the destination's saved rights/provenance/era (865-870).

**How it decides.** `map_with_source` (714-731): source mode == grant mode; source space == grant.source_space; destination space == grant.destination_space; `source.rights().contains(required_source_rights)`; same for destination; `source.length() == destination.length()` (equal length only; base alignment is not checked). Receipt verification (247-264): receipt's whole `MappingEvidence` must equal the pending one (`&self.mapping != mapping`, 247), `established == true`, and `established_facts` must be a superset of the grant's obligations. `loan_mut` refused for `BorrowedShared` (517); `stable_loan` refused for `BorrowedShared` until `peer_write_revoked` (530-536); peer-write revocation allowed only for `BorrowedShared` (548).

**Cases by shape.** Three source modes (21-25). Three wording constants (276-295) fix error text per transition. In `unmap`, a borrowed source is dropped (`MappingSource::Borrowed(_loan) => None`, 873) - the loan is released, an owned source is returned.

**Same job elsewhere.** The receipt/obligation pattern (receipt must name the exact evidence, a bool "established", and a fact set that is a superset of the grant's) is also implemented in `external_loans` (loans_and_claims.rs:755-802): there with a hand-written `if/else if` chain over field equality, here with whole-struct equality (`verify`, 241-266). Duplicated pattern in the same crate.

**Unused/one-caller.** `MappedRangeReceiptContext|range_receipt_context`: 26 lines: mapping.rs 12, lib.rs 1, `access-plans/src/resources.rs` 10, `access-plans/src/tests.rs` 3 (grepped). `map_borrowed|map_owned|PendingMap|MappingGrant`: 54 lines: mapping.rs 45, lib.rs 3, `access-plans/src/tests.rs` 6; so outside the crate only a test file uses mapping entry points (grepped).

**Stated limits.** None.

**Fail-open risks.**
- `MappingGrant::from_admitted_provider` (52-80) validates nothing: empty `required_*_rights`, empty obligation sets and any `mapped_rights` (even wider than the destination's) are accepted; mapped rights are taken from the grant, not intersected with source/destination rights (746-751). Wrong program: a grant that widens rights to the mapped view passes. Status: "open" (no comment; grant is "admitted" elsewhere).
- Empty obligation sets make `established_facts.contains(obligations)` vacuously true (257), so only `established == true` is demanded. Status: open.
- `MappedExtent::eq` compares only `evidence` (317-321).
- `map_with_source` does not check destination `sharing` mode and keeps it via `..destination` (746-751).

---

## 14. filesystem-authority/src/filesystem.rs (1049 lines)

**Purpose.** Plain data and codecs for filesystem evidence: a canonical source-tree metadata index (rows of relative path -> Directory/File/Symlink), grant roots and refusal records, logical handle identities, a host `stat` record layout description, and byte-level encode/recover functions for retained observations (header 1-7).

**Shape.**
- `filesystem_record!` macro (9-33): generates a Copy struct with `new` and by-value getters.
- `CanonicalFilesystemMetadataIndex{policy_version, source_content_commitment: [u8;32], rows: BTreeMap<Vec<u8>, RowKind>}` (185); `::new` (203-281) enforces: policy version == 1 (208), row count < 262_145 (216), each path canonical (222), aggregate path bytes <= 16 MiB (227-235), logical length <= i64::MAX (236), no duplicate paths (243-250), root row `""` present and a Directory (252-256), every non-root path's parent row exists and is a Directory (257-275). Error enum 116-127.
- `canonical_filesystem_metadata_path_is_canonical(relative, allow_empty)` (44): length cap, no NUL, no backslash, not starting `/`, no empty/`.`/`..` component; does NOT require UTF-8.
- `filesystem_root_relative_path_is_canonical` (338): same plus requires UTF-8 (341) and refuses `X:` (a colon at byte 1, 350).
- `FilesystemGrantRoot{identity: nonzero u32, path: PathBuf, canonical_metadata: Option<Index>}` (370); `FsGrants{read_roots, write_roots}` (447); `FilesystemAuthorizedPath` (407); `FilesystemGrantRefusal{operand_ordinal, access, reason}` (326) with reasons Unresolvable/OutsideGrantedRoots/UnrepresentableRootedPath/ObservationEvidenceLimitExceeded (317-322).
- Logical handle types (453-515): identity nonzero u64 (constructor `#[doc(hidden)]`, 456), kinds Descriptor/Native/Find, input resolution Resolved/Null/Unknown, output source Created/Duplicated/Borrowed.
- `FilesystemMetadataField` (518, 14 fields; `ALL` 536; `semantic_width_bits` 553: Mode/User/Group are 32, rest 64; `is_signed` 570: 4 time fields + Size). `FilesystemMetadataLayout::new` (598-656): record_size > 0, fields sorted and exactly equal to `ALL`, stored width in {16,32,64} and <= semantic width, `offset+width <= record_size` (checked add), no pairwise overlap (640-651). `host()` (670-757): one hard-coded offset table per host via `cfg!`: linux x86_64 (record 144), linux aarch64 (128), macos (any arch, 144), windows x86_64 (144); else `panic!("unsupported host filesystem metadata layout")` (740). `Default` = `host()` (760-764).
- `observation_codec` (799-1049): `ObservationCursor` (808) with checked reads returning `Option`; tag<->enum pairs for provider (873-890: Virtual 0, RealUnscoped 1, RealScoped 2, Captured 3), operation result (892-915), handle kind, handle input (934-965), handle output (967-1003), grant access, grant refusal (1020-1048). Unknown tags and zero identities return `None` (e.g. 911, 954, 987).

**Inputs/outputs.** Pure data; the codec reads/writes `Vec<u8>` through `crate::codec_bytes::push_encoded` (re-export of `optimization::codec_bytes`, lib.rs:33).

**How it decides.** Path validity by byte-level component scan (44-60, 338-357); metadata layout validity by field identity/width/offset overlap; codec validity by closed tag matches. Keys are bytes and enum tags, not display names.

**Cases by shape.** `host()` handles exactly four cfg combinations and panics otherwise (671-741) - "else { panic!("unsupported host filesystem metadata layout") }". The layout chosen is the compiler's host, not the Omega target (read; `cfg!` evaluates at the compiler's own build).

**Same job elsewhere.** Two canonical-path predicates in this file (44 vs 338) differ in UTF-8 and drive-letter rules; grep of their names across `omega-rust`: 21 lines in 9 files (filesystem.rs 3, `package-compilation/.../source_snapshot.rs` 4, `build_record_codec.rs` 5, `checked_source_cache.rs` 2, `snapshot_request.rs` 1, `filesystem_scope.rs` 1, `filesystem_scope/runtime.rs` 2, `build-output/.../captured_source/mod.rs` 2, `staged_output_tree.rs` 1). Which callers use which was not split (combined count).

**Unused/one-caller (grep).** `FilesystemMetadataLayout`: 14 lines in 6 files (checked-interpreter interpreter.rs 4, execution.rs 2, `build_admission.rs` 2, filesystem.rs 3, build-time-evaluation machine_execution.rs 2, lib.rs 1).

**Stated limits.** Header 3-6 says decoded records "re-validate nonzero handle identities and closed enum tags so corrupt package-review bytes fail decode".

**Fail-open risks.**
- `FilesystemMetadataLayout::host()` describes the host OS's stat record regardless of build target (cfg! at 671-741). A cross-target evaluation would use host offsets. Status: "open" (callers not traced; whether the evaluator uses a layout per target is outside the unit).
- `canonical_filesystem_metadata_path_is_canonical` accepts non-UTF-8 bytes (44-60) while the root-relative predicate rejects them (341); a metadata row with a non-UTF-8 path is accepted by the index and may not be reachable by a root-relative path. Status: open.
- `observation_codec::ObservationCursor::framed` casts `u64 as usize` (856) then `take` checks bounds; a huge length simply returns `None`.
- `FilesystemGrantRoot::canonical_metadata` is `Option`: absence means no index, interpreted by callers (not here).

---

## 15. filesystem-authority/src/filesystem_sponsor.rs (800 lines)

**Purpose.** Resource accounting plus private-directory custody for append-and-seal staging sessions (header 1-7). `FilesystemSponsor` (320) holds `Arc<Mutex<FilesystemAccount>>` and optional `Arc<PrivateStaging>`.

**Shape.**
- Limits (`FilesystemSponsorLimits` 22): default 4,096 entries, 256 MiB total logical bytes, 256 MiB max object extent (16-18, 29-33).
- `FilesystemAccount` (68): `id` from a process-global `NEXT_ACCOUNT_ID` counter (19, 343-347), `session_root`, `entries`, `logical_bytes`, `writers: Vec<Option<StagingWriter>>` (starts with one `None`, 356), `prepared_transaction: Option<u64>` (at most one outstanding), `next_transaction_id`.
- Two-phase operations: `prepare_create_directory` 477, `prepare_create_object_open` 487, `prepare_close` 524, `prepare_write` 541 return `Prepared*` handles; `commit`/`commit_written` apply the change (210-269), `abort` is a no-op (190, 199, 206) and `Drop` clears the reservation (272-280). `admit_growth` (86-113) checks entries, bytes and object extent against the limits.
- `bind_path` (435): normalize absolute path, strip the session root prefix, refuse the root itself. `host_path` (606-636): walks components with `symlink_metadata`, refusing non-directory/symlink parents and non-`Normal` components. `entry` (453) classifies dir/file/symlink via `symlink_metadata`. `require_sealed` (507) refuses a path that has a live writer.
- `create_private` (363-405): canonicalize parent, `DirBuilder` mode 0700 on Unix, verify canonical == root and parent unchanged, then build a sponsor owning the directory; `PrivateStaging::dispose` removes the tree (295-310) and runs on `Drop` (313-317). `owns_private_staging_root` (407-418): true only if live and the root is strictly inside the private root.
- `normalize_absolute` (126-149): lexical `..` resolution via `PathBuf::pop`, error when popping past the root.

**How it decides.** Cross-account use is refused by comparing `account_id` (116-123, 574). Writes: `offset == extent` required ("non-append write" refused, 549-553). Growth: `committed actual > prepared` rejected (251). Path containment is lexical `strip_prefix` on the normalized absolute path (441-443), then component-wise `symlink_metadata` checks for each parent (626-633).

**Cases by shape.** Only append-and-seal: staging supports create dir, create-and-open object, append write, close; everything else is `UnsupportedStagingOperation` (549). Entry kinds: Directory, Symlink, Object (61-65); any other file type -> `EntryIsNotRegularObject` (469).

**Fail-open risks.**
- `PreparedFilesystemMutation::abort`, `PreparedFilesystemOpen::abort`, `PreparedFilesystemWrite::abort` are empty bodies (190, 199, 206); the reservation is cleared by `Drop` (272-280) which only clears when the id matches. Wrong flow: none seen.
- `entries` counter grows on commit but never shrinks (no delete operation exists); `CloseWriter` only takes the writer slot (240-247) and does not release `logical_bytes` (comment 74 on `writers` says "closing releases path and extent", but the file does not decrement `logical_bytes` on close, read 240-247). Status: the comment and behaviour differ for bytes; "open".
- Symlink/TOCTOU: `host_path` checks parents with `symlink_metadata` then the caller does the host mutation later; no atomicity (read; no comment). Status: open.
- `owns_private_staging_root` returns false if the mutex is poisoned (417): fail-closed.

**Unused/one-caller (grep, count mode).** `FilesystemSponsor\b|FilesystemSponsor::|prepare_close|prepare_write|prepare_create_object_open|dispose_private_staging|owns_private_staging_root`: 74 lines across 21 files (checked-interpreter `filesystem_real.rs` 9, `filesystem.rs` 3, typed-trees `callable_applications.rs` 7, `package-manager/src/build_session.rs` 6, `package-compilation/.../filesystem_scope.rs` 6, ... , sponsor file itself 10). Combined; not split.

---

## 16. filesystem-authority/src/lib.rs (266 lines)

**Purpose.** Crate root: re-exports (35-39), `FILESYSTEM_OPERATION_ATTEMPT_SCHEMA_VERSION = 20` (41), `open_flags` bit tables (43-88), the operation table macro `filesystem_operations!` (108-130) with 50 operations (132-183), build-output obligation/receipt records (191-266).

**Shape.** `FilesystemHostOperation` enum (`#[repr(u16)]`, tags 1..=50), each with canonical name, operand-kind list (`PathBytes, Bytes, I32, U32, I64, U64, MutableBytes, MutableI64`) and result kind (I32/I64) - the table is the single source (109-130). `from_canonical_name` is a string match over the names (116). `BuildOutputObligation{root, relative_path, issued_at, state: Pending|Completed{completed_at, receipt}|Failed{failed_at, diagnostic}, post_completion_mutations}` (207); `BuildOutputReceipt{obligation, root, relative_path, sealed_at, completed_at}` (238); `MAX_BUILD_OUTPUT_OBLIGATIONS = 4_096` (191).

**How it decides.** `open_flags` bit positions are chosen by `#[cfg(target_os = ...)]` of the compiler's host (windows / macos / everything else), with a comment (22-25) saying they "mirror the checked target encoders in std/targets/<target>/filesystem_impl.omg". `wants_write` (84-87): access bits (`flags & 0x3`) equal 1 or 2, or create/trunc/append bit set.

**Cases by shape.** Open-flag tables for three cfg groups (44-69); Windows `O_APPEND`=3, macOS `O_APPEND`=3, others 10 etc.

**Same job elsewhere.** Operation names/tags are also encoded by `observation_codec` for results and by the interpreter; `FilesystemHostOperation` users not grepped individually. `wants_write`/`open_flags::`: 37 lines in 4 files (checked-interpreter `filesystem_real.rs` 27, `filesystem.rs` 5, lib.rs 1, `package-compilation/.../filesystem_scope/runtime.rs` 4) (grepped). `BuildOutputObligation|BuildOutputReceipt|MAX_BUILD_OUTPUT_OBLIGATIONS`: 50 lines in 8 files (build_machine.rs 13, runtime.rs 13, output_custody.rs 6, ...).

**Stated limits.** Comment 22-25 mirrors "checked target encoders" - the mirror is by hand (read; no generated link).

**Fail-open risks.**
- `wants_write` treats access mode 3 (`flags & 0x3 == 3`) as read-only unless a create/trunc/append bit is set (84-87). On POSIX `O_ACCMODE` value 3 is invalid, but a caller classifying read vs write grants by this predicate would treat such flags as a read. Status: open (callers in `filesystem_real.rs` not traced).
- Open flag bit positions follow the compiler's host OS, not the evaluated target (44-69). Status: open.

---

## 17. flow-effects/src/lib.rs (578 lines)

**Purpose.** Data-only crate holding the four effect summaries the checker publishes and later stages read (header 1-22), all inside `pub mod flow_effects` (41-567): `capabilities` (42-149), `invocations` (151-191), `operational` (193-440), `service_reach` (442-550). Comment 32-37 says five former files were merged into this one.

**Shape.**
- `CapabilityFlowFact{kind: Uses|Returns|Acquires|Stores|Derives, capability_symbol, machine_symbol, state_symbol, statement_index, call_ordinal, via_state_symbol}` (68-77), `CapabilityFlowPlan{flows: Arena}` (80). `CapabilityFlowKind::as_str` (57).
- `InvocationTarget{Parameter(u32), Service(SymbolHandle)}` (155), `RealizedInvocationTarget{Requirement(Symbol), Abstract(InvocationTarget)}` (161), `MachineInvocationInference` (167; five vectors: published, inferred_direct, inferred_transitive, effective, realized), `InvocationInferencePlan{machines: Vec}` with `for_machine` = linear `find` on `summary.machine == machine` (182-189).
- `OperationalPlan` (204: `root_machines`, arenas of machines/states/calls/cleanup deps/static bindings, `captured_selector_calls`, `selected_call_contexts`, `machine_index: RevisionIndex<SymbolKeyMap<SymbolHandle, usize>>`). `with_machine_index` (228-243) rebuilds the symbol->index map when `self.machines.revision()` changes. `machine_by_symbol` 249. It implements `ProofApplicationCalls<SymbolHandle>` (254-308): `visit_calls` (265-289) returns `false` when the machine has no summary, else yields one `ProofApplicationCall` per call with `binder = static_machine_parameter if valid else target_state_symbol` (276-280); `visit_cleanup` (301-307) scans ALL `cleanup_dependencies` linearly per machine.
- `MachineOperational` (311: six suspend/block booleans: published, transitive, body), `StateOperational` (323), `CallOperational` (333: `target_name: String`, target state/machine/operator symbols, direct/transitive booleans, `acknowledgement`), `CleanupDependency` (350), `Captured*` (358-379), `SelectedCallContexts` (380) with `call(context, caller_state, statement_index, call_ordinal, target_state_symbol)` (410-430) = linear search keyed by those four values, returns `None` when the context handle is invalid.
- Service reach: `ServiceReachDependency`, `MachineServiceReachInference` (456: `published, inferred_direct, inferred_transitive, concrete_transitive, concrete_effective, effective` as `ServiceReachRowId` into one row table, plus `unresolved_installation_reaches: Vec<InstallationReachRequirement>`), per-state and per-call variants, `ServiceReachInferencePlan` (502: `rows: ServiceReachRowTable`, arenas). `for_machine` (516) linear find; `for_state` (532) linear scan over all states by `state` symbol.

**How it decides.** Lookups are by `SymbolHandle` equality (identity), except `CallOperational.target_name: String` (336) which stores a spelling beside the symbols.

**Cases by shape.** None (data definitions).

**Same job elsewhere.** `InvocationInferencePlan::for_machine` (182), `ServiceReachInferencePlan::for_machine` (516) and `OperationalPlan::machine_by_symbol` (249) are three per-machine lookups: the first two scan, the third uses a cached index.

**Unused/one-caller (grep, count mode).** `OperationalPlan\b`: 86 lines in 16 files (validation `machine_calls_core.rs` 25, `effect_inference.rs` 17, `calls.rs` 6, checked-trees `facts.rs` 5, step 05 `published_facts.rs` 4, `flow.rs` 4, `checking.rs` 4, ...). `InvocationInferencePlan|ServiceReachInferencePlan|CapabilityFlowPlan`: 91 lines in 19 files (validation traits.rs 11, machine_calls_core.rs 18, effect_inference.rs 10, review-evidence `service_reach.rs` 3, `reach_dependency.rs` 2, ...).

**Fail-open risks.** `ProofApplicationCalls::visit_calls` returns `false` (not an error) when the machine has no operational summary (270-272); what the trait consumer does with `false` is outside this unit (open). `SelectedCallContexts::call` returns `None` for an invalid handle (418-420).

---

## 18. language-core/src/atomic.rs (568 lines; tests 407-568)

**Purpose.** Compiler-owned vocabulary for atomic operations: memory orderings, which orderings are legal for which operation, and the closed set of compare-exchange result shapes (header 1-7).

**Shape.**
- `MemoryOrdering{NoOrdering, Receive, Publish, ReceivePublish, GlobalOrder}` (10), `FenceOrdering{Receive, Publish, ReceivePublish}` (19), `AtomicOrderingPlan{Load, Store, ReadModifyWrite, Swap, CompareExchange{success,failure}, CompareExchangeOnce{success,failure}}` (26-39) with `success()` 338 / `failure()` 350.
- Legality: `valid_for_load` (382: NoOrdering/Receive/GlobalOrder), `valid_for_store` (386: NoOrdering/Publish/GlobalOrder), `valid_compare_exchange_failure(self, success)` (390-400): success NoOrdering -> failure NoOrdering; Receive -> {NoOrdering, Receive}; Publish -> {NoOrdering}; ReceivePublish -> {NoOrdering, Receive}; GlobalOrder -> {NoOrdering, Receive, GlobalOrder}. No legality function exists for ReadModifyWrite or Swap (any ordering).
- Two-axis compare-exchange schema: `AtomicCompareExchangeAttemptAxis{Decisive, SingleAttempt}` (67) x `AtomicCompareExchangeObservationAxis{Observing, NonObserving}` (73) -> `AtomicCompareExchangeOutcomeIdentity` four values (175, `from_axes` 190, `cases()` 249 returns static case tables 150-170 with case tags 0/1/2 and payloads Observed/Proposed/Displaced). `AtomicExpressionResultCustody{Scalar, CompareExchangeOnce(custody)}` (54) with `is_valid_for(ordering)` (320-330): Scalar is invalid for `CompareExchangeOnce`, a `CompareExchangeOnce` custody is valid only for a `CompareExchangeOnce` plan and only if `is_canonical()` (303-316).
- String keys: `MemoryOrdering::from_name` (361) matches the five spellings; `single_attempt_member` (79) maps "compare_exchange_once" -> Observing and "try_exchange_once" -> NonObserving. Consts 403-405: `ATOMIC_CORE_SOURCE = "atomic.omg"`, fence namespace "Atomic", fence name "fence".

**How it decides.** Ordering legality is a hand-written matrix keyed on the enum value (the test at 412-430 calls it "the c11 matrix"). Outcome identity is keyed on the pair of axes.

**Cases by shape.** `valid_compare_exchange_failure` enumerates five success orderings (391-399); outcome identity enumerates four combinations (194-211).

**Same job elsewhere.** `AtomicObservingCompareExchangeOperation{Decisive, SingleAttempt}` (42) has the same two variants as `AtomicCompareExchangeAttemptAxis{Decisive, SingleAttempt}` (67); `AtomicObservingCompareExchangeResultShape` (48) is a third carrier of the same Decisive/SingleAttempt distinction, with `outcome_identity()` on both (270-290). Grep: `AtomicObservingCompareExchangeOperation|ResultShape` = 13 lines (atomic.rs 9, `validation/access_views_and_borrows.rs` 2, `typed-trees/access_views_and_commitments.rs` 2); `AtomicCompareExchangeAttemptAxis` = 24 lines in 4 files (atomic.rs 14, `block_wire.rs` 6, `optimization-unit/identity.rs` 3, `scalar_graph.rs` 1).

**Unused/one-caller (grep).** `\bFenceOrdering\b`: 38 lines in 14 files (validation `value_custody_core.rs` 9, `terminal-psi/codec/block_wire.rs` 10, ...). `\bAtomicOrderingPlan\b`: 97 lines in 21 files. `valid_compare_exchange_failure|valid_for_load|valid_for_store`: 36 lines in 7 files (atomic.rs 18, `plan_policy.rs` 3, `expressions.rs` 3, `control_flow.rs` 3, `scalar_computations.rs` 2, `value_custody_core.rs` 4, `abstract-operations/atomic.rs` 3).

**Fail-open risks.** None seen in this file: all legality functions are closed `matches!` lists, and `from_name`/`single_attempt_member` return `None` for unknown spellings.

---

## 19. language-core/src/lib.rs (381 lines)

**Purpose.** Crate root. Re-exports `atomic`, `inline_assembly`, and `source_semantics` items (30-49) and defines small vocabulary modules inline: `activation_storage` (51), `cast_form` (64), `constant_value` (86), `data_layout_queries` (131), `descriptor_tables` (166), `layout_storage` (245), `operator_spelling` (254), `static_interrupts` (328).

**Shape.**
- `ActivationStorageOperation{Establish, BorrowBytes, Release, Split, Merge}` (55) + `ACTIVATION_CORE_SOURCE = "activation.omg"` (52).
- `CastForm{Value, Recast(ReferenceAccess)}` (66).
- `EvaluatedConstant{Encoded(String), UndeterminedNaN(FloatFormat)}` (90); `encoding()` returns `None` for NaN (96-101); `Default` is `Encoded("")` (104-108).
- `DataLayoutQueryKind{Size, Alignment}` from names "size_of"/"align_of" (139-145). `is_data_layout_query_spelling(target, target_symbol_valid, receiver_valid)` (155-163) = true iff the name does not resolve to a symbol, has no receiver, and spells `size_of`/`align_of`.
- `DescriptorTableOperation{ActivateTask{entries: u16}, ReadTask, ReloadSegments{entries, kind}, ReadSegment{register}}` (170), `SegmentReload{Code, Data}` and `SegmentRegister{Code, Data, Extra, Stack, Fs, Gs}` with numeric codes 0..5 (204-242).
- `layout_storage::SOURCE` = `include_bytes!` of `source/library/core/layout.omg` (248-251); `static_interrupts::CORE_DECLARATIONS` = `include_bytes!` of `source/library/core/static_interrupt.omg` (339-342).
- `OperatorSpelling` (13 variants, `ALL` 273; `from_symbol` 289 string match: `+ - * / % == != < <= > >= [] [..]`; `symbol` 308).
- `StaticInterruptOperation{Install, Context, BeginCompletion, FinishCompletion}` (332); `from_core_declaration(name, source)` (368-379) returns Some only if `source == CORE_DECLARATIONS` byte-for-byte AND `name` is one of four strings ("X86StaticInterruptScope::run", "view_static_interrupt_context", "begin_static_interrupt_completion", "finish_static_interrupt_completion"); `required_service_identity` (352): Install needs "toolchain::MachineControl", the other three none.

**How it decides.** By **spelling**: operator symbols, size_of/align_of, atomic/Static-interrupt declaration names, and a whole-source byte comparison (369). Header (23-26): "We neither parse nor judge a program".

**Cases by shape.** Fixed lists: 13 operators (273-287), 4 static-interrupt operations, 2 layout queries, 6 segment registers.

**Same job elsewhere.** The `from_code`/`code` numeric pairs for `SegmentReload`/`SegmentRegister` (204-242) and `StaticInterruptOperation::code` (359) parallel the `catalog_tag` mapping in inline_assembly.rs:73-97.

**Unused/one-caller (grep).** `FenceOrdering|from_core_declaration|is_data_layout_query_spelling` combined count 52 across 22 files (not split further for the two spelling helpers; `from_core_declaration` and `is_data_layout_query_spelling` were not individually counted).

**Fail-open risks.** `is_data_layout_query_spelling` (155-163) accepts any unresolved, receiverless call spelled `size_of`/`align_of`: an unresolved user name with that spelling is classified as a layout query rather than an unresolved-name error. Status: "stated" only by the function's contract (it is only true when neither `target_symbol_valid` nor `receiver_valid`); callers outside the unit not traced.

---

## 20. language-core/src/inline_assembly.rs (2946 lines; tests 1884-2946)

**Purpose.** Closed catalog of inline-assembly instructions and register events: for each mnemonic either a contract (target ISA, authority requirement, operand constraints, memory/interrupt/flags effects, clobbers) or a named refusal; plus the scratch-register table and a small x86 RFLAGS model (header 1-9).

**Shape.**
- `register_operations` (17-353): `AsmRegisterCarrier{U8,U16,U32,U64}` (26), `AsmRegisterOperation` (55: ProcessorHalt, InterruptControl, MemoryFence, SystemRead/Write, PortRead/Write, ModelSpecificRead/Write, GlobalDescriptorTable, TaskRegisterActivate/Read, SegmentReload, SegmentRead). `catalog_tag` (73) maps each to a u16 (0x1xx writes, 0x2xx port, 0x3xx MSR, 0x4xx halt/interrupt, 0x5xx fences, 0x6xx descriptor/segment); `from_catalog_tag` (99) is the inverse via a `match` then system-register id decode (140-147); `from_intrinsic_name` (150) matches "asm#..." strings; `requires_service_authority` (172) is true for everything except `SystemRead(Rflags)` and `MemoryFence(_)`; `input_carriers` (183), `result_carrier` (201), `target_applicability` (212, default X86_64), `is_admitted` (222).
- `scratch_registers` (355-522): 51 slot names (358-410): slots 0-13 x86 32-bit names (`eax`..`r15d`) with `ASM_SCRATCH_CLOBBERS` 64-bit names, slot 14 `zf`, slots 15-21 internal `asm#...` names, slots 22-50 `w0`..`w28` with clobbers `x0`..`x28` (485-489). `scratch_register(name)` (491-514) returns `(index, target, clobber)` only for slots <14 or >=22; flag/internal slots return `None`. `is_unmodeled_x86_scratch_view` (416-482) is a ~65-name `matches!` list; `is_unmodeled_aarch64_scratch_view` (516-521) matches wsp/xzr/wzr/x31/w31/w29/w30 or `x0`..`x30`.
- Instruction-class enums, each with a mnemonic / `asm#` intrinsic name / reverse lookup by linear `find` over an array: `AsmGlobalDescriptorTableOperation` (562), `AsmInstructionSerializationKind` (596), `AsmCacheOperationKind` (624), `AsmSchedulingHintKind` (659), `AsmControlRegister` (710: Cr0, Cr2, Cr3, Cr4; Cr2 has no write), `AsmSystemRegister` (789: `Control(..)`, `Rflags`, 9 AArch64 EL1 registers; `catalog_id` 811; `ALL` 850; `WRITABLE` 867 omits Cr2, Esr, Far), `AsmFenceKind` (1022: x86 load/store/full, AArch64 ld/st/sy), `AsmInterruptControlKind` (1091).
- `AsmTargetApplicability{Any, X86_64, Aarch64}` (985) with `intersect` (993) and `accepts_target_name` (1001): X86_64 accepts the names "linux_x86_64", "windows_x86_64", "macos_x86_64", "uefi_x86_64"; Aarch64 accepts "linux_arm64", "macos_arm64".
- `AsmOperandConstraint{role, target_register, access, expected_type_name, maximum_literal}` (1143); `AsmInstructionContract{availability UserChecked|DeriverOnly, shape, target, required_authority None|MachineOwner|PortIo|IdtControl, operands, memory_ordering, interrupt_flag_effect, flags_data_flow, clobbers}` (1216); `AsmCatalogEntry{Contract, Refused(HiddenControlExit | UnmodeledMemoryAccess)}` (1284).
- `asm_catalog_entry(mnemonic)` (1435-1850): control/system-register mnemonics via lookups (1465-1553), then a `match mnemonic` with about 40 contract arms (1556-1789) and two giant `|` lists of refusals: `Refused(HiddenControlExit)` for 80 branch/call/return mnemonics (1781-1789) and `Refused(UnmodeledMemoryAccess)` for ~250 memory-touching mnemonics (1802-1846); `_ => return None` (1847).
- `asm_intrinsic_mnemonic` (1379) maps an `asm#...` intrinsic name to its mnemonic; `asm_intrinsic_retains_flag_place` (1421) answers true if the mnemonic's contract has `SnapshotToOperand`/`RestoreFromOperand` flags flow.
- `X86FlagsEnvironment{rflags}` (1853): `new` (1867) requires bit 1 set and no bits outside mask 0x3f7fd7; `snapshot` (1874) clears bits 16/17 (RF, VM); `restore_arithmetic_bits` (1878) only touches mask 0x8d5.

**Inputs/outputs.** Pure tables; consumers pass a mnemonic or an `asm#` name and read the contract.

**How it decides.** Everything is keyed on **source spelling**: the mnemonic string (`asm_catalog_entry` 1435), `asm#...` intrinsic names, register names and target-name strings. Authority: `required_authority` field per contract; `AsmRegisterOperation::requires_service_authority` (172) by variant.

**Cases by shape.** (5) "Fixed list of shapes refuse the rest": the closed lists at 1781-1789 and 1802-1846 refuse named mnemonics; mnemonics in neither list and not a contract return `None` (1847; test 2602-2608 pins `db`, `svc`, `hvc`, `smc`, `brk`, `lea`, `dsb` as unknown). The refusal reasons are quoted at 1279-1280: "A return, call, or indirect branch would bypass Omega state edges." / "No provenance/permission-bearing operand contract exists yet." Only x86_64 and AArch64 exist (985-990). `dmb` maps to `Aarch64Full` only (1632-1634); AArch64 options other than `ld`/`st`/`sy` are refused by `from_aarch64_option` (1074-1081).

**Same job elsewhere.** The `mnemonic()` / `intrinsic_name()` / `from_intrinsic_name()` triple is hand-written for each of 8 enums (562-657, 659-707, 710-786, 803-982, 1031-1056, 1096-1116); `AsmControlRegister`'s read/write name functions (727-785) are reproduced by `AsmSystemRegister` which delegates only for `Control` (898-957). `from_read_mnemonic` / `from_write_mnemonic` / `from_read_intrinsic_name` / `from_write_intrinsic_name` are each defined twice (control register 763-785 vs system register 959-981). `AsmRegisterOperation::catalog_tag`/`from_catalog_tag` (73-148) duplicates the mnemonic catalog identity as a second numeric table. The per-register operand constants (1325-1351) are 17 constants differing only by register name.

**Unused/one-caller (grep, count mode).** `asm_catalog_entry`: 43 lines in 4 files: inline_assembly.rs 38, `validation/src/machine_calls_core.rs` 1, `validation/src/calls.rs` 1, step 01 `statements_and_assembly.rs` 3 (grepped). `X86FlagsEnvironment|asm_intrinsic_retains_flag_place|is_unmodeled_aarch64_scratch_view|is_unmodeled_x86_scratch_view`: 32 lines in 9 files (terminal-runtime `execution.rs` 3, terminal-interpreter `boundary_effects.rs` 4, validation `calls.rs` 2, step 01 `statements_and_assembly.rs` 3, ...). `accepts_target_name`: 9 call sites (grep content mode): terminal-runtime `execution.rs:440`, terminal-interpreter `engine_execution.rs:321`, `embedding.rs:2423`, `boundary_values.rs:782` and `787`, provider-admission `reachability.rs:146`, `corpus_runner.rs:855`, step 04 `assembly_and_operators.rs:635`, plus the definition `inline_assembly.rs:1001`. `DeriverOnly`: 8 lines (inline_assembly.rs 7, `statements_and_assembly.rs` 1).

**Stated limits.** 1279 comment on `HiddenControlExit`; 1280 on `UnmodeledMemoryAccess`: "No provenance/permission-bearing operand contract exists yet." Header 7-8: "keeping recognized instructions without a complete source contract as refusal entries".

**Fail-open risks.**
- `asm_catalog_entry` returns `None` for any mnemonic not listed (1847). Whether a caller treats `None` as "unknown instruction, reject" or "pass through" is outside this unit; wrong input: any unlisted mnemonic (e.g. an AArch64 `svc`) gets `None`. Status: "open" for the three callers (not traced; counts above).
- The refusal lists are closed by enumeration: a mnemonic variant not in the list (e.g. an unlisted memory-access mnemonic) falls to `None`, not to a refusal (1846-1847). Same status.
- `mov`/`movq` get `target: Any` and an empty operand list with the comment-less note that "register-move contracts delegate operand checking to the assignment" (test 2166); operand checking for moves lives elsewhere (open, not traced).
- `requires_service_authority` is true except for `Rflags` read and fences (172-177): default is "requires", i.e. fail-closed.
- `AsmTargetApplicability::accepts_target_name` returns `false` for any target spelling not in its lists (1001-1010), fail-closed; it still lists "macos_x86_64" and "uefi_x86_64" which are not among the four hosted targets named in AGENTS.md (read; AGENTS.md says linux_x86_64, linux_arm64, macos_arm64, windows_x86_64).

---

## 21. language-core/src/source_semantics.rs (461 lines)

**Purpose.** Small source-vocabulary tables that resolution/typing consult (header 1-6): the reserved `result` binder spelling (`contract_result`, 8-20), place label rendering (`place_root`, 22-77: `root.field` joined with `.`), the `self` receiver spelling (`receiver_binding`, 79-98), and enums: `PatternComponent{Fields, Case}` (103), `LocalBindingRole` (113: Value, DispatchSubject, AtomicObservedResult, RecordPattern, ArmPattern, DispatchObservation; `is_runtime_binding` 125 excludes RecordPattern/ArmPattern; `has_ordinary_initializer` 135), `Multiplicity{Unrestricted, Affine (default), Linear}` (146), `BindingRelevance{Relevant, Erased}` (156), `DataSupplyMode{CheckedShape, BoundaryOpaque}` (169), carry axes (`CarrySuspension`, `CarryCpu`, `CarryHostThread`, `CarryAddress`, 178-209) and `CarryPolicy` (214) with `STRICT`, `PERMISSIVE`, `permits(required)` (236-244), `intersect` (246-275), `Display` (278-301); `CarryPermission` (304: AcrossSuspend, AnyCpu, AnyThread, MovableAddress; `from_name` over the strings "Carry::AcrossSuspend" etc., 328-336; `relax` 338); `CallOperationalAcknowledgement{origin: Source|CompilerSynthesized, acknowledges_suspend, acknowledges_block}` (356); `DomainPredicatePresence` (370), `ReferenceAccess{Shared, Mutable, WriteOnly}` (379; `is_exclusive` 387, `is_readable` 391, `borrow_prefix` "&mut "/"&write "/"&" 395), `DomainClassification{ProgressProfile}` (421).

**How it decides.** By spelling: `is_reserved_result(name)` is `name == "result"` (11-13); `is_self_receiver` is `name == "self"` (82-84); `place_field(root, label)` strips the root prefix and then the `.` (33-37) so `mainline.a` is not rooted at `main` (test 61); `is_rooted_at` = equal or has `root.` prefix (39-41). `CarryPolicy::permits` compares per-axis: a required Forbidden/Origin/Stable axis is always satisfied; a relaxed requirement is satisfied only if the granted policy is relaxed on that axis (236-244).

**Cases by shape.** `DomainClassification` has exactly one variant (421-423). `ReferenceAccess` three. `CarryPermission::ALL` four (312).

**Same job elsewhere.** `place_label` / `place_field` render and parse a place as a `.`-joined string; `ContentStructuralPlace` in content.rs:275 stores the same idea as a typed segment list. `is_reserved_result`: 7 `signature_reserves_result` callers plus others, see below.

**Unused/one-caller (grep).** `signature_reserves_result|is_reserved_result|RESERVED_RESULT` (content mode, uncapped): 7 call sites of `signature_reserves_result` (`validation/proof_contracts_core.rs:1096` and `:10927`, step 02 `symbols.rs:522`, step 05 `checks_contracts.rs:11575`, `flow_calls.rs:949`, `published.rs:3143` and `:3519`) and 12 direct call sites of `language_core::is_reserved_result` (counted by hand from that grep: validation 4, step 02 3, step 05 4, review-evidence `names.rs:128` 1) plus 2 uses of a typed-trees wrapper method `is_reserved_result()` (`calls_and_control_flow.rs:3937`, called at `values.rs:3983` and `published.rs:3525`). `CarryPolicy|CarryPermission`: 249 lines in 61 files.

**Stated limits.** None.

**Fail-open risks.** None seen. Note on naming: `signature_reserves_result` returns `!parameter_names.any(is_reserved_result)` (15-19), i.e. true when NO parameter is named `result` (read; the name reads the other way).

---

## 22. language-semantics/src/const_value.rs (671 lines; tests 418-671)

**Purpose.** Canonical, bounded, length-prefixed text encoding of compile-time constant values, with identity (`CanonicalConstIdentity`), a displayable value (`CanonicalConstValue`), and an atom form for embedding in strings.

**Shape.**
- `DecodedCanonicalConstValue{Integer{type_name, value: i128}, Boolean, Float{format, bits}, Array{type_name, values}, Record{type_name, fields}, Variant{type_name, case_name, fields}}` (94).
- Encoding: `framed(tag, pieces)` writes `tag` then, per piece, `<len>:<piece>` (252-261). `encoding::encode` (13) and `encode_declaration` (17) walk the tree with depth < 64 and nodes < 4,096 and total encoding <= 64 KiB (88-91, 27, 70); the non-declaration form refuses `Float` (43), the declaration form writes `float:f32:%08x` / `float:f64:%016x` (36-42; f32 requires bits <= u32::MAX).
- Decoding: `decode_canonical_encoding` (241) -> `decode_node` (263): tags `float:` (declaration only, hex must be canonical width and not NaN, 274-294), `integer` (type name nonempty, value parses as i128 and re-prints identically so no `+`, leading zero or `-0`, 296-310), `boolean` (via `boolean_literal_spelling` "true"/"false", 312-318), `array` (nonempty type name, children), `record`/`variant` (nonempty names, duplicate field name refused, `decode_fields` 367-385). `take_canonical_piece` (387-404) refuses empty/non-digit/leading-zero lengths and non-char-boundary splits.
- `CanonicalConstValue{type_name, encoding, display}` (120): `new` (194, no validation), `from_decoded` (172-187: derives type_name by variant, refuses floats, encodes, then requires `decode_encoding() == Some(value)`), `boolean` (189), `atom` (206: `#omega-const:` + three length-prefixed fields), `from_atom` (218-227 using `take_length_delimited` 406-416, which parses the length with `usize::parse` and is less strict than `take_canonical_piece`), `identity` (229). `CanonicalConstIdentity{type_name, encoding}` (135) with `integer`, `from_declaration_value` (150-157: round-trip check), `decode_encoding`, `decode_declaration_encoding`.

**How it decides.** Identity is the pair (type_name, encoding) of strings; display is excluded (test 465-477). The decoder re-derives and compares text, so acceptance is by canonical-form string equality (303, 283, 288, 156, 186).

**Cases by shape.** Six value shapes (94-117); floats are admitted only in "declaration" mode and are refused in structural identities (43, 275-277; test 602-651 states "declaration floats do not become structural indices").

**Same job elsewhere.** `BuildTimeValue` (build-time-values/src/lib.rs:29) is another tree of Int/Bool/Float/Text/Struct/Case/Array values with string type/field/variant names; `EvaluatedConstant::Encoded(String)` (language-core lib.rs:90) carries this encoding. Three value representations in this unit; no conversion between them seen within the unit.

**Unused/one-caller (grep, count mode).** `CanonicalConstValue|CanonicalConstIdentity|DecodedCanonicalConstValue|boolean_literal_spelling`: 537 lines in 53 files (const_value.rs 71, `build-time-evaluation/const_initializers.rs` 53, `validation/type_references_and_wire.rs` 57, step 02 `preparation_const_evaluation.rs` 30 and `constant.rs` 30, typed-trees `lib.rs` 31, ...).

**Fail-open risks.**
- `CanonicalConstValue::new` and `from_atom` accept any three strings with no check that `type_name` matches the type inside `encoding`, or that `display` matches (194-204, 218-227); the test at 425-428 constructs `new("ignored outer carrier", encoding, ...)`. Only `from_decoded` verifies a round trip. Wrong program: an atom naming one type and encoding another would be accepted by `from_atom`; consumers that call `decode_encoding` get the inner type only. Status: "open" (consumers outside the unit not traced).
- `take_length_delimited` (406-416) accepts length spellings the canonical parser refuses (e.g. a leading `+`, because Rust `usize::from_str` accepts it - inferred, not run). Status: open.
- `from_declaration_value` / `from_decoded` return `None` when encoding exceeds bounds (fail-closed).

---

## 23. language-semantics/src/content.rs (766 lines; tests 584-766)

**Purpose.** Declarations for content conservation: an algebra identity (interval sets over a coordinate space, or counted quantities of a unit), projection expressions, structural places, conservation terms/equations/plans, a canonical interval-set arithmetic, and stable 64-bit fingerprints of reports.

**Shape.**
- `ContentAlgebraIdentity{IntervalSet{coordinate_space: String}, CountedQuantity{unit: String}}` (11) with `canonical_algebra()` converting to `semantic_vocabulary::ContentAlgebra` (18-30).
- Expressions: `ContentScalarExpression` (53: SubjectField(path), RuntimeScalarEmbedding(path), Natural(String), Successor, Arithmetic{Add|Subtract|Multiply}), `ContentProjectionExpression` (66), `ContentIntervalExpression{start, end}` (76).
- `NaturalInterval{start,end: BigInt}` (96: `new` refuses negative bounds and start > end, 102-110), `CanonicalIntervalSet` (126): `new` drops empty members, sorts, errors on overlap (`member.start < previous.end`, 146) and merges touching members (149-151); `separate` (172), `contains` (178-193, two-pointer scan assuming canonical form), `residual` (195-223, requires `contains`).
- Places/terms: `ContentStructuralPlace{version Entry|Current, root Parameter{position, symbol, name}|Result, segments Case|Field|FixedIndex}` (275), `ContentConservationTerm{Projection{...}, Separate(Vec)}` (282) with `separate()` flattening one level and sorting by encoded bytes (294-304), `ContentConservationEquation::new` ordering the two sides by encoded bytes (314-323), `ContentConservationPlan` (335), `ContentProjectionPlan` (235).
- Fingerprints: `conservation_report_fingerprint` (344), `content_conservation_plan_bytes` (358), `projection_report_fingerprint` (427), `terminal_projection_report_fingerprint` (439-511): FNV-1a (`semantic_vocabulary::Fnv1a`) over hand-built byte strings; names (field/case names) are encoded, symbol handles are not (tests 661-694: "ignores arena-local field symbols").

**How it decides.** Fingerprint identity is keyed on **names and positions**, not symbols: field/case segments encode only `.name` (404-413, 549-557); parameter roots encode only `position` (393-396), not the parameter name or symbol; the `Projection` term encodes `semantic_domain` and `projection_report_fingerprint` but not `domain` or `projection_machine` (the match uses `..` at 379-384). Equation sides and `Separate` members are ordered by their encoded bytes (302, 315).

**Cases by shape.** Two algebras only (`IntervalSet`, `CountedQuantity`); three arithmetic operators; three place-segment kinds.

**Same job elsewhere.** The source-side encoders `encode_projection` / `encode_scalar` (526-582) and the terminal-side `terminal_projection_report_fingerprint` with its nested `encode_terminal_scalar` (439-482) encode the same expression grammar twice for two representations (`ContentScalarExpression` vs `semantic_vocabulary::ContentProjectionScalar`), with the same tag bytes 1-5 and operator codes 1-3. Suspected: the third copy of the algebra tags (3 for IntervalSet, 2 for CountedQuantity) appears in `encode_algebra` 513-524 and again inline at 484-493.

**Unused/one-caller (grep, count mode).** `CanonicalIntervalSet|NaturalInterval`: 26 lines, all in content.rs (the definitions 95-232 and tests 603-658) - zero users outside this file (grepped over `omega-rust`). `terminal_projection_report_fingerprint|conservation_report_fingerprint|content_conservation_plan_bytes|projection_report_fingerprint`: 84 lines in 21 files (content.rs 19, step 05 `checks_content.rs` 15, step 06 `proofs.rs` 10, `semantic-vocabulary/storage.rs` 7, `completed_storage_and_conservation.rs` 5, `optimization-unit/identity.rs` 5, ...).

**Stated limits.** None.

**Fail-open risks.**
- `ContentConservationTerm::separate` flattens only one level of nested `Separate` and does not deduplicate (294-304): a hand-built `Separate(vec![Separate(vec![Separate(..)])])` stays nested, so equal contents can encode differently. Status: open.
- Fingerprints omit `domain`, `projection_machine`, parameter names and owner/callable (379-384, 393-396, 358-369): two plans differing only in those fields have equal bytes/fingerprints. Status: stated for symbols/names by tests 661-694 ("ignores arena-local field symbols"); for the others, open. FNV-1a is a 64-bit non-cryptographic hash (353-355, 434-436, 508-510).
- `CanonicalIntervalSet::contains/residual` rely on both sets being canonical; the fields are private and only `new` builds them (125-170), so this holds (read).

---

## 24. (cross-cutting) What the unit does not contain

Not in this unit and not read: `arena/src/lib.rs` (`Arena`, `Handle`, `HandleSpan`, `ArenaSpanInserter`, `RevisionIndex`), `optimization::codec_bytes`, `semantic_vocabulary`, `source::display_literal_bytes`, `numerics`, `layout_plans`. Claims about them are inferred from names and uses in this unit.

---

## Coverage

All 24 files in the unit file were read in full, in order, in chunks. Line counts (from the unit file):
paged.rs 1-2609 (2609); build-time-values lib.rs 1-91 (91); compiler-storage derive 1-144 (144); compiler-storage lib.rs 1-672 (672); values.rs 1-540 (540); diagnostics lib.rs 1-444 (444); invocation.rs 1-538 (538); evaluation-accounting lib.rs 1-757 (757); tests/accounts.rs 1-257 (257); tests/cached_usage.rs 1-144 (144); extent.rs 1-459 (459); extents lib.rs 1-1334 (1334); loans_and_claims.rs 1-1635 (1635); mapping.rs 1-1487 (1487); filesystem.rs 1-1049 (1049); filesystem_sponsor.rs 1-800 (800); filesystem-authority lib.rs 1-266 (266); flow-effects lib.rs 1-578 (578); atomic.rs 1-568 (568); inline_assembly.rs 1-2946 (2946); language-core lib.rs 1-381 (381); source_semantics.rs 1-461 (461); const_value.rs 1-671 (671); content.rs 1-766 (766).
Read in full: 19,597 lines. Skimmed: 0. Not read: 0. (The long mnemonic lists in inline_assembly.rs 1781-1846 and 2216-2573 were read but are enumerations; I did not diff the test lists against the match lists line by line.)

## Five things a maintainer most needs to know about this unit

1. Handles resolve to a default "dummy" value instead of failing: `PagedArenaSet::get`/`get_mut` (paged.rs:462-476), `SealedPagedArena::get` (1093) and `GenerationalPagedArena::get` (1486) all return a `T::default()` stand-in for a stale or foreign handle, and `get_mut` hands out a writable dummy; safe use needs `is_valid`/`try_get`. `OwnedInterner` and `OwnedTextInterner` compare equal to everything (paged.rs:1726, 1967). `GenerationalPagedArena` has no users outside the arena crate and `reclaim_empty_pages` is a stub returning 0.
2. The extents crate is a linear-authority calculus whose "admitted" entry points are ordinary public constructors (`ExtentRootGrant::from_admitted_provider`, `from_established_program_local`, `ExtentProviderIssuance::from_admitted_provider`, `MappingGrant::from_admitted_provider`, receipts' `from_admitted_provider` taking caller-supplied booleans `confined_to_range`/`borrow_released`/`established`). Conservation is enforced for split/merge/attenuate/loan, but admission of the grants and receipts is by naming convention. `external_loans` and `ordering_events` have no users outside the crate; `mapping` is used only by `access-plans` (resources.rs `range_receipt_context` 10 lines, tests).
3. Several tables are keyed on source spelling and host environment rather than identity: the inline-assembly catalog (`asm_catalog_entry` by mnemonic string; ~80 refused control-flow and ~250 refused memory mnemonics enumerated, unknown mnemonics return `None`, inline_assembly.rs:1435-1850), `accepts_target_name` (a list of target-name strings incl. `macos_x86_64`, `uefi_x86_64`, 1001-1010), `open_flags` bit positions and `FilesystemMetadataLayout::host()` chosen by `cfg!` of the compiler's host OS (filesystem-authority lib.rs:43-69; filesystem.rs:671-741), `from_core_declaration` (byte-equal source + four name strings, language-core lib.rs:368-379).
4. Fixed-shape duplication: the mnemonic/intrinsic-name/reverse-lookup triple is hand-written for eight asm enums; `AsmControlRegister` and `AsmSystemRegister` each define the four from_*_name lookups; `AtomicObservingCompareExchangeOperation` (atomic.rs:42), `AtomicCompareExchangeAttemptAxis` (67) and `...ResultShape` (48) carry the same Decisive/SingleAttempt split three ways; `content.rs` encodes the projection grammar twice (526-582 vs 439-482); the receipt-verification pattern is hand-coded in external_loans (loans_and_claims.rs:759-785) and as whole-struct equality in mapping.rs:241-266; two canonical-path predicates differ on UTF-8 and drive letters (filesystem.rs:44 vs 338).
5. Accounting asymmetries: in `EvaluationInvocationAccount` only work and live-storage are checked against both invocation and aggregate; build-log, filesystem-attempt and result-custody checks compare only the aggregate (invocation.rs:235, 253-259, 394-396) and the sponsor-level legacy charges refuse non-`CheckedTree` measurement while the build-log/filesystem/handle charges do not (lib.rs:195-200 vs 272, 289, 304). In `filesystem_sponsor.rs`, `writers` comment says closing releases extent but `CloseWriter` does not decrement `logical_bytes` (240-247, comment 74), and `entries` never decreases.

## What you are unsure about

- Whether callers outside this unit treat `None`/`false`/dummy results as rejection: `asm_catalog_entry` `None` (3 external callers), `ProofApplicationCalls::visit_calls` `false`, `wants_write` for access mode 3, `get` dummy fallbacks. Counts were taken by grep; call sites were not read.
- Combined-pattern counts: several greps used `a|b|c` patterns and the Grep tool's count mode counts lines per file, so per-name counts are not separable (marked "combined" where so).
- The claim that `usize::from_str` accepts a leading `+` (const_value.rs:408) is from Rust language knowledge, not from running code.
- Whether `CloseWriter` not decrementing `logical_bytes` (filesystem_sponsor.rs:240-247) is intended: the comment at line 74 says "closing releases path and extent", but I did not trace the consumer of `writers`/`logical_bytes` after close.
- `OwnedInterner`/`OwnedTextInterner` always-true `PartialEq` may be deliberate (cache fields excluded from structural equality); I found no comment saying so in paged.rs.
- Test lists vs match lists in inline_assembly.rs (e.g. 1781-1846 vs 2216-2573) were not diffed mechanically.
