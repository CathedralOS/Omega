# Beta compiler tests

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `selfhost.sh` | Reconstructs the canonical assembler tape from `assembler.beta`. | Delete only when stronger exact reconstruction replaces it. |
| `asm_ref.py`, `asm-diamond.sh` | Independent assembly relation and differential gate. | Delete together when checked assembly correspondence subsumes them. |
| `register-label-regression.sh` | Pins register/label lexical boundaries. | Delete when generated checked vectors cover every case. |
| `examples/` | Small executable encoding fixtures. | Delete a fixture only when an equally direct generated control subsumes it. |
