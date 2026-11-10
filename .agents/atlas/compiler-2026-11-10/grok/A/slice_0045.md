# slice_0045

Digest: `area_13` (`digests/area_13.md`). Rows A0341–A0348.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0341 | ad:15936 | CLOSED | Production uses pair `integer_bound_terms_equal` with `integer_bound_offset` before accepting structural order or equality. |
| A0342 | ad:23738-23746 | REACHABLE | Stored-integer write validation returns an empty list with no diagnostic when the destination primitive does not accept integer literals. |
| A0343 | br:2045 | UNDECIDED | `DynamicTrait` makes `collect_type` return true without enumerating borrows; whether `dyn Trait` can store borrow leaves was not traced to acceptance. |
| A0344 | br:2877-2879, 3004-3006 | REACHABLE | Missing required lifetime name makes region `constrain` / `matches` return true before comparing actual region identity. |
| A0345 | cc:2765 | UNDECIDED | `Assembly` / `AssemblyFact` statements `continue` without `preserves_rank`; whether assembly can mutate rank coordinates was not read here. |
| A0346 | cc:2624-2626, 2719-2721 | UNDECIDED | Members with no body witness are skipped in `project_component`; downstream admission when `Ok(None)` was not fully traced. |
| A0347 | cc:606-636 | CLOSED | `origin` at the sole caller is always a machine enumerate index matching `adjacency.len()` from `prepare_runtime_rank_components`. |
| A0348 | calls:2062-2069 | REACHABLE | `encode`/`decode` on a receiver spelled like any data definition exits `report_unresolved_value_call` with no diagnostic. |

## A0341

**Claim.** `integer_bound_terms_equal` treats any two `Integer` bounds as the same term (read `omega-rust/psi/compilation/validation/src/arithmetic_domains.rs:15933-15981`) and ignores `offset` in `Symbol` patterns (`15938-15946`).

**Search.** `rg '\binteger_bound_terms_equal\b' omega-rust` (count mode): 4 matches in 3 files — `arithmetic_domains.rs` 2 (definition `15933`, re-export `16150`), `lib.rs` 1 (re-export `1539`), `checks_borrows_overlap.rs` 1 (import `490`). No other production call sites; the import uses alias `bound_terms_equal` at `780`, `803`, `827`.

**Trace.** Read `integer_bound_terms_equal` in full (`15933-15981`, read). Sole external consumer is `checks_borrows_overlap.rs` module `indexes`: `structural_bound_is_at_or_before` (`779-784`), `structural_bound_is_strictly_before` (`802-807`), and `structural_bound_equal` (`826-831`) each require `bound_terms_equal(left, right)` and then compare `bound_offset(left)` vs `bound_offset(right)` via `integer_bound_offset` (`arithmetic_domains.rs:15983-15992`, read). Comment at `checks_borrows_overlap.rs:811-813` documents “same symbol at the same offset” semantics (read).

**Input.** Borrow overlap / selector bound reasoning during checked-tree formation when structural ordering or equality is consulted.

**Verdict CLOSED.** Every production path that calls `integer_bound_terms_equal` also enforces numeric offset agreement; differing literal values or symbol offsets cannot pass structural equality or ordering on term equality alone (`checks_borrows_overlap.rs:779-831`, read).

## A0342

**Claim.** A stored integer write whose destination does not accept integer literals is skipped (`ad:23738-23746`).

**Search.** `rg '\bvalidate_stored_integer_write\b' omega-rust`: 3 matches — definition `arithmetic_domains.rs:23721`, re-export `26819`, single call `program_validation.rs:1919`.

**Trace.** Read `validate_stored_integer_write` from `23721` through the early return (`23734-23745`, read). Guard: `!destination_primitive.is_some_and(|p| p.accepts_integer_literal())` OR no layout plan with nonempty `integer_fields` ⇒ `return Vec::new()` with no diagnostic. Caller `program_validation.rs:1911-1932` (read) extends `outputs.stored_integer_writes` only when diagnostics unchanged; empty `writes` adds nothing.

**Input.** A flow-checked assignment whose established value is treated as integer interval data, but the assignment target’s declared primitive does not `accepts_integer_literal()`.

**Verdict REACHABLE.** Such a program can pass this layer without a stored-integer write record or diagnostic from this function; whether compilation still rejects elsewhere was not traced.

## A0343

**Claim.** A `dyn Trait` value carrying a returned borrow would be classified view-free (`br:2045`).

**Search.** `substituted_result_is_view_free` (`borrow_and_reflection.rs:1134-1158`, read) calls `collect_type` and requires `output.is_empty()`.

**Trace.** Read `collect_type` (`1477-2047`, read). `TypeReferenceNode::DynamicTrait { .. }` arm (`2045-2045`) returns `true` without pushing into `output`. Upstream: `substituted_result_is_view_free` used from `execution_terminal_unit_control_statement_sequence.rs:3486` and `checks_borrows.rs:6433,6465` (grep listing only; callers not opened).

**Input.** Type reference rooted at `DynamicTrait` evaluated for “view free” / empty carried-lifetime inventory.

**Verdict UNDECIDED.** Code matches the digest on classification (`br:2045`, read). Whether the language allows borrow-bearing `dyn Trait` values that would make “view-free” wrong for acceptance was not read (wiki / earlier typing not opened).

## A0344

**Claim.** A required reference with no lifetime name accepts any actual region (`br:2877-2879`, `3004-3006`).

