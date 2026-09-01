# Beta tests

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `reconstruction.sh` | Reconstructs the canonical Beta compiler tape from `beta_compiler.beta`. | Delete only when stronger exact reconstruction replaces it. |
| `beta_ref.py`, `compiler-diamond.sh` | Independent assembly relation and differential gate. | Delete together when checked assembly correspondence subsumes them. |
| `register-label-regression.sh` | Pins register/label lexical boundaries. | Delete when generated checked vectors cover every case. |
| `examples/` | Small executable encoding fixtures. | Delete a fixture only when an equally direct generated control subsumes it. |
