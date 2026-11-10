slice_0014

| entry id | file:line | verdict | one-line reason |
|---|---|---|---|
| A0101 | `execution_terminal_unit_state_graph.rs:2000-2002`, `2566-2568`, `3021-3023`; `borrow.rs:501-512` | CLOSED | `is_image_storage_root` only classifies string-literal expression roots (read `borrow.rs:505-512`); early exits are intentional for immutable image storage, not missing checks. |
| A0102 | `execution_terminal_unit_state_graph.rs:3568-3580` | CLOSED | Unrestricted multiplicity and no-code affine owners are explicitly exempt from exit-disposition rows per module contract at `3458-3464` (read). |
| A0103 | `execution_terminal_unit_state_graph.rs:3895-3897`, `3905-3909`; `published_facts.rs:7666` | CLOSED | Empty natural ranks apply only when there is no implementation witness and no runtime rank components; termination is fixed before plans via `TerminationFacts` (read). |
| A0104 | `execution_terminal_unit_state_graph.rs:8481-8487`, `8704-8713`, `9251-9268` | REACHABLE | A Unit-returning state whose tail is only erased-dispatch `When`/`Always` transitions can get `ReturnUnit` without lowering any arm’s reachable operations. |
| A0105 | `execution_terminal_unit_state_graph.rs:7362-7373` | REACHABLE | Failed `add_reference_type` with no dynamic selector still emits a `ProjectedLoan` transfer using default `carrier_type_identity` instead of refusing the edge. |
| A0106 | `execution_terminal_unit_structural_scalar_store.rs:3051-3088`; `execution_terminal_unit_control_statement_sequence.rs:9245-9280` | CLOSED | Skipped store-sequence assignments are replanned in the statement loop via `assignments::plan` and the atomic-assignment arm (read). |
| A0107 | `execution_terminal_unit_types.rs:4786-4872`, `1093`, `4653-4659` | UNDECIDED | Structural entry memberships are dropped on `continue` when roots/routing are missing; scalar/entry parity was not traced end-to-end. |
| A0108 | `flow.rs:742-748` | UNDECIDED | Unresolved borrow-call callee symbols skip union edges only; whether that changes program acceptance was not traced through flow consumers. |

### A0101

1. Read `borrow::accesses::is_image_storage_root` in full (`borrow.rs:501-513`, read): returns true only for `PlaceRoot::Expression` that is a valid `ExpressionNode::String(_)` (read).
2. Read `loan_backing::local_backing_binding` in full (`execution_terminal_unit_state_graph.rs:1992-2057`, read): returns `Ok(None)` at `2000-2001` when image root.
3. Read `loan_backing::owned_parent_source` in full (`2558-2590`, read): same early `Ok(None)` at `2566-2567`.
4. Read `contents_at` in full (`3010-3117`, read): returns `true` at `3021-3022` for image roots without walking loan paths.
5. Search `\blocal_backing_binding\b` in `omega-rust`: 2 hits (read), both in `execution_terminal_unit_state_graph.rs` (definition + `retained_storage` closure at `1912`).
6. Search `\bowned_parent_source\b`: 6 hits in `execution_terminal_unit_state_graph.rs` (read); read caller `retained_storage` (`1798-1944`, read), which only needs backing metadata for captured loans already admitted in flow/borrow facts.
7. Search `is_image_storage_root` in stage-05 crate: 4 uses in `execution_terminal_unit_state_graph.rs` plus definition in `borrow.rs` (read).

**Input reaching the branch:** A place root that is a string-literal expression in state-loan backing or loan-independence walks (`retained_storage` and the `contents_at` walker).

**Verdict: CLOSED.** Check: `borrow.rs:501-512` restricts the accepting branches to immutable string image roots, matching the comments at `495-499` (read). `retained_storage` still requires an exact captured call and operand (`1812-1825`, read) before backing is consulted.

**Unsure:** None for image-root classification; did not read every `contents_at` caller beyond the loan-backings module (`grep` count 3).

### A0102

