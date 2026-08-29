# Checker implementations

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `beta/` | `check.beta` is the authoritative checker source; `eq.beta` is the independent definitional side of the one retained operational seam. | `check.beta` changes only with an atomic artifact rebuild; delete `eq.beta` if the seam is formally subsumed or retired. |
| `reference/` | `check_ref.py` is the one untrusted, independently written complete checker used only by `gates/check-ref-diamond.sh`. | Delete when a stronger independent formal check replaces the diamond. |

No proof search, source conversion, corpus, or policy implementation belongs
here. Additional language-hosted copies are not retained merely to increase
implementation count.
