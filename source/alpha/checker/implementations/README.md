# Checker implementations

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `gamma/` | `check.gamma` is the authoritative checker source, including D40's closed `FloatMeaning` correspondence term and carrier-specific equality; `eq.gamma` is the independent definitional side of the one retained operational seam. | `check.gamma` changes only with an atomic artifact rebuild; delete `eq.gamma` if the seam is formally subsumed or retired. |
| `reference/` | `check_ref.py` is the one untrusted, independently written complete checker, including the D40 correspondence tuple validation, used only by `gates/check-ref-diamond.sh`. | Delete when a stronger independent formal check replaces the diamond. |

No proof search, source conversion, corpus, or policy implementation belongs
here. Additional language-hosted copies are not retained merely to increase
implementation count.