1. Read `local_results::validate` in full (`3492-3595`, read).
2. Read `local_results::permits_disposal` (`3469-3490`, read) and `operation_custody` closure using it (`4727-4747`, read).
3. Search `permits_disposal`: multiple call sites in `operation_custody` module only (read list from grep).

**Input reaching the branch:** A structural result operation with `Multiplicity::Unrestricted` (`3568-3569`), or an affine result classified `no_code_completion_owner` (`4641-4648`, read) with borrow-free contents (`3571-3578`).

**Verdict: CLOSED.** Check: module comment `3458-3464` (read) states no-code completions do not require exit disposition rows; unrestricted results at `3568-3569` (read) align with unrestricted transfer accounting at `4625-4626` and `4688-4689` (read), which skips disposal constraints.

**Unsure:** Did not read every downstream consumer of `permits_disposal` beyond the `operation_custody::admit` paths around `4838-5029`.

### A0103

1. Read `ranking::natural_rank_plans_for_fused` in full (`3884-3936` plus continuation through `3934-3936`, read).
2. Read `ranking::complete_witness_ranks` (`4140-4179`, read).
3. Read `build_traced` caller chain at `9658-9673` (read) and `execution.rs:build_checked_terminal_machine_selections` (`1350-1377`, read).
4. Read `published_facts.rs` termination build (`7660-7674`, read).
5. Search `\bnatural_rank_plans\b` in `omega-rust`: 5 hits (`state_graph.rs`, `execution.rs`, `execution_scalar_computations.rs`, read).

**Input reaching the branch:** Fused machine body with no `implementation_witness` on any fused owner and an admitted rank plan whose `runtime_components` span is empty (`3892-3896`, read).

**Verdict: CLOSED.** Check: early `Some((Vec::new(), Vec::new()))` at `3895-3896` (read) applies only with empty runtime components; `complete_witness_ranks` skips owners without witness (`4147-4148`, read); `TerminationFacts` are built before check facts consume `rank_arrivals` (`published_facts.rs:7660-7674`, read). `build_checked_terminal_machine_selections` marks unsupported only when witness exists and `natural_rank_plans` returns `None` (`1363-1364`, read), not when it returns empty vectors.

**Unsure:** Did not read full `crate::checks::termination::build_checked_termination_plan` implementation (path not opened).

### A0104

1. Read `terminator::build` tail matchers at `8480-8487`, `8704-8713`, and `8924-8925` (read).
2. Read `erased_proof_completion` and `transition_on_erased_dispatch_subject` in full (`9251-9309`, read).
3. Search `\berased_proof_completion\b`: 3 hits, all in `execution_terminal_unit_state_graph.rs` (read).

**Input reaching the branch:** `CheckedControlResultPlan::Unit` state whose matched transition tail has every guard on an erased dispatch subject (`8482-8483`, `8705-8708`, read), including `Always` tails per `9280-9281` (read).

**Verdict: REACHABLE.** Smallest program shape: a Unit machine state ending in a two-arm or multi-arm transition tail where each `When` only names an erased dispatch subject (or `Always`), while at least one arm’s unreachable-from-plan body would perform a side effect the planner does not serialize; the planner still returns `ReturnUnit` via `erased_proof_completion` (`8485-8486`, `8711-8712`, read). Comment at `9251-9253` (read) documents arms are not executed in this completion path.

**Unsure:** Did not prove whether typed/flow stages always reject side effects hidden behind erased-dispatch-only tails before stage-05 planning.

### A0105

1. Read `projected_loan_transfer` in full (`7291-7400`, read), especially `7362-7373`.
2. Read alternate stored-reference path at `6703-6706` (read), which requires `add_reference_type` `Some` (stricter).
3. Read shape retention `execution_terminal_unit.rs:7944-7952` (read).
4. Search `\bprojected_loan_transfer\b`: 3 hits (definition + 2 call sites at `6655`, `6735`, read).

**Input reaching the branch:** Projected loan edge transfer where `byte_sequence_carrier` is `BoundedView` without dynamic selectors, or `add_reference_type` returns `None` without dynamic selectors (`7371-7372`, read).

