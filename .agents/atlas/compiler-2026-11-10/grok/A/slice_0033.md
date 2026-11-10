# slice_0033

Area digest preamble (read): Area 10 covers Psi stages 06–08 (checked trees → Lowered Psi → optimizations → Terminal Psi). Path shorthand: bare names under `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/`; `07/` and `08/` under the matching pipeline dirs. Reader labels (open/stated/traced) vs digest author “checked”. Citations at `457adf4a11f`; current checkout line numbers for cited sites match the digest excerpts (read).

| id | file:line | verdict | one-line reason |
| --- | --- | --- | --- |
| A0248 | scalar_computations.rs:6060-6082 | REACHABLE | Identical `Value` select arms skip the false `claim_continuation` and emit no `Conditional`, so a false-route permission claim can be dropped. |
| A0249 | scalar_computations.rs:7107-7117 | UNDECIDED | `consume_pure_call_arguments` runs only for `LocalInitializer` roots; stage-05 checks for other roles were not traced. |
| A0250 | scalar_computations.rs:5847 | CLOSED | `continuation = 0` is a placeholder only when `normal_result` scope replaces it inside `argument_with_normal_result`. |
| A0251 | scalar_computations.rs:4432-4441 | CLOSED | Early return applies only when `established_case_payload` is `Some`, after shared-borrow admission. |
| A0252 | scalar_lowering.rs:1761-1763 | UNDECIDED | Closed `Boolean`/`Integer` clauses become `Proposition::Truth`; whether `Boolean(false)` requires reach lowering untraced. |
| A0253 | scalar_lowering.rs:1783-1792 | CLOSED | `projected_guarantees` drops non-projectable rows before `clauses`; direct `clauses` still errors on `None` / `SourceCheckedGuarantee`. |
| A0254 | scalar_lowering.rs:1827-1831 | UNDECIDED | `type_identity_text(...).unwrap_or_default()` on erased proof formals; whether missing identities can reach this after stage 05 not traced. |
| A0255 | scalar_lowering.rs:2506-2508 | CLOSED | Loop `continue` only filters terminal publication; lowering still errors on missing identity for carriers that are published. |

## A0248

**Cited site (read):** `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_computations.rs:6060-6082` — in `selection::selection`, `identical_values` is true when neither arm appears in `completed` and both plan nodes are `CheckedScalarComputationKind::Value` with equal payloads; then `continuation` becomes `when_true_target` without `when_false_completion`, `when_false_target`, or the `LoweredScalarBranchTerminator::Conditional` block (`6072-6106`).

**Enclosing function (read):** `Expansion::selection` (`scalar_computations.rs:5975-6123`); type disagreement on condition/arms returns `unsupported` (`5988-6007`). True-arm `claim_continuation` with `selected: true` still runs (`6040-6050`, read).

**Callers (search):** pattern `self.selection(` in `omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/scalar_computations.rs` — 1 production call at `7625` inside `argument`’s `CheckedScalarComputationKind::Select` arm (`7619-7636`, read).

**Upward trace (read):** `argument` (`scalar_computations.rs:7260+`) is reached from scalar graph expansion during stage-06 body emission. Skipping the false arm removes the `PreparedClaimEdge` that `claim_continuation` would attach for `selected: false` (`5939-5971`, read). Condition-prefix computations are still wrapped afterward (`6108-6117`, read), but there is no branch between distinct true/false scalar targets when `identical_values` holds.

**Input that reaches the branch:** Checked `Select` computation whose true and false operands are both closed `Value` nodes with the same constant, not yet listed in `completed`, while claim-binding destinations for the source expression still depend on the false decision route.

**Smallest program (plain words):** A scalar conditional whose then- and else-arms are the same integer constant, with permission claims on the conditional expression that require a false-branch claim when the condition is false; lowering keeps only the true-branch claim and never records the false route.

## A0249

**Cited site (read):** `scalar_computations.rs:7107-7117` — in `retained_root_argument`, only `CheckedScalarExpressionRole::LocalInitializer { binding_ordinal }` invokes `calls::consume_pure_call_arguments`; any other `root.role` skips that call and returns `Argument::Computation(root.root)` (`7118`, read).

**Enclosing function (read):** `retained_root_argument` (`7093-7119`); `retained_root` (`7065-7091`) then type-checks and calls `argument`. `consume_pure_call_arguments` (`712-778`, read) itself returns `Ok(())` without work for non-`Body` callees, non-empty structural arguments, or non-`Value` operands (`725-758`, digest record 069 item 70).

