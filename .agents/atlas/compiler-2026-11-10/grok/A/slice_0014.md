# slice_0014

Digest preamble: area_09 (`digests/area_09.md`) — Psi stage 05 execution plans, flow, published facts, values; shorthand `SG` = `execution_terminal_unit_state_graph.rs`, `SSS` = `execution_terminal_unit_structural_scalar_store.rs`, `TYPES` = `execution_terminal_unit_types.rs`, `F` = `flow.rs`. Cited line numbers matched at read time in the current checkout (same as digest anchors at `457adf4a11f`).

| id | file:line | verdict | one-line reason |
|---|---|---|---|
| A0101 | omega-rust/psi/.../execution_terminal_unit_state_graph.rs:2000-2002 | UNDECIDED | Image-root early exits only fire for string-literal `PlaceRoot::Expression`; whether that always makes “no local home” / `true` sound at program accept was not traced to a rejecting check. |
| A0102 | omega-rust/psi/.../execution_terminal_unit_state_graph.rs:3568-3570 | UNDECIDED | `Unrestricted` and qualified no-code owners skip disposition lookup; whether any program that should fail cleanup still passes `permits_disposal` was not determined. |
| A0103 | omega-rust/psi/.../execution_terminal_unit_state_graph.rs:3895-3897 | UNDECIDED | Empty rank vectors when there is no witness and no runtime components; whether termination/rank obligations elsewhere reject bad programs was not read. |
| A0104 | omega-rust/psi/.../execution_terminal_unit_state_graph.rs:8482-8487 | UNDECIDED | Unit terminator via `erased_proof_completion` when all transitions match erased-dispatch pattern; whether an arm could carry runtime effects without being planned remains open. |
| A0105 | omega-rust/psi/.../execution_terminal_unit_state_graph.rs:7362-7372 | UNDECIDED | Failed `add_reference_type` uses default carrier identity when selectors are static; whether downstream checks catch a bad identity was not traced. |
| A0106 | omega-rust/psi/.../execution_terminal_unit_structural_scalar_store.rs:3061-3088 | UNDECIDED | Failed `plan_assignment` skipped for call/structural-value/array/atomic assignments; whether another builder always plans them on the same statement was not verified end-to-end. |
| A0107 | omega-rust/psi/.../execution_terminal_unit_types.rs:4786-4872 | UNDECIDED | Entry-domain memberships `continue` on missing root, routing, or definition; scalar lane overlap at TYPES:1093 was not traced to a common reject path. |
| A0108 | omega-rust/psi/.../flow.rs:742-748 | UNDECIDED | Borrow rows with unresolved caller/callee machine drop grouping edges; whether mis-grouped flow lets an ill-formed program compile was not determined. |

## A0101

**Claim (read):** GROUP Execution plans — image storage roots return “no local home” or `true` (digest SG:2000-2002, 2566-2568, 3021-3023).

**Cited sites (read):**
- `local_backing_binding` at `execution_terminal_unit_state_graph.rs:1992-2014`: if `is_image_storage_root(program, root)` then `Ok(None)` (`:2000-2002`).
- `owned_parent_source` at `:2558-2578`: same early `Ok(None)` (`:2566-2568`).
- `contents_at` on `loan_backing`’s storage walker at `:3010-3034`: if image root, `return true` (`:3021-3023`).

**Enclosing functions (read):** Full bodies of `local_backing_binding` and `owned_parent_source`; `contents_at` through `:3034` plus caller `contents` (`:3000-3008`) and `root` (`:2982-2998`).

**`is_image_storage_root` (read):** `borrow.rs:501-513` — only `PlaceRoot::Expression` that is a valid `ExpressionNode::String(_)`.

**Callers (search `omega-rust`, word boundary, count mode):**
- `local_backing_binding`: 2 (definition + closure call at `execution_terminal_unit_state_graph.rs:1912` inside `retained_storage`).
- `owned_parent_source`: 6 (definition + five call sites in `retained_storage` / parent collection, same file `:1930`, `:2247`, `:2289`, `:2301`, `:2306`).
- `retained_storage`: 2 in `omega-rust/psi` (definition `:1798`, production call `:6918`).
- `contents_at`: 3 in `omega-rust/psi/.../execution_terminal_unit_state_graph.rs` (definition + internal recursion).

