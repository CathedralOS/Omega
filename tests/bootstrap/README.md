# Bootstrap-chain tests

Only tests whose subject spans more than one language rung live here. Tests of
one accepted language or compiler live under `tests/<language>/` instead.

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `chain-hygiene.sh` | Checks cross-rung owner inventory in source archives and Git checkouts, including empty relocation leftovers and actual alternate owners. | Delete only when another topology regression test covers both forms of source custody. |
| `alpha-beta-edge.sh` | Checks the selected Alpha seed, Alpha conformance, exact trusted Beta compiler reconstruction, and finite root audit. | Delete only when a stronger Alpha-to-Beta edge gate subsumes all observations. |
| `epsilon-source-closure.sh` | Checks ordered multi-member Epsilon source materialization, exact current-D reproduction, and malformed manifest rejection. | Delete only when the selected Epsilon compiler consumes and verifies the same member manifest directly. |