**Callers of `retained_root_argument` (search):** pattern `retained_root_argument` in `omega-rust` — 1 definition (`7093`), 2 call sites (`5756` in `normal_return`, `7074` in `retained_root`); count 3 lines in file (def + 2 calls).

**Callers of `consume_pure_call_arguments` (search):** pattern `consume_pure_call_arguments` in `omega-rust` — 1 definition (`712`), 1 call (`7108`); no other production uses.

**Upward trace (read):** `normal_return` (`5733+`) passes caller `role` into root lookup (`5747-5749`). `retained_value` / `retained_root` used from `unit_arguments.rs` (`3041`, `3307`, `6118`) and `scalar_lowering.rs` (`335`, read: names only). Lowering errors on duplicate/mismatched roots (`5751-5754`, `7100-7101`) but does not re-run pure-call consumption except for `LocalInitializer`.

**What would decide:** Whether stage 05 already enforces pure-call argument retention for every `CheckedScalarExpressionRole` that can reach `retained_root_argument` without this lowering hook.

**Smallest program (plain words):** Not stated — would be a checked scalar root in a non-`LocalInitializer` role whose call-operand retention is wrong and is neither rejected earlier nor caught here.

## A0250

**Cited site (read):** `scalar_computations.rs:5845-5848` — in `normal_return`, after `finish` maps targets, `continuation` is the ordinary target, but `NormalCompletionTarget::Selected(_)` sets `continuation` to `0` before `argument_with_normal_result` (`5849-5857`).

**Enclosing function (read):** `Expansion::normal_return` (`5733-5869`).

**Callee (read):** `argument_with_normal_result` (`7297-7333`) — when `normal_result` is `Some`, `normal_result_scope` (`5872-5922`) supplies `selected.unwrap_or(target)` as the real target (`7327-7330`); for `NormalResultTarget::Value`, it rebuilds claim edges along the scoped path (`5895-5918`).

**Callers of `normal_return` (search, read names only):** used from scalar return lowering in `scalar_computations.rs` returns module (not fully enumerated).

**Why every path is closed:** `0` is never used as a final branch index when `scope` is `Some`; it is overwritten by `normal_result_scope` before `binding`/`push` on the computation path (`7326-7344`, read).

**Input that reaches the branch:** Scalar return classified as `NormalCompletionTarget::Selected` with normal-result claim/dispatch metadata attached.

## A0251

**Cited site (read):** `scalar_computations.rs:4432-4441` — in `observe`, after readable-custody and path setup, if `bindings.established_case_payload(...)?` is `Some(payload)`, the function returns `Ok(payload)` immediately.

**Enclosing function (read):** `observe` (`4394-4475+`, read through `4475`). Prior guards: atomic path (`4401-4403`), shared-borrow admission (`4404-4406`), `resolved_observation_path` (`4409-4410`, read).

**Callers of `observe` (search):** pattern `\bobserve(` in `scalar_computations.rs` — 4 matches: definition at `4394`, calls at `3019`, `4151`, `4255` (3 production call sites).

**Upward trace (read):** `established_case_payload` (`expression_bindings.rs:5435-5449`) maps `structural_fields.established_payload` to `LoweredDirectExpression::Parameter`; skipped tail includes `runtime_index` / `byte_overlay` checks (`4442-4467`, read) meant for non-established carrier paths.

**Why closed:** The early return is conditional on `Some` established payload from checked binding tables; otherwise execution continues into the carrier checks the digest names.

**Input that reaches the branch:** Field observation on a case payload the checked tree already records as established at the given place, case, and field.

## A0252

**Cited site (read):** `scalar_lowering.rs:1761-1763` — in `contracts::clauses`, `ClosedScalarContractValue::Boolean(_)` and `Integer(_)` become `Proposition::Truth` (value ignored).

**Enclosing function (read):** `clauses` (`1748-1781`); `None` and `SourceCheckedGuarantee` error (`1767-1773`, read).

**Callers of `contracts::clauses` (search):** pattern `contracts::clauses(` in `omega-rust` — `attached_unit.rs` 4 sites, `unit_arguments.rs` 1, `unit_composed_control.rs` 1 (count mode: 6 total occurrences including references).

