# Proof-checker tests

The authoritative checker source and tape remain under `source/alpha/checker/`.
This directory owns only its executable tests and independent reference.

| Retained child/files | Role | Deletion condition |
| --- | --- | --- |
| `reconstruct-artifact.sh` | Reconstructs and smoke-checks the exact committed checker tape. | Delete only when a stronger exact reconstruction replaces it. |
| `gates/` | Calculus, soundness, and semantic differential gates. | Delete individual gates only when stronger checks subsume them. |
| `corpus/` | Generated proof discriminator inputs. | Delete when stronger generated or formal coverage subsumes them. |
| `reference/` | Independent host-language checker. | Delete when a stronger independent formal check replaces it. |