**Upward trace (read):** `retained_storage` (`:1798-1984+`) is invoked from state-transfer planning at `:6918`; failure is `Err(...)` which makes the surrounding `collect::<Option<_>>()` at `:6907` yield `None`, blocking that transfer plan. Image-root branches return success without a local backing ordinal (`Ok(None)` / `true`), so planning continues. `build_traced` (`:9597+`) eventually needs a complete graph; individual `None` arms fail the machine plan rather than silently accepting the whole program. Borrow access for image roots is handled separately at `borrow.rs:539-545` (`read`).

**Input that reaches the branch (read, plain words):** A captured state-loan operand whose parent lifetime root is classified as a string-literal image storage expression (not a parameter/local root).

**Verdict:** UNDECIDED — digest classed this “open”; I did not find a smallest should-reject program that still compiles solely because of `:2000-2002` / `:2566-2568` / `:3021-3023`, nor a single later check proving every image-root loan path is closed (`borrow.rs:499` says access still needs its loan occurrence; that path was not fully traced).

## A0102

**Claim (read):** Unrestricted results and no-code completion owners pass with no cleanup check or disposition row (digest SG:3568-3570; SG:3571-3581).

**Cited sites (read):** `local_results::validate` at `execution_terminal_unit_state_graph.rs:3492-3594`:
- `:3568-3569` — `Multiplicity::Unrestricted` → `Some(())` without reading `facts.contextual_cleanup.dispositions`.
- `:3571-3580` — `no_code_completion_owner` with `has_no_code_cleanup_contents` and borrow-free contents → `Some(())`.
- `:3582-3594` — otherwise requires a matching disposition row.

**Enclosing function (read):** Full `validate`; public wrapper `permits_disposal` at `:3469-3490`.

**Callers (search `omega-rust`, `\bpermits_disposal\(` count):** 9 in `execution_terminal_unit_state_graph.rs` (1 definition in `local_results` + 8 call sites in structural-result consumer logic, e.g. closure at `:4727-4746`).

**Upward trace (read):** `permits_disposal` gates whether a structural result may be treated as disposable on an exit/successor arm (`:4737-4746` inside result custody planning). `false` forces alternate retirement paths; `true` from early `Some(())` skips disposition proof. Machine planning returns `None` on hard failures elsewhere in the same function (e.g. duplicate transfers `:4748-4753`), which rejects the machine plan, not a source-level diagnostic.

**Input (read):** A structural result binding with `Multiplicity::Unrestricted`, or an affine owned no-code-cleanup result satisfying the `:3571-3578` predicates, at a selected exit used by the consumer closure.

**Verdict:** UNDECIDED — whether any program ought to be rejected because a disposition row is missing for an unrestricted or no-code result was not established; unrestricted multiplicity may be intended to excuse disposition (`read` module comment at `:3458-3464`).

## A0103

**Claim (read):** No implementation witness and no runtime components ⇒ empty rank vectors; termination decided elsewhere (digest SG:3895-3897, 3905-3909).

**Cited sites (read):** `natural_rank_plans_for_fused` at `execution_terminal_unit_state_graph.rs:3884-3919`:
- `:3895-3897` — `!has_witness && admitted.runtime_components.is_empty()` → `Some((Vec::new(), Vec::new()))`.
- `:3905-3908` — skip owner with no witness and empty observations.
- `:3910-3911` — `!judgment.local_terminates` → `None` (fails planning).

**Enclosing functions (read):** Full `natural_rank_plans_for_fused`; wrapper `natural_rank_plans` at `:3872-3882`.

**Callers (search `omega-rust`):**
- `natural_rank_plans`: 5 references (2 in `state_graph.rs`, import + use in `execution.rs:1364`, use in `execution_scalar_computations.rs:5349`).
- `natural_rank_plans_for_fused`: 3 call sites in `state_graph.rs` (`:3881`, `:9660`, re-export).

**Upward trace (read):** `build_traced` at `:9659-9660` propagates `None` from `natural_rank_plans_for_fused`, aborting the state graph for that machine. Empty `Some` allows the graph to build with zero natural ranks. `execution.rs:1361-1364` marks terminal signature `Unsupported` only when there *is* an implementation witness and `natural_rank_plans` returns `None`, not when it returns empty ranks (`read`). Rank arrival facts are prepared in `published_facts.rs` (`grep`; `rank_arrivals` at `:7638+` not read in full).

**Input (read):** Fused machine body with no `implementation_witness` on any fused owner and an admitted rank plan with empty `runtime_components`, while each retained judgment still has `local_terminates == true`.

