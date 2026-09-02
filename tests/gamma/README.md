# Gamma tests

This directory owns executable gates for the Gamma language and its direct Alpha
evaluator.

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `evaluator-slice.sh` | Builds and exercises Gamma words, stacks, cells, calls, tail transfers, arithmetic, I/O bounds, and terminal statuses. | Delete when subsumed by a stronger complete Gamma gate. |
| `evaluator-reconstruction.sh` | Requires the Gamma reconstructor and trusted Beta compiler to emit the same evaluator tape from canonical source. | Delete if a stronger non-embedded Gamma fixed point replaces it. |
| `compiler-fixed-point.sh` | Requires the Gamma-written compiler to reproduce its native tape and agree across seeded/native Delta0 compilation. | Delete if the experiment is rejected or a canonical Gamma compiler supersedes it. |
| `state-machine-customer.sh`, `fixtures/delta0_compiler.gamma` | Proves Gamma can implement a direct addressed-CFG compiler and execute its exact Alpha result. | Delete when the canonical Delta compiler subsumes this customer. |