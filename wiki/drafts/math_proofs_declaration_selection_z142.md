# MATH-PROOFS-DECLARATION-SELECTION — scope verification (2026-09-20, `00ed2cec7c`)

Bare mined stub at `TASKS.md:8485`. Two sibling rows already name it
verbatim; verified at `00ed2cec7c`: the verdict stands — no independent
slice exists.

## Coverage map

- **Resolved leg** — the checked-call-selection surface:
  `CHECKED-CALL-SELECTION-OCCURRENCE-MATH-PROOFS` / `PROOF-SUBJECT-
  CHECKED-CALL-ATTRIBUTION` resolved at `1fc01bb690`
  (`validation/src/proof_contracts/contract_entailment/
  specification_calls.rs` attributes the callee's selected precondition
  to the call's exact subject; `case_call_wrong_subject` /
  `case_citation_wrong_result` reject, `case_call_premises` compiles).
- **Declaration/selection surface** — `typed-trees-to-checked-trees/
  src/proof/mathematical_{signature,declarations}.rs(+)` under live
  MATH-FOUNDATION-BINDINGS claim (exp 00:09Z).
- **Sample surface** — `samples/cli/proofs/math_proofs` fenced by
  PROOF-SAMPLES-CHECKED-CALL-SELECTION.
- **Corpus surface** — `tests/omega/{pass,fail}/proofs` under
  PROOF-CERTIFICATION-BRIDGE (exp 00:51Z).
- **Kernel substrate** — PROOF-KERNEL-CORE item-level claim live.

The broader mathematical-predicate/foundations route is
PROOF-CONTRACT-MIGRATION's connected implementation elaborating to
PROOF-KERNEL-CORE's term model (per MATHEMATICAL-FOUNDATIONS-REAL's
resolved row).

## Outcome

No code change — record only. Coordinator may fold the stub into the
CHECKED-CALL-SELECTION-OCCURRENCE-MATH-PROOFS /
PROOF-CONTRACT-MIGRATION cluster.