**Verdict:** UNDECIDED — digest relies on termination being decided elsewhere; I did not read `published_facts` termination builders or a witness program that should be rejected but receives empty ranks and still completes `build_traced`.

## A0104

**Claim (read):** Unit state with all transitions on erased dispatch subjects (or `Always`) returns Unit without planning arms; stated intent at SG:9251-9254; open on runtime effects (digest SG:8482-8487, 8704-8713, 9255-9282).

**Cited sites (read):**
- Conditional tail pattern at `:8481-8487` calls `erased_proof_completion` when `result == Unit` and both branches pass `transition_on_erased_dispatch_subject`.
- Guarded-jump tail at `:8704-8712` — same predicate on all tail transitions.
- `erased_proof_completion` at `:9255-9268` — `TerminatorPlan::ReturnUnit` with comment that arms are not executed (`:9251-9254`, read).
- `transition_on_erased_dispatch_subject` / `expression_names_erased_dispatch_subject` at `:9275-9309`.

**Enclosing function (read):** `terminator::build` from `:8082` through the match arms containing `:8482` and `:8704` (partial read of large `build`; tail helpers read in full).

**Callers (search `omega-rust`, `\bterminator::build\(` count):** 1 at `execution_terminal_unit_state_graph.rs:9860` inside `build_traced`.

**Upward trace (read):** Successful `terminator::build` is required for `build_traced`; `erased_proof_completion` short-circuits arm planning and emits `ReturnUnit`. `unit_completion_failure.succeeded()` is called before return (`:8485`, `:8711`) — exact semantics of that counter not read. Other terminator paths still `return None` on guard/type failures (`:8491-8494`, read).

**Input (read):** Unit-result state whose reachable transition prefix is either (a) two complementary `When` transitions whose guards name only an erased dispatch subject local, or (b) a transition tail where every `When`/`Always` transition satisfies `transition_on_erased_dispatch_subject`.

**Verdict:** UNDECIDED — comment states arms are not executed (`read`); I did not determine whether a transition arm could still imply runtime effects (calls, stores) that this path omits from the plan while the machine plan still succeeds.

## A0105

**Claim (read):** When `add_reference_type` fails, carrier identity becomes default and the plan proceeds (digest SG:7362-7372).

**Cited site (read):** `projected_loan_transfer` at `execution_terminal_unit_state_graph.rs:7291-7374` — bounded-view branch uses `Default::default()` (`:7367`); otherwise `add_reference_type` failure with `!has_dynamic_selector` uses `Default::default()` (`:7371`), else `return None` (`:7372`).

**Enclosing function (read):** Full `projected_loan_transfer` through `:7384+` (`structural_transfer_plan` call not fully read).

**Callers (search `omega-rust`, `\bprojected_loan_transfer\(` count):** 3 (definition + `:6655` + `:6735` in the same file’s transfer planner).

**Upward trace (read):** Call sites require `loan_permitted` and successful path projection; `None` from `projected_loan_transfer` drops that transfer arm only. Parent `collect::<Option<_>>()` at `:6907` fails if any required transfer is missing (context `read` at `:6600-6907`).

**Input (read):** Projected loan transfer to a parameter/local with static path segments (no runtime/computed index), target not classified as bounded-view byte carrier, and `ShapeCollector::add_reference_type` returning `None`.

**Verdict:** UNDECIDED — default `TypeIdentityId` may be benign for bounded cases; I did not trace whether a later custody or type check rejects the resulting plan or allows compilation incorrectly.

## A0106

**Claim (read):** Assignments that fail `plan_assignment` but look like calls/structural values, plus atomic/array assignments, are skipped assuming another builder handles them (digest SSS:3061-3088; SSS:3051-3056).

**Cited sites (read):** `build_structural_scalar_field_store_sequence_traced` at `execution_terminal_unit_structural_scalar_store.rs:3017-3090`:
- `:3051-3056` — skip atomic carrier and array assignment types.
- `:3061-3074` — try `plan_assignment`; on success extend stores.
- `:3075-3086` — on failure, if value is `Call` or has structural value root, `continue` (skip store).
- `:3087-3088` — otherwise `return None`.

**Enclosing function (read):** Full `build_structural_scalar_field_store_sequence_traced`.

**Callers (search `omega-rust`, `\bbuild_structural_scalar_field_store_sequence_traced\(` count):** 2 (definition + `execution_terminal_unit_control_statement_sequence.rs:9136`).

