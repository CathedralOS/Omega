# Beta tests

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `root-audit.py` | Independently binds, partitions, decodes, and checks reachability of the admitted compiler source/tape pair. | Delete only with a stronger checked correspondence over the same finite subject. |
| `reconstruction.sh` | Reconstructs the canonical Beta compiler tape from `beta_compiler.beta`. | Delete only when stronger exact reconstruction replaces it. |
| `beta_ref.py`, `compiler-diamond.sh` | Independent assembly relation and differential gate. | Delete together when checked assembly correspondence subsumes them. |
| `register-address-regression.sh` | Pins register/address syntax, numeric control flow, bounds, and status-gated publication. | Delete when generated checked vectors cover every case. |
| `word-prefix.py`, `word-prefix.sh` | Pins the shared `0x` prefix at assertions, late assertions, control operands, and data words for every printable initial token byte. | Delete only with equivalent exact status and stream-prefix coverage. |
| `examples/` | Small executable encoding fixtures. | Delete a fixture only when an equally direct generated control subsumes it. |

Run `sh tests/beta/compiler/word-prefix.sh` from the repository root on macOS
arm64, or from Git Bash on Windows x64 with Python 3 available as `python3`.
The Alpha-to-Beta edge gate includes it. The Python helper supplies literal
inputs and checks exact process status, stdout, and stderr; it does not assemble
source or produce a trusted artifact. A rejected raw stdout prefix is not a
published tape. The macOS-only `register-address-regression.sh` additionally
checks the build wrapper preserves existing artifacts after late failures.
