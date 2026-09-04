# Bootstrap-chain tests

Only tests whose subject spans more than one language rung live here. Tests of
one accepted language or compiler live under `tests/<language>/` instead.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `alpha-beta-edge.sh` | Checks the selected Alpha seed, Alpha conformance, exact trusted Beta compiler reconstruction, and finite root audit. | Delete only when a stronger Alpha-to-Beta edge gate subsumes all observations. |
