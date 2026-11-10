slice_0076

| entry id | file:line | verdict | one-line reason |
|----------|-----------|---------|-----------------|
| A0585 | value_custody_core.rs:8618-8636 | REACHABLE | Flat per-state statement scan treats any assignment as establishing a ref local for all following statements, ignoring control flow including guarded transitions. |
| A0586 | value_custody_core.rs:9551 | REACHABLE | Search step budget exhaustion returns None and `validate_prepared_inline_storage` only diagnoses when `runtime_inline_array_extent` returns Some. |
| A0587 | value_custody_core.rs:1705,1720 | CLOSED | Unresolved `Named`/`Generic` names get type-reference validation errors in the same `validate()` pass before acceptance; Affine default is fail-safe for claim counting. |
| A0588 | terminal-installation/src/lib.rs:593-600 | REACHABLE | Missing target realization row uses `continue`, omitting the application from `AdmittedTargetLayouts` without `MissingMember` (enum variant unused here, read). |
| A0589 | terminal-installation/src/lib.rs:653 | REACHABLE | Duplicate-boundary check compares only to the last retained lifecycle after `continue` skips, so same boundary non-adjacent rows can both be kept. |
| A0590 | evaluation_and_synchronous_calls.rs:910 | REACHABLE | `authority` `None` skips per-machine `require_selection`; product compile paths pass `package_inputs: None` into `prepare_boundary_calling_plans` (read). |
| A0591 | evaluation_and_synchronous_calls.rs:928-933 | REACHABLE | Edge custody `require_selection` runs only when both `declaration(...)` calls succeed; `Err` from header lookup skips the check entirely. |
| A0592 | provider_and_target_installation.rs:709-715 | REACHABLE | Source carrier domains with no target match are not errors unless listed in `source_required`; extra provider theories can pass admission. |

### A0585

1. Read in full: `reference_local_establishment::validate` at `omega-rust/psi/compilation/validation/src/value_custody_core.rs:8604-8644` (read); helpers `whole_local_target` (8646-8657) and `is_reference` start (8659+, partial).
2. Search: `rg 'reference_local_establishment::validate' omega-rust` → 2 hits (definition + one call).
3. Caller read: `program_validation.rs:4275-4282` inside per-state loop before per-statement validation (read); did not read full `validate()` coordinator.
4. Input to accepting branch: any state's flat `statement_nodes` list; on `StatementNode::Assignment` targeting whole local `symbol`, `unestablished.retain` drops `symbol` when `stored == Some(symbol)` regardless of prior `Transition` with `When` guard (8618-8636, read).
5. Entry accuracy: description mentions guarded transition; code is control-flow-insensitive for all assignments in the flat list, not transition-nested bodies (transitions hold patterns/targets, not assignment statements, read `calls_and_control_flow.rs:3716-3724`).
6. Verdict **REACHABLE**: a state can declare a ref local without initializer, take an unconditional transition that always exits, then contain a later assignment that only the checker sees as establishing the local while real executions never run that assignment before a later use on another path—the checker may accept where establishment never occurs on viable paths.

### A0586

1. Read: `Search::ty` at `value_custody_core.rs:9546-9630` (partial to 9629); caller `runtime_inline_array_extent` (9528-9537) and `validate_prepared_inline_storage` (9723-9771, read).
2. Search: `rg 'runtime_inline_array_extent' omega-rust` → 4 hits (def, use at 9762, re-exports in validation `lib.rs` and monomorphization).
3. Caller chain: `monomorphization.rs:2682` runs validator and fails compile on diagnostics (read); no alternate error when extent is `None`.
4. Input: types whose inline fixed-array extent search consumes 4096 `remaining` steps; `checked_sub(1)?` at 9551 returns `None` for the whole search (read).
5. Verdict **REACHABLE**: a program whose fixed-array extent depends on a runtime value but whose type graph forces the 4096-step cap to exhaust before finding that extent should be rejected; `None` is treated as “no runtime inline extent” and produces no diagnostic (9762-9767 only errors on `Some`).

### A0587

1. Read in full: `frontier_type_multiplicity` and enclosing `type_multiplicity` in `claim_frontier` at `value_custody_core.rs:1623-1722` (read).
2. Search: `rg '\bfrontier_type_multiplicity\b' omega-rust` → 7 hits, all inside `value_custody_core.rs` `claim_frontier` (inferred production use only via `append_linear_claim_frontier`).
3. Caller upward: `append_linear_claim_frontier` → exported `linear_claim_frontier` / `declared_claim_frontier` → structural and expression validators (grep hits; did not read every consumer in full).
4. Input: `TypeReferenceNode::Named` or `Generic` with invalid/missing definition lookup and non-primitive name; `unwrap_or(Multiplicity::Affine)` at 1705 and 1720 (read).
5. Parallel default: `TypedTrees::type_multiplicity` at `declaration_tables_and_index.rs:6154,6173` also uses `Affine` (read)—same accepting semantics at the program table layer.
6. Verdict **CLOSED**: `program_validation` runs `reference_local_establishment` and, in the same `validate()` pass, per-statement `local_data_statements::validate_type_reference` / `type_references::validate_type_reference_handle` (program_validation.rs:3166,3195, inferred from dispatch 3814-3817); unknown type names produce diagnostics (e.g. type_references wire path 6764-6768, read). Acceptance requires an empty diagnostic batch at `validate_program` (3946-3956). Affine understates linear claims and is conservative for obligations, not permissive acceptance of bad programs.

