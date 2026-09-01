# Delta interpreter tests

| Retained files | Role | Deletion condition |
| --- | --- | --- |
| `interp.gamma` | Temporary bounded executable Delta oracle. | Delete when the direct compiler subsumes every retained semantic role. |
| `test-interp.sh`, `test-interp-arena.sh` | Semantic and fixed-arena controls for the oracle. | Delete with the oracle or when stronger controls subsume them. |
