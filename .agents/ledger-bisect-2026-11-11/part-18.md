# Ledger bisect part 18 (k=18, N=40)

Measured PIN: `51e8c45c793007f211d0d9baee7de16767172444`. Slice from `cursor/ledger-regressions-report-d9c6`: 17 rows. Host: linux x86_64, `OMEGA_CORPUS_JOBS=1`. Each confirmation build used a wiped `CARGO_TARGET_DIR=/tmp/bisect-target-<sha>-<tag>`.

Classification against the PIN ledger copy and the PIN witness profile: `0` matches the ledger expectation, `1` reproduces the PIN diagnostic class, `2` fails with a different diagnostic.

## PIN counts

| | Count |
| --- | ---: |
| fixed on main | 14 |
| still broken | 3 |
| slice total | 17 |

## Clusters still broken at the PIN

| Rows | Kind | PIN diagnostic | Witness | Stops matching | PIN-class commit | Status |
| ---: | --- | --- | --- | --- | --- | --- |
| 2 | pass/run rejected | cannot prove selected case for payload access `choice.payload.target`: no captured source place is available | `pass/borrows/runtime_copied_reference_conditional_structural_result_exit` | `ccd41080b4c87d58f5b924c2e1a10552510ab32c` (#1945) | `a8135b51f5eeb25d65562e90cd680b941271e576` (#1935) | bisected |
| 1 | pass/run rejected | `InvalidUnitMachinePlan` machine `width`: computed owned structural operand changes its source custody | `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit` | `22f215eb3e7d9814cb3e29f42856e56bb57e60ed` (#2078) | same commit (class matches the PIN) | bisected |

Nearest ledger-good commits on first-parent (the goods used to bound the original search):

- Cluster 1: `2476d1be2203009f1b32b8164bb6224c562c3bd5` (`compiler: preserve updated borrowed fields through selected calls (#1604)`)
- Cluster 2: `32a0a0d53c24b16707c4b95d4258cf33fa3e0018` (`psi: carry canonical case predicates through construction and matches (#556)`)

### Cluster 1 — conditional reference case payload (2 rows)

Members, both still broken at the PIN with the same diagnostic. Neither splits: both flip from `checked` to rejected at the stop-matching commit, and both flip from the lifetime diagnostic to the PIN diagnostic at the PIN-class commit.

- `pass/borrows/runtime_copied_reference_conditional_structural_result_exit` (witness)
- `pass/borrows/runtime_copied_reference_conditional_nominal_backing_exit`

**First commit where the row stops matching the ledger:** `ccd41080b4c87d58f5b924c2e1a10552510ab32c`, first-parent merge subject `psi: retain generic constructor arguments when receiving owned values (#1945)`.

Witness at that commit, fresh target, three runs: rejected, rejected, rejected. Diagnostic each time: `machine probe state entry expression type application uses undeclared lifetime argument 'storage' in View`. That is not the PIN class (`2`, not `1`).

First parent `7b6948c0ebe7b8c0d8c6f527ad9e8fdc62ffc394` (`psi: keep field rules when build scripts share application types (#1947)`), three runs: `checked`, `checked`, `checked` (codes `0,0,0`).

Nominal member once: rejected with the same lifetime diagnostic at `ccd41080`, `checked` at `7b6948c0`.

Mechanism, from `omega-rust/psi/compilation/pipeline/04_typed-trees-to-typed-trees/src/constructed_results.rs`:

```
@@ -572,8 +573,17 @@ fn retain_constructor_result_type(
         0,
     );
     let mut inferred = crate::monomorphization::StaticProposals::default();
-    for arguments in
-        identity_context::argument_tuples(program, scope, expression, owner.symbol, peers)
+    let source_arguments =
+        identity_context::application_arguments(program, literal.application_type, owner.symbol);
+    for arguments in source_arguments
+        .into_iter()
+        .chain(identity_context::argument_tuples(
+            program,
+            scope,
+            expression,
+            owner.symbol,
+            peers,
+        ))
```

The same function then keeps a prior generic application, including `lifetime_arguments: previous_lifetimes`, when those arguments agree with inference (`@@ -698,24 +708,34`).

**First commit showing the PIN diagnostic class:** `a8135b51f5eeb25d65562e90cd680b941271e576`, first-parent merge subject `psi: reduce repeated work when checking call results (#1935)`.

Witness, fresh target, three runs: codes `1,1,1`, each with `machine probe state entry statement 2: cannot prove selected case for payload access choice.payload.target: no captured source place is available`.

First parent `8357ee7572150c75e06625e8770217c06cb61761` (`tests: record passing case-payload programs (#2048)`), three runs: codes `2,2,2`. Outcome each time: rejected, diagnostic `undeclared lifetime argument 'storage' in View` (not `checked`).

Nominal member once: PIN diagnostic at `a8135b51`, lifetime diagnostic at `8357ee75`.

Mechanism, from `omega-rust/psi/compilation/validation/src/proof_contracts/contract_entailment/structural_judgment/cases.rs`:

```
@@ -211,18 +327,33 @@ impl StructuralJudge<'_> {
         map: &[(String, StructuralTerm)],
         guarantees: &mut Vec<CaseGuarantee>,
     ) {
-        use specification_calls::{SelectedCallContract, owner_parameters};
         if !self.has_exact_case_subjects() {
             return;
         }
+        let eligibility = self.case_guarantee_eligibility();
+        if eligibility.is_some_and(|table| table.contains_case(fact) == Some(false)) {
+            return;
+        }
+        self.extend_contract_case_guarantees(owner, fact, map, guarantees, eligibility);
```

### Cluster 2 — case_where width custody (1 row)

Member: `pass/facts/case_where_equality_data_predicate_nested_payload_carriage_exit`.

**Breaking commit** (stops matching and shows the PIN class): `22f215eb3e7d9814cb3e29f42856e56bb57e60ed`, first-parent merge subject `psi: preserve common fields when matching case values (#2078)`.

Witness, fresh target, three runs: codes `1,1,1`. Diagnostic each time: `terminal-artifact production failed: Lowering(InvalidUnitMachinePlan { machine: "width", reason: "computed owned structural operand changes its source custody", gap: None })`.

First parent `d25889014d5e1a5f3770dacd77612125f046e372` (`psi: answer state-owner and machine-symbol lookups from the declaration index (#2182)`), three runs: `checked`, `checked`, `checked` (codes `0,0,0`).

Mechanism: mechanism not established. The `InvalidUnitMachinePlan` reason is emitted from `owned_argument_home` in `06_checked-trees-to-lowered-psi/.../expression_bindings.rs`, and `#2078` does not edit that function or the `computed owned structural operand changes its source custody` string.

## Fixed on main at the PIN

- `pass/facts/recast_parent_restoration_through_call_result_exit`
- `pass/facts/recast_parent_restoration_through_const_name_exit`
- `pass/facts/recast_parent_restoration_through_field_exit`
- `pass/facts/recast_parent_restoration_through_local_exit`
- `pass/facts/recast_parent_restoration_through_match_join_exit`
- `pass/facts/recast_parent_restoration_through_other_machine_exit`
- `pass/facts/recast_parent_restoration_through_other_state_exit`
- `pass/facts/recast_parent_restoration_through_parameter_exit`
- `pass/facts/recast_parent_restoration_through_target_body_exit`
- `pass/providers/runtime_provider_reference_allocation_exit`
- `pass/calls/nonreturning_record_match_normal_exit`
- `pass/calls/nonreturning_record_match_stopped_exit`
- `pass/domains/qualified_record_case_predicate`
- `fail/comptime/generic_constructor_field_demand_extent_domain_violation`

## Commands

```bash
# wiped target per attempt; classify_strict.py prints 0/1/2 plus status and diagnostic
CARGO_TARGET_DIR=/tmp/bisect-target-<sha>-<tag> mbx build -p omega
CARGO_TARGET_DIR=/tmp/bisect-target-<sha>-<tag> mbx test -p compiler --no-run --test corpus_runner
# cluster 2 and the ledger break: git bisect start --first-parent <PIN> <good>
# PIN diagnostic class for cluster 1: git bisect start --first-parent <PIN> ccd41080
#   script exits 1 only when the PIN diagnostic class is reproduced
```

Job count: 1. Branch head after this update is the commit that records this file.
