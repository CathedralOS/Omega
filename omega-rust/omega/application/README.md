# Application

[cli/](cli/README.md) is Cargo package and binary `omega`, a router. It parses
a command, hands it to the one owner of that command, and turns the outcome
into output and an exit status:

| Command | Owner |
| --- | --- |
| `omega <root.omg>` (build), `--check`, `omega psi inspect` | the compiler |
| `omega run` | the interpreter runtime |
| `omega packages install`, `omega packages update`, `omega packages audit` | the package manager |

The owner calls whatever else it needs; the router holds no compile, run,
build or package workflow of its own. See the
[destination](../../pipeline.md#destination).
