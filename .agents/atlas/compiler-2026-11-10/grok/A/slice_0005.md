# slice_0005

Area digest preamble (read): `digests/area_08.md` — stage 05 borrow ledger plus fixed checks (`borrow.rs`, `checks_borrows*.rs`, `checks_content.rs`, contracts, multiplicity, termination). Basenames under `omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/` unless noted. Words: open / stated / traced / suspected / inferred. Digest citations at `457adf4a11f`; this checkout: cited excerpts match at the line numbers below (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0033 | checks_borrows_resources.rs:100, :129-131 | REACHABLE | Root mismatch or invalid operand drops call-loan resource rows with no diagnostic on the main install path. |
| A0034 | checks_borrows_resources.rs:2940-2953 | REACHABLE | Un-activated call-result or descriptor-load loans are omitted from checked resource rosters without diagnostic. |
| A0035 | checks_borrows_resources.rs:5773 | REACHABLE | `retained_inputs` failure for `UnretainedDerived` call loans is swallowed; stored-call-input rows are omitted. |
| A0036 | checks_borrows_resources.rs:4001-4013 | REACHABLE | After descriptor rules fail, a live same-symbol stored loan can satisfy permission via `collect_binding_observation`. |
| A0037 | checks_content.rs:2283-2285 | REACHABLE | Callee with `body_is_present` skips owned result-claim proof for a call-site qualification fact. |
| A0038 | checks_content.rs:2286-2288 | REACHABLE | Non-boundary public requirement signature returns success without further result qualification checks on that branch. |
| A0039 | checks_content.rs:2307-2317 | REACHABLE | Boundary trait machine signature with no boundary requirement route accepts when projection is absent or path is nested. |
| A0040 | checks_content.rs:1258 | REACHABLE | Only `TransformationSource::Call` replacements run `authenticate_call`; body-sourced frontier replacements are skipped. |

## A0033

**Cited sites (read):** `checks_borrows_resources.rs:100-101` — `install_resource` returns `None` when `draft.backing.root != draft.resource.parent_lifetime.root`. `checks_borrows_resources.rs:129-131` — `call_operands::collect` returns immediately when `!facts.flow.control.operands.is_valid(operand)` with no diagnostic.

**Enclosing functions (read):** `call_operands::install_resource` (`:81-115`); `call_operands::collect` (`:117-336`).

**Callers — `install_resource` (search):** pattern `\binstall_resource\b` in `omega-rust` — 1 definition (`:81`), 1 discard use `let _ = install_resource(...)` (`:72` in `call_operands::install`), 1 strict use `.ok_or_else(|| diagnostic(...))` (`:6086-6090`, different module path).

**Callers — `call_operands::collect` (read):** `call_operands::install` (`:42-65`) loops every call argument operand; recursive `collect` at `:155`, `:195`, `:252`.

**Upward trace (read):** `call_operands::install` (`:42-73`) builds `ResourceDraft`s then publishes via `install_resource`; failed installs are ignored (`:72`). `initialize_checked_direct_borrow_resources` calls `call_operands::install` (`:7079`) before `reconstruct_borrow_resource_rosters` (`:7084-7085`). Caller chain: `checking.rs:1122-1127` — `Err` from `initialize_checked_direct_borrow_resources` rejects lowering; no error is produced on the `:72` path. `collect_suppliers` (`:339-394`) maps successful `permission_sources::collect` into drafts whose `parent_lifetime.root` is `supplier.backing.root` (`:383-387`); mismatch at install time drops the row.

**Input that reaches the branch:** A completed call operand whose captured backing place root differs from the constructed parent-lifetime root, or an argument operand handle marked invalid in flow facts while borrow paths still expect a call-loan resource.

**Smallest program (plain words):** A call passes a borrow carrier whose supplier backing root and parent lifetime root disagree; resource installation silently omits the call-loan row and compilation can continue without a borrow-resource diagnostic on that path.

## A0034

**Cited site (read):** `checks_borrows_resources.rs:2940-2953` — in the per-loan loop, if the loan is a call-result loan or descriptor-load incoming reference and the activation bit is false, `continue` skips roster construction for that loan.

**Enclosing function (read):** `loan_resources::reconstruct_borrow_resource_rosters` (`:2905-…`, loop at `:2915-2953`).

**Callers (search):** pattern `reconstruct_borrow_resource_rosters` in `omega-rust` — 1 definition (`:2905`), 1 production call (`:7085`), 1 re-export name in `use` list (`:7067`); count 3.

**Upward trace (read):** `initialize_checked_direct_borrow_resources` (`:7073-7110`) installs operands, stored inputs, validates formation sources, then reconstructs rosters and `install_borrow_resources` (`:7103-7109`); `Ok(())` when roster vectors build. Unskipped loans get `direct.push` / reborrow drafts (`:2954+`); skipped loans never enter those arenas. `checking.rs:1122-1127` only fails on returned `Err` diagnostics from this initializer, not on per-loan `continue`.

**Input that reaches the branch:** A state-owned loan marked call-result or descriptor-load whose activation index is false in `flow.borrow_lifetimes.activations` for that flow state.

**Smallest program (plain words):** A helper saves a call-result or descriptor-loaded reference whose lifetime activation never records as active at the owning flow state; the checker builds resource rosters without that loan and does not diagnose the gap.

## A0035

**Cited site (read):** `checks_borrows_resources.rs:5773` — `Err(_) => {}` on `retained_inputs(...)` for `UnretainedDerived` loans with `established_by_call` valid (`:5761-5774`).

**Enclosing function (read):** `stored_call_inputs::install` (`:5744-5785`); `retained_inputs` (`:5787-5866+`).

**Callers (search):** pattern `stored_call_inputs::install` in `omega-rust` — 1 call (`:7081`); module defined `:5733`.

**Upward trace (read):** Only loans pushed in `retained` get `CheckedStoredCallBorrowInputs` rows (`:5778-5783`). Swallowed errors leave no `stored_call_inputs` binding for that loan. Same initializer as A0034; no diagnostic vector is appended in this `install`. Digest comment at `:5734-5738` (read) states intent to bind saved references to evaluated inputs.

**Input that reaches the branch:** An `UnretainedDerived` loan established by a call where `retained_inputs` returns `Err` (e.g. missing completed call generation, lost original evaluated input, or permission mismatch at `:5808-5825`).

**Smallest program (plain words):** A stored helper reference derived from a call cannot be matched to retained call-input resources; the checker omits stored-call-input facts and continues without reporting the failure (except optional trace eprintln when `check_sites()`).

## A0036

**Cited site (read):** `checks_borrows_resources.rs:4001-4013` — after failed descriptor resolution, if `plain_observation` and `collect_binding_observation(...)` returns true, `resolved = true` without recording failure.

**Enclosing function (read):** `permission_sources::collect` (`:3617-4022`); `collect_binding_observation` (`:4819-4886`).

**Callers — `permission_sources::collect` (search):** pattern `permission_sources::collect` in `omega-rust` — 2 call sites (`:357` in `call_operands::collect_suppliers`, `:1666` elsewhere in same file); 1 definition in `mod permission_sources` (`:3244+`).

**`collect_binding_observation` (search):** 3 references (`:3961`, `:4002`, definition `:4819`).

**Upward trace (read):** Primary path tries invocation, formation, descriptor, reborrow, then binding observation (`:3961-3971`), then invocation accesses (`:3974+`). Failure truncates `output` and may retry observation (`:4000-4013`). Success clears failure accounting (`:4015-4017`). `collect_binding_observation` accepts empty projection, `FlowOperandCompletion::Observation`, symbol root, and any state loan on that symbol with compatible reborrow effect and statement span (`:4828-4869`). `collect_suppliers` treats `collect` success as permission complete (`:357-393`).

**Input that reaches the branch:** Descriptor permission collection failed for a non-frozen observation capture, but another stored loan on the same owner symbol is live at that statement with a compatible access effect.

**Smallest program (plain words):** A call needs a descriptor permission the precise rules cannot supply, yet an unrelated live loan on the same local symbol can be cited as backing; the last-chance observation path marks permission resolved and accepts.

## A0037

**Cited site (read):** `checks_content.rs:2283-2285` — after declared contract path/domain match (`:2273-2277`), if `validation::call_target_has_checked_body_contract(program, target_symbol)`, closure returns `Some(())` without `result_claim`.

**Enclosing function (read):** `call_results::check_call_result_qualifications` (`:2163-…`); validity closure starts `:2193`; success sets `valid` at `:2527-2528`, failure pushes diagnostics `:2529+`.

**Callers (search):** pattern `check_call_result_qualifications` in `omega-rust` — 1 definition (`:2163`), 1 call `check_retained_content_custody` (`:7217`).

**`call_target_has_checked_body_contract` (read):** `validation/calls.rs:40746-40764` — true when target machine/state has `body_is_present` (or target-family members all have bodies); does not inspect result claim tables.

**Upward trace (read):** `check_retained_content_custody` (`:7211-7224`) appends diagnostics on `Err`; `check_checked_facts_recording` (`checks.rs:12144-12153`) extends diagnostics but does not re-prove skipped facts. Early `Some(())` bypasses `result_claim` checks at `:2423-2425` and conservation replay below.

**Input that reaches the branch:** `FactOrigin::CallEnsures` domain membership on the call result place with provenance domain, exact invocation evidence, matching declared qualification, and callee symbol whose machine has a present body.

**Smallest program (plain words):** A call publishes a qualified result domain on the result expression for a callee with a checked-in tree body, without an owned result claim for that path; the content check accepts based on body presence alone.

## A0038

**Cited site (read):** `checks_content.rs:2286-2288` — if `public_contract.is_some_and(|(declaration, _)| !declaration.is_boundary)`, return `Some(())` immediately after the checked-body shortcut.

**Enclosing function (read):** same closure inside `check_call_result_qualifications` (`:2240-2527`).

**Callers:** same as A0037 (`:7217`).

**Upward trace (read):** This branch runs before `private_access_result_contract` (`:2290-2305`), boundary-trait shortcut (`:2307-2317`), contract-fact disjunction (`:2335-2396`), and `result_claim` requirement (`:2423+`). `requirement_contract_signature` supplies `public_contract` (`:2241-2242`).

**Input that reaches the branch:** Call-site ensures fact on result place where the requirement contract signature exists and its declaration is not marked boundary.

**Smallest program (plain words):** A public (non-boundary) requirement signature wraps the call; a qualified result is published at the call site without going through private-access or owned result-claim verification on this route.

## A0039

**Cited site (read):** `checks_content.rs:2307-2317` — when `(projection.is_none() || !path.is_empty())` and `boundary_requirement_signature(...)` is `None` and some boundary trait advertises `trait_machine_signature_by_symbol` for the target, return `Some(())`.

**Enclosing function (read):** same closure in `check_call_result_qualifications`.

**Callers:** same as A0037.

**Upward trace (read):** `boundary_requirement_signature` defined `:2779+` (not read in full). This accept runs before parameter/projection consumption checks (`:2319-2417`) and before mandatory `result_claim` at `:2423`. `valid` true suppresses diagnostics (`:2528-2532`).

**Input that reaches the branch:** Qualified call-result fact where content projection is missing or place path is non-empty, no boundary requirement signature for the domain, but a boundary trait still exposes a machine signature for the callee target.

**Smallest program (plain words):** A nested or unprojected qualified result on a call to a target only linked through a boundary trait listing, with no boundary requirement route; the checker accepts the ensures fact without result-claim or input-conservation proof.

## A0040

**Cited site (read):** `checks_content.rs:1257-1261` — in `check_used_calls`, for each frontier replacement, only `TransformationSource::Call { .. }` invokes `authenticate_call`; other sources fall through with no judgment in this loop.

**Enclosing function (read):** `call_ancestry::check_used_calls` (`:1233-1274`).

**Callers (search):** pattern `check_used_calls` in `omega-rust` — 1 definition (`:1233`), 1 call (`:7216`).

**Related (read):** `authenticate_call` (`:1305-1320`) eventually requires `TransformationSource::Call` in `call_parents` (`:1330-1331` returns `Err("ancestry edge is not a completed call")` for other sources — but Body replacements never reach it from this loop. `TransformationSource::Body` exists elsewhere (`:1708`, multiplicity fixtures).

**Upward trace (read):** `frontier.replacements` from `content.descendant_frontier` (`:1244-1251`). Only Call-sourced handles are pushed to `checked` (`:1260`). Errors from `authenticate_call` become diagnostics (`:1265-1271`). Body-sourced used replacements produce no diagnostic here. Then `check_call_result_qualifications` runs (`:7217`).

**Input that reaches the branch:** A used body-conservation frontier listing claim transformations whose source is `Body` (not `Call`).

**Smallest program (plain words):** Content custody derivation relies on body-sourced claim replacement edges in the conservation frontier; call ancestry checking never authenticates those edges and compilation can proceed if no other pass rejects them.

## What I did not read

- Full bodies of `permission_sources::collect` before/after `:4001`, `retained_inputs` after `:5866`, and `boundary_requirement_signature` (`:2779+`).
- Whether lowering or other borrow checks (`checks_borrows.rs`, overlap, calls) always diagnose missing resources/inputs skipped in A0033–A0035 (not traced upward past `initialize_checked_direct_borrow_resources`).
- Full `check_call_result_qualifications` failure paths after `:2529` and non–call-ensures facts in the same function.
- `git show 457adf4a11f:` line diff for this slice (inferred current lines match digest from read).
- Tests and `omega-rust/psi/compilation/validation` consumers of `call_loan_resources` / `stored_call_inputs`.

SLICE COMPLETE
