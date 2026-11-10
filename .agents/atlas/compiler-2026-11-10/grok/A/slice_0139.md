# slice_0139

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A1065 | scalar_applications.rs:1545-1547 | REACHABLE | Empty scalar conditions make `admits_machine` return true for every machine under `Selected` membership; runtime admission gates only reject when it returns false. |
| A1066 | scalar_applications.rs:163-169,826-828,1910-1929,2181-2183 | UNDECIDED | Layout-observation prerequisites are dropped in `prepare` without tracing whether layout demand re-checks them; other sub-claims are partly intentional or unclosed to program acceptance. |
| A1067 | resources.rs:1439-1466 | REACHABLE | Run relocation validates only a sampled cell set; a destination profile that differs on an unsampled interior cell may still pass. |
| A1068 | arena.rs:1023-1038,1113-1137; paged.rs:462-476,1093-1095,1486-1512 | UNDECIDED | Stale or invalid handles read dummy or empty spans by design; not exhaustively traced whether any production acceptance path relies on that without `is_valid`. |
| A1069 | owned_index.rs:92-97; paged.rs:1726-1730,1967-1971 | UNDECIDED | `with_updated` returning `None` is intentional in the sole production caller; interner `PartialEq` always true was not traced to a program-acceptance failure. |
| A1070 | compiler-storage lib.rs:402-408 | CLOSED | Production remapping registers `RetainedSource` on decode; `Handle::read` is the only other `generation::<T>` user and passes through when no domain is registered. |
| A1071 | invocation.rs:344-367; evaluation-accounting lib.rs:272,289,304 | UNDECIDED | `release_storage` underflow is debug-gated; dual sponsor vs usage charging in the checked interpreter was read but not tied to accepting an invalid Omega program. |
| A1072 | extents lib.rs:414-456,691-713; mapping.rs:52-80,257,746-751; loans_and_claims.rs:1412 | CLOSED | Vacuous ordering admission has no callers outside `extents`; mapping grants in production are admission-built; `map_with_source` enforces grant fields against live extents. |

## A1065

**Cited site (read):** `omega-rust/psi/compilation/validation/terminal-installation/src/scalar_applications.rs:1535-1553` — `CompletedScalarScope::admits_machine` matches `ScalarEntryMembership::Selected(members)` but, when `module.scalar_applications.conditions.is_empty()`, returns `true` at lines 1545-1546 before testing `members.contains(&owner)` at 1551-1552.

**Callers (search `admits_machine` in `omega-rust`, count 5 hits, read):**

- Definition/test: `scalar_applications.rs:1418` (test).
- `embedding.rs:2848` — `admitted_scalar_inputs` rejects when entry has a declaration and `!primary.admits_machine(...)`.
- `engine_execution.rs:298` — execution BFS rejects machines with a declaration when `!scope.admits_machine(...)`.
- `artifact_admission.rs:994` — native closure rejects execution machines with a declaration when `!primary.admits_machine(...)`.

All three production gates fail closed only when `admits_machine` is false; they do not add a separate selected-membership check when conditions are empty.

**Input that reaches the branch (read):** An artifact whose Terminal Psi module has zero scalar conditions, plus a completed scalar scope whose `entry_membership` is `Selected` (from `close_scope` at 1880-1885 when `application_machines()` is present). Any machine with a checked declaration then receives `admits_machine == true` even if its proof owner is not in the selected vector.

**Wrong program (plain words):** A build completes scalar admission for an invocation-scoped closure with a selected machine list, but the artifact carries no scalar conditions; execution or native lowering then treats an additional machine outside that list as admitted because the membership check is skipped.

## A1066

**`include_evaluation_entry` (read):** `scalar_applications.rs:162-169` — returns `Ok(())` immediately unless scope is `Invocation` or `AbiStorage`. For `Source` scopes this is intentional no-op behavior, not a fall-through acceptance of a bad program by itself.

