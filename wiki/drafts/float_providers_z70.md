# FLOAT-PROVIDERS — re-verification (2026-09-21, `7d03d489e3`, linux x86-64)

Swarm wave w9 / zergling-z70. Claim ticket `a3337fe9` on
`omega-rust/psi/semantics/validation/src/proof_contracts/float_projection_bindings/semantic_values.rs`
+ this draft (TASKS.md multiply-fenced; row stamp deferred).

## Witnesses re-run at tip

- `cargo nextest run -p compiler --test float_semantic_applications
  --no-fail-fast` (RUST_MIN_STACK=67108864): **6/6 PASS** in 35.5s —
  `closed_semantic_application_proves_real_core_source_contract`,
  `closed_semantic_application_refutes_false_real_core_source_contract`,
  `closed_float_evaluation_does_not_override_authored_equality`,
  `open_semantic_application_remains_unproved`,
  `produced_artifact_verifies_authored_float_meaning_ensures`,
  `symbolic_float_applications_validate_all_operands_after_source_removal`.
- `OMEGA_FAIL_CANARY_FILTER=float/float_semantics_lookalike_grants_no_primitive
  cargo nextest run -p compiler --test canary_suite
  proof_and_float_suites::proof_and_domain_canaries::fail_canaries_reject_with_expected_diagnostic_fragment`:
  **1/1 PASS** — the row's acceptance reject leg still rejects.

## What is settled

The source-check leg is complete: `closed_float_meaning_equality`
(`semantic_values.rs`) evaluates only exact toolchain
projections/applications through `kernel_discharge`, refuses open terms
(exhausted work budget or unknown operand supplies no fact), preserves the
selected equality's exact carrier via `has_builtin_spelled_expression_meaning`,
and is consumed by `contract_entailment/structural_judgment.rs:1651`. The
`exact_toolchain_float_format_const` sealed-source custody route (Name +
StructLiteral against `canonical_value_encoding`, BINARY32/BINARY64
distinguished leaf-by-leaf) is intact.

## What remains open (unchanged by this verification)

The row's accepted next leg is a three-hop chain that does not fit in one
fenced claim:

1. Retain the authored equality as a typed contract proposition with its
   exact owner/use-site obligation (checked side).
2. Admit it through the owner's scalar-contract lowering —
   `checked-trees-to-lowered-psi/src/scalar_graph/scalar_contracts.rs::
   covered_requires` still admits no non-reflexive meaning clause. **Fenced
   to RC-REPOSITORY (`Zergling-109 / rc-repository-uefi-globs`, expires
   2026-09-21T14:39Z) — claim probe returned exit 2 on this slot.**
3. Discharge through ordinary `CertificateDerived` production and the
   independent replay route.

Until (2) lands, work in (1)/(3) is unverifiable dead admission — no producer
can reach it. No code change on this slot; adjudication stands.
