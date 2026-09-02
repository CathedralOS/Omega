# Gamma tests

This directory owns executable gates for the Gamma language and its direct Alpha
evaluator.

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `evaluator-slice.sh` | Builds and exercises Gamma words, stacks, cells, calls, tail transfers, arithmetic, I/O bounds, and terminal statuses. | Delete when subsumed by a stronger complete Gamma gate. |
| `evaluator-reconstruction.sh`, `evaluator_reconstructor.gamma` | Requires the test-owned Gamma reconstructor and trusted Beta compiler to emit the same evaluator tape from canonical source. | Delete if a stronger non-embedded Gamma fixed point replaces it. |
| `compiler-fixed-point.sh` | Requires evaluator/native Gamma executions to reproduce the canonical Beta receipt and Beta to reconstruct the selected compiler tape; the direct comparator must agree on Delta0. | Replace only atomically with a stronger selected Gamma compiler reconstruction. |
| `gamma-to-beta-experiment/run.sh`, `fixtures/gamma_to_beta_surface.gamma`, `fixtures/gamma_to_beta_surface.beta` | Conformance gate for interpreted/native Gamma-to-Beta compilation, retained readable Beta, Beta assembly, direct-comparator tape equality, execution, malformed no-output behavior, exact identities, near-limit composition, and adjacent Alpha oversize refusal. | Delete only when stronger checked source-to-source correspondence subsumes it. |
| `state-machine-customer.sh`, `fixtures/delta0_compiler.gamma` | Proves Gamma can implement a direct addressed-CFG compiler and execute its exact Alpha result. | Delete when the canonical Delta compiler subsumes this customer. |