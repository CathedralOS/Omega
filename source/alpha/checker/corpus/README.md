# Checker diagnostic corpus

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `fuzz/` | `check-ref-fuzz.py` generates deterministic complete-rule comparisons for the single independent checker diamond. | Delete with `gates/check-ref-diamond.sh` or replace atomically with its successor generator. |

This service intentionally owns no theorem library. Mathematical examples and
proof-search fixtures do not strengthen the direct compiler lattice and are not
retained here.