**Layout observation prerequisite drop (read):** `scalar_applications.rs:826-828` — in `ScalarEntryPrerequisites::prepare`, `applies` returns `false` for `ScalarApplication::LayoutObservation`, so those prerequisite rows are removed at 850-854 and never enter `admit` (935+). **Not traced:** whether `build-time-evaluation` always schedules layout work via `layout_dependencies` (`scalar_applications.rs:2434+`) before an entry needing that owner is admitted. Call site `build-time-evaluation/src/scalar_applications.rs:111-115` calls `prepare` then `applications()` but layout-observation-owned prerequisites may be absent from `rows`.

**`finish` coverage (read):** `scalar_applications.rs:1910-1934` — checks `retained scalar completion/range/predicate has no target application` only when `self.kind` is `ScalarClosureKind::Source`. `Deployment`, `Invocation`, and `AbiStorage` skip this block.

**`with_application_roots(None)` (read):** `scalar_applications.rs:2181-2183` returns `Ok(self)` unchanged. Single production caller `build-time-evaluation/src/scalar_applications.rs:285` passes `selected.application_roots.as_deref()`; when `None`, observations keep default scope (inferred: deliberate).

**Verdict basis:** Cannot determine whether a program that should be rejected for missing layout-observation prerequisites or incomplete non-`Source` coverage is still accepted without reading the full layout evaluation and closure completion graph.

## A1067

**Cited site (read):** `omega-rust/psi/foundation/access-plans/src/resources.rs:1439-1466` builds a deduplicated `cells` sample: always `0`, and when `count > 1` also `1` and `count - 1`, plus boundary-derived indices from region offsets and small offsets (1444-1459); when `footprint` is zero, truncates to one cell (1462-1463).

**Loop (read):** 1468-1485 runs placement congruence and `validate_initialization_resources` only for sampled cells; returns `Ok(first)` compatibility from cell `0` only (1481-1485).

**Tests (read):** `access-plans/src/tests.rs:2790-2824` varies the last cell (`late_write_only` at 2811-2823), not a distant interior cell.

**Wrong program (plain words):** A run relocation whose destination resource profile matches the source on every sampled cell but differs on a high interior cell (for a long run with fixed footprint) is accepted because that cell is never checked.

**Production caller (read):** `terminal-interpreter/src/storage_providers.rs:3061,3068` and `omega/compilation/pipeline/03_target-operations-to-selected-instructions/src/selection_access_lifecycles.rs:2919` call `validate_run_relocation_resources`.

## A1068

**Arena `get` / `get_mut` (read):** `arena.rs:1023-1037` — `index_from_valid_handle` (1310-1339) returns `invalid_index()` (`usize::MAX`, 1407-1408) on stale generation or unoccupied slot; `get` then uses `items.get(index).unwrap_or_else(|| self.dummy())` (1026), and `get_mut` writes `self.dummy` when index is out of range (1033-1036).

**Stale generation path (read):** mismatch at 1325-1338 yields `invalid_index`, not a panic.

**Spans (read):** `span_or_empty` / `span_mut_or_empty` at 1113-1136 return empty slices when `valid_span_range` fails.

**Paged variants (read):** `paged.rs:462-465` (`get` → dummy), 468-475 (`get_mut` → dummy), 1093-1095 (`SealedPagedArena::get` → dummy), 1486-1499 (`ConcurrentPagedArena::get` → dummy ref on invalid/stale).

**Caller search:** Not performed exhaustively across all `omega-rust` arena `get` sites with versus without preceding `is_valid` (would decide whether compiler acceptance depends on silent dummy reads).

**Wrong program (plain words):** Internal compiler state holds a dangling arena handle; a path that omits `is_valid` reads default slot content and continues compilation instead of rejecting the input.

## A1069

**`with_updated` (read):** `owned_index.rs:92-97` — when `update(Some(existing))` returns `None`, still sets `*seen = revision` and runs `existing`.

**Production caller (read):** `checked-trees/src/lib.rs:273-294` — `update` returns `None` after incremental index extension (287-287, 294) by design; not a stale-index bug on that path.

