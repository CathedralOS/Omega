# Gamma tests

This directory owns executable gates for the Gamma language and its direct Alpha
evaluator.

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `evaluator-slice.sh` | Builds and exercises the currently implemented evaluator slice, including terminal status classification. | Delete when subsumed by a complete Gamma evaluator conformance gate. |
| `state-machine-customer.sh`, `fixtures/delta0_compiler.gamma` | Proves Gamma can implement a direct addressed-CFG compiler and execute its exact Alpha result. | Delete when the canonical Delta compiler subsumes this customer. |