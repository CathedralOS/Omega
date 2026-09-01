# Delta tests

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `compiler/` | Frontend, profile, and emitter-substrate tests for the Gamma-written compiler. | Delete only when the completed checked compiler edge subsumes them. |
| `interpreter/` | Bounded executable oracle and its resource tests. | Delete when the direct compiler covers every retained semantic surface. |
| `reference/` | Independent evaluator and differential corpus. | Delete when the checked direct edge subsumes every named diagnostic role. |