**Interner equality (read):** `paged.rs:1726-1729`, `1967-1970` — `PartialEq` always returns `true`. **Search** `==.*Interner` / `OwnedIndex.*==` across `omega-rust` (`grep`): no production equality tests on interners found (inferred: used for opaque record identity, not semantic comparison).

**Undecided:** Whether always-true interner equality can cause incorrect merged compiler-storage graphs affecting which programs pass validation.

## A1070

**Cited site (read):** `compiler-storage/src/lib.rs:402-416` — `generation<T>` returns `Ok(original)` when no `GenerationRemappingDomain` for `TypeId::of::<T>()` (408-408); otherwise looks up `restored` or errors (409-415).

**Decode registration (read):** `syntax-trees/src/control_flow_and_names.rs:889` — `SyntaxTextOwner::read` calls `input.remap_generations::<RetainedSource>()`; `RetainedSource::read_storage` uses `define_generation` (855-862). **Search** `define_generation` in `omega-rust`: only that file plus tests in `compiler-storage/src/lib.rs`.

**Handle generations (read):** `arena.rs:137-141` — non-zero arena index uses `input.generation::<T>(generation)?` on decode.

**Closing check:** Every production path that remaps generations registers a domain before `define_generation`; types without a domain correctly pass through unchanged.

## A1071

**`release_storage` (read):** `invocation.rs:344-367` — `debug_assert!` on live cell/byte balances (357-362), then unchecked subtraction (363-366). Callers at 495, 527, 536 (read: same file). In release builds, violation would not abort (inferred).

**Aggregate sponsor charges (read):** `evaluation-accounting/lib.rs:272-301` update only the aggregate `consumed` mutex. `invocation.rs:227-265` updates both shared sponsor and invocation-local counters and enforces aggregate limits.

**Checked interpreter (read):** `checked-interpreter/src/execution.rs:2360-2383` charges both optional `BuildEvaluationSponsor` and local `usage` meters for build log and filesystem attempts.

**Undecided:** Whether accounting mismatch can cause an invalid Omega program to be accepted rather than only corrupting internal limits.

## A1072

**Public constructors (read):** `extents/src/lib.rs:414-431`, `439-456`, `691-713` — `const fn` field fillers with no proof parameters.

**`MappingGrant::from_admitted_provider` (read):** `mapping.rs:52-80` stores caller-supplied rights and obligations without validation. **`map_with_source` (read):** `mapping.rs:714-751` checks grant against source/destination custody and required rights, then sets `mapped` extent rights from `grant.mapped_rights` (746-749) without requiring `mapped_rights` to be a subset of source rights beyond the required-source check (720-721).

**Vacuous fact check (read):** `mapping.rs:257` — `established_facts.contains(obligations)` is vacuous when `obligations` is empty.

**Empty ordering roles (read):** `loans_and_claims.rs:1412-1426` — `covered == requirement.roles` with both empty admits. **Search** `admit_role_coverage` in repository: only `loans_and_claims.rs` tests and `lib.rs` re-export; no callers outside `extents` (read grep).

**Production grant construction (read):** `MappingGrant::from_admitted_provider` use outside tests: only `mapping.rs:992` (in-module tests) and `access-plans/src/tests.rs:736`. Runtime extent roots use `ExtentRootGrant::from_admitted_provider` from admission (`terminal-runtime/.../backing.rs:65` read via grep).

**Wrong program for widened grant:** Would require feeding a hand-widened `MappingGrant` into mapping; no traced Omega compilation path builds grants that way.

## What I did not read

- Exhaustive `omega-rust` audit of every arena `get`/`get_mut` site without `is_valid`.
- Full scalar layout-evaluation graph proving or disproving recovery of dropped layout-observation prerequisites.
- All terminal-installation and deployment scalar tests for empty-condition `Selected` membership.
- Remaining `area_15` digest body beyond the preamble and ACCEPT rows.
- `git show 457adf4a11f:` paths (current checkout line numbers matched cited symbols).

SLICE COMPLETE
