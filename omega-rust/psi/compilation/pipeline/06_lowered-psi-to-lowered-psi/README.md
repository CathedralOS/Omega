# Lowered Psi optimization

Start at [psi_optimization.rs](src/psi_optimization.rs). It validates the complete
input, executes selected passes in canonical order, validates the output, and
records input/output semantic and proof identities. Empty selection follows the
same validation path. Only this owner can construct `PsiOptimizationStageResult`,
the result accepted by Terminal publication.

Each pass owns its rewrite and independent validation. Shared identity-retention
policy lives beside the coordinator: [ranking coverage](src/retained_identities/ranked_coverage.rs)
identifies evidence-covered blocks and values; [proof values](src/retained_identities/proof_values.rs)
retains identities named by propositions and crash routes. Copy propagation,
global value numbering and dead scalar elimination consume the relevant policy
directly. Shared [errors](src/optimization_error.rs) do not depend on dispatch.

The [stage tests](tests/stage.rs) cover validated identity, malformed inputs and
canonical selection order. Neighboring integration tests exercise each pass's
rewrites and retained proof, debug and source-custody companions.