**Upward trace (read):** `lower_scalar_contract` (`attached_unit.rs:554-579`) maps each closed row through `clauses` for requires/ensures lowering. Stage 05 can publish `ClosedScalarContractValue::Boolean(*left)` from equal boolean literals (`published_facts.rs:3137-3140`, read). Digest record 070: “closed Boolean `false` clause would be lowered as no requirement; producer not traced” (read).

**What would decide:** Whether any satisfiable checked program can carry `Boolean(false)` (or unsatisfiable integer constant) in requires/clauses that passed stage 05 and therefore compiles with vacuous `Truth` obligations.

**Smallest program (plain words):** A machine entry or contract row whose closed requires row is the boolean constant false, if that row can appear in checked trees after stage 05 — lowering would emit no falsifying obligation.

## A0253

**Cited site (read):** `scalar_lowering.rs:1783-1792` — `projected_guarantees` iterates `guarantees` and drops `None` and `Some(ClosedScalarContractValue::SourceCheckedGuarantee { .. })`.

**Enclosing function (read):** `projected_guarantees` (`1783-1792`).

**Call site (read):** `attached_unit.rs:10205-10242` — filtered iterator is collected, then passed to `contracts::clauses` for normal guarantees (`10241-10242`).

**Why closed:** Rows filtered here are intentionally non-projectable; any `None` reaching `clauses` directly still fails (`1772-1773`, read), and `SourceCheckedGuarantee` in `clauses` is unsupported (`1767-1770`, read). Remaining rows still go through full `proposition` lowering.

**Input that reaches the branch:** Contract `ensures()` slice containing `None` slots or source-checked guarantee markers mixed with predicate/boolean rows.

## A0254

**Cited site (read):** `scalar_lowering.rs:1827-1831` — `erased_proof_formal_declarations` interns `checked.type_identity_text(parameter.type_identity).unwrap_or_default()`.

**Enclosing function (read):** `erased_proof_formal_declarations` (`1818-1834`). Digest also cites `2129-2167` (read): `lowered_proof_term` uses `unwrap_or_default()` for `Construction`, `Array`, and `Projection` type identities (`2129-2165`), while mathematical scalar arguments use `ok_or` instead (`2113-2117`, read).

**Callers of `erased_proof_formal_declarations` (search):** pattern `erased_proof_formal_declarations` in `omega-rust` — multiple production sites in `unit_arguments.rs`, `unit_composed_control.rs`, `attached_unit.rs`, `scalar_computations.rs` (count > 10; not individually read).

**What would decide:** Whether stage 05 guarantees `type_identity_text` is `Some` for every erased proof parameter and lowered proof term that reaches these functions; if not, empty strings enter Terminal Psi proof tables.

**Smallest program (plain words):** Not stated — would be a checked program whose proof formal or proof term carries a stale or missing type-identity id that still reaches stage-06 lowering.

## A0255

**Cited site (read):** `scalar_lowering.rs:2506-2508` — in `mathematical_carriers`, `continue` when `!carrier.domains.is_empty() || carrier.level_arity != 0`.

**Enclosing function (read):** `mathematical_carriers` (`2497-2534+`). Published carriers require `type_identity_text(...).ok_or(...)` (`2509-2511`, read).

**Callers (search):** pattern `mathematical_carriers` in `omega-rust` — definition in `scalar_lowering.rs`, production call `attached_unit.rs:17217-17218` when assembling `TerminalModule` (`17245`, read name only).

**Why closed for acceptance:** The branch omits carriers from the emitted terminal table; it does not bypass an earlier reject path. Programs that need a published carrier with domains still hit unsupported paths elsewhere if identity is missing (`2509-2511`); skipping is publication filtering, not a substitute proof check.

**Input that reaches the branch:** Checked proof evidence carrier records with nonempty `domains` or nonzero `level_arity` in `checked.facts.proof.source_evidence_carriers`.

## What I did not read

- Stage-05 proof/contract checking sufficient to close A0252 (refuted boolean requires) and A0254 (missing `type_identity_text` on proof formals/terms).
- Claim-binding destination tables for selections (would decide false-route claim necessity in A0248 beyond the lowering structure).
- Full bodies of all `erased_proof_formal_declarations` call sites and downstream terminal proof validators.
- Record 069 items 70–71 beyond the cited slices (`consume_pure_call_arguments` internals partially read).

SLICE COMPLETE
