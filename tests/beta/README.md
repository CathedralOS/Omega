# Beta tests

`compiler/` tests the canonical Beta-text-to-Alpha-tape implementation without
placing host runners or fixtures beside the language and compiler artifacts.

| Retained child | Role | Deletion condition |
| --- | --- | --- |
| `compiler/` | Beta assembler reconstruction, differential, and lexical tests. | Delete only when stronger checked coverage subsumes the same relations. |
