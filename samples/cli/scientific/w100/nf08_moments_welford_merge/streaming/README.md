# NF08 streaming

Run `omega --check main.omg`, `omega run --target linux_x86_64 main.omg`, and
`omega inspect-terminal --machine Main::main --target linux_x86_64 main.omg`
from this directory. Native success exits 0; failures exit 81.

Oracles and tolerances are frozen in ../ORACLES.md before the first run.

NF01 must provide package metadata declaring the `numerics` alias used here.
