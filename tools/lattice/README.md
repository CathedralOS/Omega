# Lattice topology tooling

This directory contains only shared path plumbing and one topology gate. It
does not own a compiler stage, source closure, proof, artifact, or full-chain
runner.

```sh
sh tools/lattice/check-path-hygiene.sh
sh source/alpha/verify.sh --edge
```

The second command is the presently closed lattice floor: audited Alpha seed
behavior plus exact assembler construction. It is run directly from its owner.
There is deliberately no wrapper that relabels that one command as a complete
lattice run or prints ceremonial status for compiler edges that do not exist.
Future edge construction and admission commands stay adjacent to the compiler
artifact they produce.

`check-path-hygiene.sh` is the single repository-topology gate. It positively
enumerates the implemented compiler source/tape identities, inventories every
retained Alpha-through-Omega file, rejects alternate bootstrap owners and
native compiler identities above Alpha, and prevents a lower compiler owner
from reaching beyond its immediate successor.

`paths.sh` exports only paths consumed by present gates. Missing future
Gamma/Delta/Omega compiler sources and tapes are not represented by placeholder
variables; the files enter the positive inventory when they actually exist.
Shell and Python remain replaceable invocation plumbing and may not parse an
accepted language, lower code, manufacture proof premises, or decide
admission.

## Retention and deletion

| File | Present consumer | Deletion condition |
| --- | --- | --- |
| `paths.sh` | Alpha, Beta, Gamma, and topology gates that share exact owner paths | Absorb the remaining locators into a single cheaper canonical invocation mechanism, then delete this file and update every consumer atomically. |
| `check-path-hygiene.sh` | Repository checks for the direct-lattice ownership and retention invariant | Replace it only with one canonical gate that enforces the same positive artifact inventory, immediate-successor boundary, and file-level retention proof more economically. |

The retired `verify-lattice.sh`, `test-paths.sh`, `lattice_path` role facade,
future-artifact locators, root compiler cache, and receipt profiles had no
independent semantic or acceptance role. Git history owns them.
