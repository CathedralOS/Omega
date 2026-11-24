# Atlas packet unit_025: rewrite candidate validator

Status: in progress (early receipt).

Scope: [record unit_025](../../records/unit_025.md) and its area-12 digest lines in
[12-native-representations.md](../../digests/12-native-representations.md). The unit is one file,
`source-rust/subsystems/compiler/psi-to-native/representations/abstract-operations/optimization-unit-semantics/src/candidates.rs`
(cited below as `CAND:<line>`). Baseline `24ad2700685e`; the file is byte-identical in length
(10,799 lines) at `a0152d26`, and every cited line still holds the code the record names.

| Original claim | Code path | Outcome | Evidence |
| --- | --- | --- | --- |
| Record §9 item 8; digest line 176 (plan returns empty) | `state_specialization::plan_dispatch`, `CAND:9876-9878`; caller `CAND:9255` | Intended (fails closed) | When every incoming edge resolves, the plan is cleared and the caller rejects the empty plan. |
| Record §9 item 9 | `ProofCheckValidationRoute::for_rule`, `CAND:6584-6597`; router `CAND:6892-6908` | Intended (fails closed) | An unknown rule yields `None`, and the router returns `CandidateAnalysisContractMismatch`. |
| Record §9 item 10 | `validate_psi_rewrite_candidate`, `CAND:10466` | Intended (fails closed) | A proof-check rule goes to the proof-check router regardless of patch kind; each routed validator unpacks its own patch variant with `candidate_patch!` and refuses any other. |
| Record §9 item 11 | `independent_cse_expression`, `CAND:3187` | Intended (fails closed) | `CompatiblePolicy` yields `None`; its callers use `independent_compatible_policy_scalar_leader` instead (`CAND:4127-4130`). |
| Record §9 item 7 | `validate_unreachable_private_machines_candidate`, `CAND:1957` | Intended | Pruning whole machines changes no block, so the validator requires empty `affected_blocks` and substitutions. |