### A0588

1. Read: `admit_target_layouts` at `terminal-installation/src/lib.rs:573-672` (focus 591-600, 640-671, read).
2. Search: `rg 'admit_target_layouts' omega-rust` → 17 hits (production: native-compiler, build-settings, artifact_admission, embedding, etc.).
3. Callers not traced in full; admission success does not require every `target_layout_applications` entry to appear in output.
4. Input: artifact module with layout application lacking `realizations` row for `selections.target()`; `continue` at 598-599 (read).
5. Entry accurate: `TargetLayoutSelectionError::MissingMember` exists (531-558) but is not returned on this path (read); grep shows constructions only under ti-prov for a different selection error.
6. Verdict **REACHABLE**: an artifact can be admitted with a layout application silently dropped, leaving unresolved target-layout record material for consumers that assume full closure.

### A0589

1. Read: lifecycle loop `terminal-installation/src/lib.rs:641-664` (read); `validate_access_lifecycle_plans` ordering check at `terminal-psi/semantics/src/access.rs:521-529` (read, encoding-time ordering of full module list).
2. Search: `rg 'lifecycles\.last' omega-rust` → this site only.
3. Input: `access_lifecycle_plans` iteration after per-plan `continue` when target filter mismatches (643-651); duplicate detection only against `lifecycles.last()` (653-657).
4. Verdict **REACHABLE**: if two plans share `boundary` but are separated by a skipped plan (target mismatch `continue`), both can be retained despite `InvalidLifecycle` intent—ordering invariant at encode time does not fix adjacency after filtering.

### A0590

1. Read in full: `admit_compiler_evaluation` `evaluation_and_synchronous_calls.rs:830-974` (partial 880-922 full, read).
2. Search: `rg 'admit_compiler_evaluation' omega-rust` → 3 files (def, `calling_policy_plans.rs:85`, tests).
3. Caller read: `prepare_boundary_calling_plans` sets `authority: package_inputs.map(...)` (`plan_computation.rs:199-202`); `evaluate` passes `self.authority.as_deref()` (290). Production `execution_settlement.rs:401-405` forwards `package_inputs: Option` (read). `source_build.rs:72` uses `package_inputs: None` (read).
4. Input: cross-package `facts.selections` with `authority == None`; inner block 910-921 never runs (read).
5. Verdict **REACHABLE**: compile-time calling-policy admission can succeed for cross-package declaration selections when compilation runs without package inputs, skipping authority checks the API documents for `Some(authority)`.

### A0591

1. Read: edge loop `evaluation_and_synchronous_calls.rs:924-936` (read); `declaration` at 1052-1060 requires retained checked facts header (read).
2. Search: same as A0590 call graph; edge custody is nested inside `admit_compiler_evaluation` only.
3. Input: invocation edge with `authority: Some(...)` where `declaration(scope, edge.source.machine())` or target side returns `Err`; `let (Ok(source), Ok(target))` pattern skips custody (928-931, read).
4. Verdict **REACHABLE**: cross-package custody is not enforced for edges whose machine lacks compiler-evaluation header facts, even when authority is present.

### A0592

1. Read in full: `admit_memberships` `provider_and_target_installation.rs:692-731` (read); callers `admit_input_memberships` / `admit_result_memberships` (632-687, partial read).
2. Search: `rg '\badmit_memberships\b' omega-rust` → 3 hits, all in `provider_and_target_installation.rs`.
3. Input: each source-domain on carrier (701-714); unmatched domains produce no binding and no error; only `source_required` domains must map at 716-724 (read).
4. Verdict **REACHABLE**: provider artifact can carry an extra membership theory on the paired structural type that the receiver does not require and is not in `source_required`, yet component provider admission returns `Ok(bindings)`.

## What I did not read

- Full bodies of all `admit_target_layouts` and `admit_compiler_evaluation` production callers beyond cited call sites.
- Entire `validate()` prologue roster and every consumer of `linear_claim_frontier`.
- Terminal artifact producers' invariants for layout realizations and lifecycle plan ordering at emission time (beyond `validate_access_lifecycle_plans` header, read).
- Whether corpus programs exist that witness each REACHABLE case (no runtime execution per instructions).

SLICE COMPLETE