**Upward trace (read):** Caller `statement_sequence::build` (`:9134+`) pulls store operations into the statement plan after construction prefix; same sequence loop also plans calls and structural operations separately in the walk (`read` at `:9149+` — not every statement handler read). `return None` from store sequence fails the whole sequence build (`?` at `:9146`).

**Input (read):** Post-construction assignment statement that is not atomic/array-typed, where `plan_assignment` returns `None` but the RHS is a call expression or has a recorded structural value root in `CheckFacts`.

**Verdict:** UNDECIDED — digest assumption that another builder owns the store was not verified by reading the call/structural planners for the same statement index; no concrete should-reject program identified.

## A0107

**Claim (read):** Memberships skipped when root, domain definition, or routing is missing; scalar lane catch not traced (digest TYPES:4786-4872, 1093, 4653-4659, 4760-4762).

**Cited site (read):** `fold_parameter_entry_domains` at `execution_terminal_unit_types.rs:4786-4872` — multiple `continue` paths (`:4822`, `:4828`, `:4834`, `:4846`, `:4851` fail on missing `domain_definition`), `:4845-4846` skip when not routed and not vacuous.

**Related sites (read partially):** `boundary_domain_requirements` at `:1093` — similar `continue` for primitive memberships without provenance; `register_authored_result_domains` at `:4760-4762` — `continue` on missing domain definition.

**Enclosing function (read):** Full `fold_parameter_entry_domains`.

**Callers (search `omega-rust`, `\bfold_parameter_entry_domains\(` count):** 3 (definition + `execution_terminal_unit_calls.rs:6061`, `:6080` inside entry/signature builders).

**Upward trace (read):** `free_structural_scalar_entry_signature_traced` and `scalar_and_structural_parameters` propagate `None` only when `fold_parameter_entry_domains` returns `None` (invalid semantic domain at `:4852-4853`, or failed `add_selected_domain`), not when entries are skipped via `continue`. `execution_terminal_unit_calls.rs` callers use `?` — skip-only paths still yield `Some(())` from fold.

**Input (read):** Structurally compiled state whose `requires` membership names a parameter symbol that cannot be resolved, lacks domain routing/vacuity, or lacks a published domain definition.

**Verdict:** UNDECIDED — digest explicitly left scalar-lane coverage untraced; I did not read scalar parameter domain publication (`VS` lane) to see if the same membership is enforced or rejected later.

## A0108

**Claim (read):** Borrow call with unresolved caller or callee adds no grouping edge; dependent machines may land in separate flow groups (digest F:742-748).

**Cited site (read):** `machine_groups` at `flow.rs:697-804`, loop at `:741-748`:
- `:742-744` — missing `machine_position_by_symbol` for borrow row’s machine → `continue`.
- `:746-748` — call target not in `state_locations` → `continue` (no union in `owner` / `part_owner`).

**Enclosing function (read):** Full `machine_groups`; caller `build_flow_facts` at `:291-350` uses groups for partitioned fixed-point (`:347-348`, read).

**Callers (search `omega-rust`, `\bmachine_groups\(` count):** 2 in `flow.rs` (definition + `:347`).

**Upward trace (read):** `build_flow_facts` (`published_facts.rs:11077` reference from grep, not read) always runs flow for all machines via grouped builders; missing edges only shrink unions — they do not skip building flow for either machine. Ill-formed borrow rows therefore yield more independent groups, not a silent `FlowFacts` omission at the driver level (inferred from `build_groups` structure `:354+`, partial read).

**Input (read):** `BorrowFacts` state row whose `machine_symbol` is absent from the program machine table, or a recorded call borrow whose `target_symbol` does not resolve to any machine state symbol.

**Verdict:** UNDECIDED — whether separated groups allow facts to stabilize incorrectly for programs that should be rejected was not determined; no `machine_position_by_symbol` failure injection traced from earlier phases.

## What I did not read

- Full `terminator::build` and `statement_sequence::build` bodies beyond cited arms.
- `published_facts.rs` rank/termination construction (`:7638+`) and full `build_check_facts` driver.
- Scalar values lane (`values_scalar.rs` / `VS`) for domain membership parity with A0107.
- Stages before Psi 05 that might reject unresolved borrow targets or bad type identities before execution planning.
- `ShapeCollector::add_reference_type` implementation (`execution_terminal_unit_types.rs:3015+`).
- Corpus or sample programs (per worker scope: no build/run).

SLICE COMPLETE
