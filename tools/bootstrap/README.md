# Bootstrap-chain tooling

This directory owns replaceable invocation, tape materialization, exact
artifact construction, and the chain-layout gate. It owns no language
semantics, compiler source, proof, canonical artifact, or test corpus.

```sh
sh tools/bootstrap/check-chain-hygiene.sh
sh tests/bootstrap/alpha-beta-edge.sh --edge
```

The second command is the presently closed bootstrap floor: audited Alpha seed
behavior plus exact trusted Beta compiler reconstruction. It is run from the
cross-rung test owner.
There is deliberately no wrapper that relabels that one command as a complete
chain run or prints ceremonial status for compiler edges that do not exist.

`check-chain-hygiene.sh` is the single repository-topology gate. It positively
enumerates the implemented compiler source/tape identities, inventories every
retained source, test, and bootstrap-tool owner, rejects alternate bootstrap
owners and native compiler identities above Alpha, and prevents a lower
compiler owner from reaching beyond its immediate successor.

`paths.sh` exports canonical selected-owner paths. Future compiler artifact
paths may be named while absent; the topology gate does not pretend they exist.
Shell and Python remain replaceable invocation plumbing and may not parse an
accepted language, lower code, manufacture proof premises, or decide
admission.

The component directories hold sourceable materializers or deliberate artifact
construction commands. Tests consume them from `tests/`; canonical artifacts
remain under their language owner.

## Retention and deletion

| Child/files | Present consumer | Deletion condition |
| --- | --- | --- |
| `paths.sh` | Alpha, Beta, Gamma, Delta, Epsilon, Omega, and topology gates that share exact owner paths | Absorb the remaining locators into a single cheaper canonical invocation mechanism, then delete this file and update every consumer atomically. |
| `check-chain-hygiene.sh` | Repository checks for direct-chain ownership, source purity, and retention | Replace it only with one canonical gate enforcing the same positive inventory and immediate-successor boundary more economically. |
| `alpha/` | Alpha seed selection and tape stamping used by current tests and tools. | Delete only when every caller has an equally direct canonical invocation. |
| `beta/` | Trusted Beta compiler materialization and disposable program builds. | Delete only when every caller has an equally direct canonical invocation. |
| `gamma/evaluator_env.sh` | Selected Beta-authored functional Gamma evaluator materialization. | Delete only when every caller has an equally direct canonical invocation. |
| `gamma/artifact_env.sh` | Downgraded concatenative Gamma compiler materialization for retained comparison gates. | Delete with the nested concatenative bootstrap evidence. |
| `delta/` | Downgraded concatenative-Gamma-written Delta compiler composition for retained comparison gates. | Delete with the nested Delta bootstrap compiler. |

The retired `verify-lattice.sh`, `test-paths.sh`, historical bootstrap-role facade,
future-artifact locators, root compiler cache, and receipt profiles had no
independent semantic or acceptance role. Git history owns them.
