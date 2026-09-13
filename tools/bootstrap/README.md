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

`tools/bootstrap/alpha/seed_env.sh` binds the exact size and SHA-256 of the
selected audited Alpha container from the `bootstrap/0_alpha/README.md`
retention inventory; `stamp_seed` refuses to carry any other bytes.
`tools/bootstrap/beta/artifact_env.sh` binds the exact size and SHA-256 of
`beta_compiler.beta` and `beta_compiler_bytecode.tape` and refuses to
materialize any other bytes; those identities are those audited in
`bootstrap/1_beta/AUDIT.md` and decoded by `tests/beta/compiler/root-audit.py`.
`tools/bootstrap/gamma/evaluator_env.sh` binds `gamma_evaluator.beta` and
`gamma_evaluator_bytecode.tape` the same way against
`bootstrap/2_gamma/EVALUATOR_PROFILE.md`. A digest match is an identity check,
not a proof.

`tools/bootstrap/delta/compiler_env.sh` binds the Delta edge's request entry,
source manifest, `GammaComposedV1` record, and repacked canonical closure the
same way against `bootstrap/3_delta/README.md` and `delta_compiler.composed`;
it also checks the record names the selected evaluator tape and packed
closure. `tools/bootstrap/epsilon/evaluator_env.sh` binds the
`epsilon_compiler.delta.sources` manifest and repacked evaluator closure
against `bootstrap/4_epsilon/README.md`. Both refuse before writing when the
bound bytes differ, including a changed manifest or member.

`check-chain-hygiene.sh` is the single repository-topology gate. It positively
enumerates the implemented compiler source/tape identities, inventories every
retained source, test, and bootstrap-tool owner, rejects alternate bootstrap
owners and native compiler identities above Alpha, and prevents a lower
compiler owner from reaching beyond its immediate successor.
Owner roots come from tracked and nonignored untracked files in a Git checkout,
or from physical files in a source archive. Empty directories left by a move do
not create owners. `sh tests/bootstrap/chain-hygiene.sh` checks this inventory
behavior in disposable archive and checkout fixtures.
Alpha, Beta, and Gamma may not contain subdirectories. Delta, Epsilon, and D
may retain concept-owned folders but not a redundant root `compiler/` directory.
The separate `bootstrap/proofs/` owner contains the checker and encoding theory;
it is not another language rung.

`source_closure.py MANIFEST OUTPUT` checks and concatenates explicitly ordered
source members. `DeltaSourceClosureV1` selects `.delta` members for the Epsilon
evaluator; `EpsilonSourceClosureV1` selects `.epsilon` members for Omega D. Both
use the same identity, length, digest, and canonical relative-path rows. The
manifest parent owns the complete source-file inventory: missing, extra,
wrong-language, stale, and symlinked members reject before output replacement.
Inventory traversal checks membership only; it never selects or orders inputs.

`paths.sh` exports canonical selected-owner paths. Future compiler artifact
paths may be named while absent; the topology gate does not pretend they exist.
Proof gates use `OMEGA_PATH_DERIVATION_CHECKER_SOURCES` and
`OMEGA_PATH_BETA_ENCODING_SOURCES` from that same registry.
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
| `beta/` | Trusted Beta compiler materialization with the bound-identity check, and disposable program builds. | Delete only when every caller has an equally direct canonical invocation. |
| `gamma/evaluator_env.sh` | Selected Beta-authored functional Gamma evaluator materialization. | Delete only when every caller has an equally direct canonical invocation. |
| `delta/compiler_env.sh` | Bound identity and canonical closure materialization for the Gamma-authored Delta compiler. | Delete only when every caller has an equally direct canonical invocation. |
| `epsilon/evaluator_env.sh` | Bound identity and canonical closure materialization for the Delta-authored Epsilon evaluator. | Delete only when every caller has an equally direct canonical invocation. |
| `source_closure.py` | Byte-exact assembly of the Delta-written Epsilon evaluator and Epsilon-written Omega D source closures. | Delete only when the selected lower compilers accept the same canonical member closures directly. |

The retired `verify-lattice.sh`, `test-paths.sh`, historical bootstrap-role facade,
future-artifact locators, root compiler cache, and receipt profiles had no
independent semantic or acceptance role. Git history owns them.