**Trace.** Read `region_correspondence::infer_inputs` (`2850-2985`, read). Closure `constrain` at `2870-2938`: `let Some(required) = required else { return true; }` (`2877-2878`). Read `region_correspondence::validate` (`2987-3050+`, read): closure `matches` at `2998-3030` has the same `None` required ⇒ `true` at `3004-3005`. `prepare_callable_inputs` (`205-244`, read) calls `infer_inputs`; `PreparedCallableBorrowRefinement::prepare` (`256-262`) surfaces errors as `Err(String)`. Use path: `machine_parameters_and_bindings.rs:559` calls `prepare` and maps failure to diagnostics (`566-570`, read).

**Input.** Callable borrow refinement where a required reference parameter’s lifetime name is absent (`None`) while the actual callable carries a concrete region identity.

**Verdict REACHABLE.** Elided required lifetime names skip region comparison in both inference and validation closures; refinement can proceed without equating actual and required regions at those sites (`br:2877-2878`, `3004-3005`, read).

## A0345

**Claim.** `Assembly` / `AssemblyFact` statements are accepted without rank-preservation check (`cc:2765`).

**Trace.** Read `project_component` state loop (`2713-2805`, read). Non-transition statements: `StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => continue` (`2764-2765`) before `prefix::preserves_rank` used for other statement kinds (`2753-2762`, `2825-2837`, read).

**Search.** `rg 'project_component' omega-rust`: definition `call_cycles.rs:2582`, call `RuntimeRankComponents::judge` `669`.

**Input.** Runtime ranked cycle witness projection over a machine state whose body includes inline assembly statements.

**Verdict UNDECIDED.** Assembly is skipped for rank preservation in this loop (read). Whether assembly operands can write rank parameters (and thus should fail ranking) is not decided in this file per digest; backend/asm write semantics were not read.

## A0346

**Claim.** Cycle member with no source-body witness is skipped; component judged by other members (`cc:2624-2626`, `2719-2721`).

**Trace.** Read `project_component` (`2582-2721`, read). `body_witnesses` may be `None` per member (`2607-2611`, `2624-2625` ⇒ `Ok(None)`). Loop at `2719-2721`: `let Some(mappings) = &member_mappings[position] else { continue; }`. `complete` false ⇒ `natural = None` (`2661-2674`, read). `RuntimeRankComponents::judge` calls this at `669` (read).

**Caller (partial).** `checks_termination.rs:273` calls `topology.judge`; witness vector built from selected rank admissions (`256-272`, read). Full component-selection search when some members lack witnesses was not read line-by-line.

**Verdict UNDECIDED.** Skipping unwitnessed members matches the digest (read). Whether a partial witness set can still yield admitted runtime ranking for the SCC was not traced to a definitive accept/reject outcome.

## A0347

**Claim.** Out-of-range index reads as “no unranked component” (`cc:606-636`).

**Trace.** Read `unranked_component_reached_from` (`606-636`, read). `seen.get_mut(origin)?` and `adjacency.get(origin)?` (`621-622`) propagate `None` from `Option` on bad index. `pending` walk uses `seen.get_mut(position)?` (`624-625`).

**Search.** `rg 'unranked_component_reached_from' omega-rust`: definition `606`, sole call `checks_termination.rs:8155`.

**Caller trace.** `prepare_runtime_rank_components` sets `adjacency: graph.edges` (`696-698`, read). `collect_proof_call_dependencies` initializes `vec![Vec::new(); machines.len()]` (`3974-3975`, read). Termination pass uses `for (position, machine) in program.machines().iter().enumerate()` (`8015`, read) and passes `position` into `unranked_component_reached_from` (`8155`, read).

**Verdict CLOSED.** On the only call path, `origin` is always in `0..adjacency.len()` equal to `machines.len()`; out-of-range `origin` would require a mismatched index not produced by that enumerate loop (read). `None` therefore means “no unranked component found” under consistent graph indexing, not a silent skip from bad input in normal use.

## A0348

**Claim.** `X.encode(..)` / `X.decode(..)` with `X` like any data definition returns without diagnostic whether the method exists (`calls:2062-2069`).

**Trace.** Read `target_resolution::report_unresolved_value_call` (`1962-2087`, read). After other early returns, `matches!(target, "encode" | "decode")` plus `data_definitions().any(|d| d.name.as_str() == receiver)` ⇒ bare `return` (`2062-2068`) before unresolved-call diagnostic (`2082-2087`).

**Search.** `rg 'report_unresolved_value_call' omega-rust`: definition `1962`, calls `calls.rs:3470`, `3548` (read listing).

**Input.** Value-position call `Receiver.encode(...)` or `Receiver.decode(...)` where `Receiver` is a local/parameter name matching a top-level data definition’s name, with no trait/machine resolution for that method.

**Verdict REACHABLE.** This function exits without pushing an error in that case (read); duplicate pattern noted at `calls:39211` (grep only, not opened) may also bypass checks.

## What I did not read

- `wiki/` or language-guide material on `dyn Trait` lifetimes (A0343).
- Inline assembly / rank-parameter write semantics outside `call_cycles.rs` (A0345).
- Full `checks_termination.rs` component-selection search and `retain_runtime_rank_component` outcomes for partial witnesses (A0346).
- Downstream rejection paths for skipped stored-integer writes beyond `validate_stored_integer_write` (A0342).
- `calls.rs:39210-39217` and remaining `encode`/`decode` bypass sites (A0348).
- Entire `prove_integer_ordering` / premise oracle beyond cited call sites (A0341).
- `git show 457adf4a11f:` paths (lines matched current checkout for all eight cites).

SLICE COMPLETE
