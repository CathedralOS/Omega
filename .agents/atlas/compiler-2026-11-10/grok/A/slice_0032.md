# slice_0032

Digest: area_10 (Psi stages 06–08, lowering to Terminal Psi). Preamble read: `.agents/atlas/compiler-2026-11-10/digests/area_10.md` lines 1–25 (read). Path shorthand: bare names under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`.

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0240 | retention.rs:3556 | UNDECIDED | Unlisted `OperationKind` arms return `None`; whether a domain-tagged result can still compile without operand-sourced memberships was not traced to an accept gate. |
| A0241 | retention.rs:3742 | CLOSED | Early `Ok(())` only when there are no structural domains or no domain-carrier structural results to enrich. |
| A0242 | retention.rs:3474 | UNDECIDED | Three `if let Ok` sites drop membership sources; codec reference validation exists but was not traced on the compilation success path. |
| A0243 | retention.rs:5369 | UNDECIDED | Unclassified loans are stored as `Unresolved`; no downstream rejector for suspension `Unresolved` variants was found in the read set. |
| A0244 | retention.rs:4881 | UNDECIDED | Crossings outside the machine/state-block filter are skipped with no error; whether a live crossing can be omitted while lowering still succeeds was not traced. |
| A0245 | scalar_computations.rs:712 | UNDECIDED | Several `Ok(())` exits skip pure-argument cross-check; only `LocalInitializer` invokes it and stage-05 callee shape was not read. |
| A0246 | scalar_computations.rs:3104 | UNDECIDED | Missing place-loan plan selects the non-loan store path; whether the checker forbids captured stores without a plan was not read. |
| A0247 | scalar_computations.rs:3568 | UNDECIDED | Missing identity text becomes empty spelling; whether terminal path segments always carry resolvable text was not read. |

## A0240

**Cited site (relocated):** `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/retention.rs:3556` — digest cited `retention.rs:3556`; line still matches `_ => return None` in `operand_memberships::retained` (read).

**Enclosing function (read):** `operand_memberships::retained` (`retention.rs:3411-3570`) builds an optional `PayloadMembershipContract` from the operation `kind` match (`3426-3555`). Listed kinds: `MoveStructuralField`, `EstablishReference`, `EstablishStructuralArray`, `EstablishRecord`, `EstablishStructuralCase`. Any other kind hits `_ => return None` (`3556`). Non-empty `projected_qualifications` are required before `Some(...)` (`3558-3569`); helper failures also return `None` via `?`.

**Callers (search):** `rg '\boperand_memberships::retained\b'` over `/agent/repos/omega` — **1** production use: `retention.rs:3840` inside `payload_memberships::retain` (`3786-3953` loop) (read).

**Upward trace (read):** `payload_memberships::retain` (`3741`) is invoked from `machine_lowering.rs:2065` after body lowering, in the terminal-selection retention batch (`2060-2067`). At `3840-3849`, a `None` from `operand_memberships::retained` simply skips pushing that contract; other sources in the same iteration still run (`3814-3829`, `3830-3838`, `3851-3931`).

**Input reaching the branch:** A lowered operation with a structural result whose carrier is in `MembershipReachability` (`3421-3422`), on an operation kind not in the five arms — e.g. any other `OperationKind` with `operation.result.structural()` present.

**Accept decision:** Lowering continues; `merge` only adds qualifications when a contract is produced (`3938-3948`). No `LoweringError` is raised at `3556`. Whether omission causes a program that should be rejected to be accepted depends on consumers of `result.projected_qualifications` for unlisted kinds; those consumers were not read (inferred from unit_069 note: status open).

**Verdict:** UNDECIDED — consumer and compilation accept gate not traced.

## A0241

**Cited sites (read):** `retention.rs:3742-3744` — `if module.structural_domains.is_empty() { return Ok(()); }`. `retention.rs:3768-3770` — if no machine has a domain-carrier structural operation, `return Ok(())`. `retention.rs:3783-3785` — per-machine `continue` when `!has_candidate`.

**Enclosing function (read):** `payload_memberships::retain` (`3741-3956`).

**Callers (search):** `rg 'payload_memberships::retain'` — **1** production call: `machine_lowering.rs:2065` (read).

**Upward trace (read):** Retention runs after emission; early exit leaves operation results unchanged (fewer merged qualifications than after a full fixed point). `MembershipReachability` and the candidate scan (`3752-3767`) limit work to machines that already emit structural results on domain carriers.

**Input reaching the branch:** (1) Module with empty `structural_domains`. (2) Module where no operation’s structural result type is reachable from any domain carrier. (3) Machine index with no qualifying operations.

**Accept decision:** These are preconditions for the enrichment pass, not alternate accept paths for ill-formed input. Programs with active domain obligations require non-empty `structural_domains` and candidate operations (read `3752-3765`).

**Verdict:** CLOSED — early exits apply only when there is nothing to retain; they do not bypass a separate rejection check for invalid domain use (read).

## A0242

**Cited sites (read):**
- `retention.rs:3474-3487` — `if let Ok(captured) = descriptors::reference_backing_memberships(...) { projected_qualifications.extend(captured); }` (ignores `Err`).
- `retention.rs:3488-3503` — `&& let Ok(current) = history.current_selected_payload_contract_in(...)` (ignores `Err`).
- `retention.rs:3814-3828` — `if let Ok(projected_qualifications) = reference_memberships.for_operation(...)` (ignores `Err`).

**Enclosing functions (read):** `operand_memberships::retained` (`3411-3570`) for the first two; `payload_memberships::retain` loop (`3798-3933`) for the third.

**Callers:** Same as A0240/A0241 (`retention.rs:3840`, `machine_lowering.rs:2065`).

**Upward trace (read):** Ignored errors drop one membership source; lowering still returns `Ok(())` from `retain` (`3955`).

**Closing check (partially read):** `omega-rust/psi/compilation/representations/terminal-psi/codec/src/foundation_validation.rs:7436-7638` — `validate_reference_memberships` / `validate_reference_membership` recomputes captured qualifications and errors with `"reference establishment cannot retain source membership obligations"` when `result.projected_qualifications` does not match (`7607-7636`) (read). **Caller search:** `rg 'validate_reference_memberships'` — invoked from `foundation_validation.rs:10478` inside `validate_structural_foundation` (`10256`) (read). **Decode path (read):** `semantic_module.rs:4109` calls `validate_structural_foundation` on `decode_module`; `encode_module` at `4072-4076` does not call it (read `semantic_module.rs:4072-4110`, inferred from unit_081 record cite).

**Input reaching the branch:** `EstablishReference` operation where backing or history lookup fails but the arm still produces or merges other qualifications; or any operation where `reference_memberships.for_operation` returns `Err`.

**Verdict:** UNDECIDED — whether every compilation success path runs `validate_structural_foundation` (or an equivalent check) before treating the program as accepted was not traced; stage `08/lib.rs:104-117` calls `activation_storage::validate_module`, not full foundation validation (read).

## A0243

**Cited sites (relocated, read):**
- `retention.rs:5369` — `TerminalSuspensionLoanCapture::Unresolved` when stored-loan capture cannot be resolved (`5353-5370`).
- `retention.rs:5393` — `_ => TerminalSuspensionLoanResource::Unresolved` in resource classification (`5371-5394`).
- `retention.rs:5547-5548` — `SuspensionCrossingLoanBacking::Unresolved` → `TerminalSuspensionLoanBacking::Unresolved`.
- `retention.rs:5647-5648` — `SuspensionValueBorrowFrontier::Unresolved` → `TerminalSuspensionValueLoans::Unresolved`.

**Enclosing function (read):** `retain_loan_graph` (`retention.rs:5099-5653`), called from `retain_suspension_call_plans` (`4863-5097`).

**Callers (search):** `rg 'retain_suspension_call_plans'` — **2** hits: definition `retention.rs:4863`, call `machine_lowering.rs:2041` (read).

**Upward trace (read):** `retain_suspension_call_plans` still returns `Ok(())` after writing plans with unresolved fields (`5095`). Stage 05 can also classify frontier as unresolved (`checks.rs:6282`, `6715` — not read in full).

**Search for rejector:** `rg 'TerminalSuspensionLoanCapture::Unresolved'` over `omega-rust/psi/compilation/representations/terminal-psi/semantics` — **0** matches (read grep). Occurrences in `ownership.rs` hashing and codec tests only (read grep summary).

**Input reaching the branch:** Suspension crossing loan fact that is not `Incoming`, `LifetimeDependency`, `OutputInput`, `Stored`, or `CapturedActual` in the expected resolved shape, or stored loan without formation capture / valid resource handles.

**Verdict:** UNDECIDED — whether a later stage rejects terminal plans carrying `Unresolved` loan capture/resource/backing/frontier was not traced.

## A0244

**Cited site (read):** `retention.rs:4881-4886` — `.filter(|crossing| source_machines.contains(&crossing.machine) || state_blocks.iter().any(|block| block.state == crossing.state))` on `checked.facts.carry.suspension_crossings`.

**Enclosing function (read):** `retain_suspension_call_plans` (`4863-5097`).

**Callers:** `machine_lowering.rs:2041` with `&source_machines` and `&lowered.state_blocks` (read `2035-2048`).

**Upward trace (read):** Filtered-out crossings never reach occurrence join (`4888-4901`). No error is reported for omission. `fused_component_transfer` (`5911-5939`) only applies when join fails inside the filtered set (`4896-4898`).

**Input reaching the branch:** A checked `suspension_crossings` row whose `machine` is not in `source_machines` and whose `state` does not appear in any `state_blocks` row passed to retention.

**Verdict:** UNDECIDED — whether lowering can complete for a program that still has a live suspension on an unselected machine/state without emitting a plan was not read (`lower_terminal_selection` machine set not read).

## A0245

**Cited sites (read):** `scalar_computations.rs:712-778` — `consume_pure_call_arguments`:
- Non-`CheckedScalarCallee::Body { .. }` → `return Ok(())` (`725-733`).
- Non-empty structural arguments → `return Ok(())` (`741-742`).
- Operand not `CheckedScalarComputationKind::Value` → `return Ok(())` (`755-758`).
- Otherwise calls `consume_pure_call_argument` which errors if no retained binding matches (`706-708`) (read `660-709`).

**Callers (search):** `rg 'consume_pure_call_arguments'` — **2** hits in `scalar_computations.rs`: definition `712`, call `7108` (read).

**Upward trace (read):** `retained_root_argument` (`7093-7119`) calls it only when `root.role` is `CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }` (`7107-7117`).

**Input reaching the skip branches:** Scalar computation root tagged `LocalInitializer` whose call node is not a `Body` callee, has structural arguments, or has non-`Value` operand nodes — cross-check not run.

**Verdict:** UNDECIDED — whether stage 05 only constructs `Body` pure calls with `Value` operands for local initializers (so skips are dead) was not read.

## A0246

**Cited sites (read):**
- `scalar_computations.rs:3104-3110` — `loan_subject` returns `Ok(None)` when `planned_place_loans(...).find(|loan| loan.symbol == symbol)` is `None`.
- `scalar_computations.rs:3457-3459` — `loaned_store` returns `Ok(None)` when `loan_subject` is `None` (after non-`Local` root returns `Ok(None)` at `3454-3455`).

**Callers (search):** `rg 'loaned_store'` — **1** production call: `unit_operation_frame.rs:6054` in `scalar_field_store` (read).

**Upward trace (read):** `scalar_field_store` (`6041-6165`): if `loan` is `None`, execution uses `root` from the store plan and the parameter/local non-loan emission path (`6114-6165`) instead of the captured-loan path (`6067-6113`) (read).

**Input reaching the branch:** Checked scalar field store to `CheckedStoreRoot::Local { symbol }` where `terminal_unit_effects` has no `CheckedPlaceLoanPlan` for that symbol in the state.

**Verdict:** UNDECIDED — whether the checker always emits a place-loan plan whenever the loaned store path is required was not read (`prepare_place_observations` errors on failed capture at `3155-3157` but is a separate path).

## A0247

**Cited site (read):** `scalar_computations.rs:3568-3572` — `StructuralPathSegment::Field(field)` maps via `identities.text(*field).unwrap_or_default()` into `CheckedMemberIdentity::of_spelling(...)`.

**Enclosing function (read):** `checked_path_segment` (`3562-3577`), used from `captured_store_path` (`3540-3559`) on loan path segments.

**Callers:** `captured_store_path` used from `loaned_store` (`3473`) and loaned byte-store preparation (read `3473`, `6054` chain).

**Input reaching the branch:** Terminal structural path segment `Field` whose identity handle has no text in `TerminalIdentityTableBuilder` at lowering time.

**Verdict:** UNDECIDED — whether emission always registers field identity text before loan path lowering was not read; empty spelling may or may not diverge from checked member identity.

## What I did not read

- Full `validate_structural_foundation` driver and whether product compilation invokes it before success (`foundation_validation.rs` beyond `7436-7638`).
- Stage 05 rules for scalar computation callee kinds, structural arguments, and place-loan plans for field stores (`05_typed-trees-to-checked-trees/`).
- `lower_terminal_selection` / `source_machines` construction versus full `suspension_crossings` roster (`machine_lowering.rs` outside `2025-2072`).
- Corpus or sample programs exercising unlisted operand kinds, unresolved suspension loans, or filtered suspension crossings.
- `git show 457adf4a11f:` comparisons (current checkout later than digest commit; line numbers for cited sites were re-checked by symbol where noted).

SLICE COMPLETE