**Verdict: REACHABLE.** Smallest program shape: a state-graph edge that projects a loan onto a parameter type where `shapes.add_reference_type` fails but the path has no runtime/computed index segments; planning still returns `Some(transfer)` with `carrier_type_identity: Default::default()` (`7369-7399`, read). Invalid identities are not added to `retained_type_identities` (`7949-7951`, read) but the transfer remains in the composed plan.

**Unsure:** Did not trace stage-06 lowering for edges carrying invalid/default `carrier_type_identity`.

### A0106

1. Read `build_structural_scalar_field_store_sequence_traced` loop (`3017-3090`, read).
2. Read `execution_terminal_unit_control::assignment_type` (`5627-5648`, read).
3. Read statement-sequence assignment dispatch (`9225-9290`, read) and store-sequence caller (`9134-9146`, read).
4. Search `build_structural_scalar_field_store_sequence`: 2 hits (`structural_scalar_store.rs`, `control_statement_sequence.rs`, read).

**Input reaching the branch:** Post-construction assignment where `atomic_assignment_carrier` is set (`3051-3052`), `assignment_type` is `Some` (`3054-3055`), `plan_assignment` fails but value is a `Call` or has a structural value root (`3075-3085`, read).

**Verdict: CLOSED.** Route: main statement walk replans assignments via `assignments::plan` at `9265-9280` (read); atomics via dedicated arm `9245-9263` (read). The store sequence’s `continue` skips duplicate store planning, not whole-statement admission; failure path `3087-3088` returns `None` when no alternate applies (read).

**Unsure:** Did not read full `assignments::plan` body to list every assignment kind it covers.

### A0107

1. Read `fold_parameter_entry_domains` in full (`4786-4871`, read).
2. Read `boundary_domain_requirements` membership loop including `1093` (`1042-1094`, read).
3. Read `fold_boundary_result_domains` skip on missing `domain_definition` (`4653-4659`, read).
4. Search `\bfold_parameter_entry_domains\b`: 4 hits (`types.rs` + 3 in `execution_terminal_unit_calls.rs`, read); read callers `6059-6061`, `6078-6080` (read).

**Input reaching the branch:** Authored `requires … in <domain>` membership on an entry/formal where `single_segment_name_symbol` fails, structural parameter row is missing, domain is neither routed nor vacuous (`4820-4846`, read), or scalar boundary contract loop hits `1093` (read).

**Verdict: UNDECIDED.** Structural lane silently `continue`s; entry text at `4777-4778` (read) claims bare requires are not erased, but several guards skip registration. Scalar lane catch-up (`1093` and scalar parameters in `boundary_domain_requirements`) was not traced into values/published establishment.

**Unsure:** Whether skipped structural memberships are enforced later in scalar qualification publishing or proof facts.

### A0108

1. Read `machine_groups` in full (`697-798` excerpt through callee union loop, read).
2. Read `build_flow_facts` caller at `347-350` (read).
3. Search `\bmachine_groups\b` in `omega-rust`: 2 hits in `flow.rs` (read).

**Input reaching the branch:** A borrow-facts call row whose `target_symbol` is absent from `state_locations` (`746-748`, read), so the loop `continue`s and adds no `(caller, callee)` union.

**Verdict: UNDECIDED.** Input is a borrow call fact with unresolved state target symbol during flow grouping. Did not trace whether incorrect partitioning changes fact convergence such that a program that should fail check-facts instead passes.

**Unsure:** Whether borrow-fact construction already rejects unresolved `target_symbol` calls before flow grouping runs.

### What I did not read

- Full `assignments::plan` implementation in `execution_terminal_unit_control_statement_sequence.rs`.
- `crate::checks::termination::build_checked_termination_plan` source file.
- Stage-06 lowering validators for `ProjectedLoan` transfers with invalid `carrier_type_identity`.
- Downstream flow consumers of `machine_groups` output beyond `build_groups` entry (`flow.rs:354`).
- Full `operation_custody::admit` past the `permits_disposal` call sites listed in grep.
- Entire `contents_at` call graph (only 3 internal references found).

SLICE COMPLETE
