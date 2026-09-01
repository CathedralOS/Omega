# Checker implementations

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `gamma/` | `check.gamma` is the authoritative checker source, including D40's closed `FloatMeaning` correspondence term and carrier-specific equality; `eq.gamma` is the independent definitional side of the one retained operational seam. | `check.gamma` changes only with an atomic artifact rebuild; delete `eq.gamma` if the seam is formally subsumed or retired. |

No proof search, source conversion, corpus, or policy implementation belongs
here. The independent host-language checker and its gate live under
`tests/proof-checker/`. Additional language-hosted copies are not retained
merely to increase implementation count.
