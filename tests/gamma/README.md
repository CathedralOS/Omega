# Gamma tests

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `compiler/` | Canonical Gamma compiler surface and artifact-structure tests. | Delete only when stronger checked source-to-tape validation subsumes them. |
| `reference/` | Independent parser, interpreter, and differential diagnostics. | Delete when direct operational refinement subsumes every retained shape. |
