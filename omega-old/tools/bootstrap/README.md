# Bootstrap-chain tooling

This directory holds replaceable scripts that run the bootstrap chain, write
tapes, build exact artifacts and check the chain's layout. It holds no
language semantics, compiler source, proof, canonical artifact or test corpus.
These temporary host scripts fall under the
[whole-chain minimization contract](../../bootstrap/MINIMIZATION.md).

```sh
sh tools/bootstrap/check-chain-hygiene.sh
sh tests/bootstrap/alpha-beta-edge.sh --edge
```

The second command, kept with the cross-rung tests, checks the finished part
of the chain: the audited Alpha seed's behavior and an exact rebuild of the
trusted Beta compiler. No wrapper calls it a full chain run or prints status
for compiler edges that do not exist.

`tools/bootstrap/alpha/seed_env.sh` pins the exact size and SHA-256 of the
audited Alpha container chosen in `bootstrap/interpreted/0_alpha/README.md`;
`stamp_seed` refuses any other bytes.
`tools/bootstrap/beta/artifact_env.sh` pins the exact size and SHA-256 of
`beta_compiler.beta` and `beta_compiler_bytecode.tape` and refuses any
other bytes; these identities are audited in
`bootstrap/interpreted/1_beta/AUDIT.md` and decoded by `tests/beta/compiler/root-audit.py`.
`tools/bootstrap/gamma/evaluator_env.sh` pins `gamma_evaluator.beta` and
`gamma_evaluator_bytecode.tape` the same way against
`bootstrap/interpreted/2_gamma/EVALUATOR_PROFILE.md`. A matching digest checks
identity; it is not a proof.

`tools/bootstrap/delta/compiler_env.sh` pins the Delta edge's request entry,
source manifest, ordered support-member manifest, `GammaComposedV2` record,
repacked canonical closure and packed support section the same way against
`bootstrap/interpreted/3_delta/README.md` and `delta_compiler.composed`. It
checks that the record names the chosen evaluator tape, packed closure
and packed support section. It also pins the staged-compiler gate's
`development_driver.gamma` diagnostic entry, packed on top of the pinned
member bytes; `tests/bootstrap/delta-identity.sh` tests its refusals.
`tools/bootstrap/epsilon/evaluator_env.sh` pins the
`epsilon_compiler.delta.sources` manifest and repacked evaluator closure
against `bootstrap/interpreted/4_epsilon/README.md`. Both refuse before writing if
the bytes differ from the pins, including a changed manifest or member. The Epsilon file also pins the canonical `execution_driver.delta`
slice adapter and the evaluator receipt identity that the pinned Delta edge
rebuilds from the pinned evaluator, driver and support section;
`tests/bootstrap/epsilon-identity.sh` checks the driver and receipt refusals
and every consuming gate's record.

`tools/bootstrap/omega/compiler_env.sh` pins the
`omega_compiler.epsilon.sources` manifest and repacked D closure against
`bootstrap/interpreted/5_omega/README.md` the same way. It also pins the
entries the omega-parser, omega-outcome, omega-request and omega-executable
gates append on top of the pinned member bytes, and
the omega-request gate's canonical sealed request fixture; each gate's
README records its own. The same file pins D's OCREQ request entry,
`program.omg`: the canonical 180-byte Omega source the request compiles,
SHA-256 `9cc2d065cf33e3a9de241d061f407665892f7171a431936a0e5ad9e5d3eeea85`.
It is a separate input fed to the pinned customer entry as sealed bytes,
not a manifest member.
`tools/bootstrap/proofs/sources_env.sh` pins the
`implementation.gamma.sources` and `theory.gamma.sources` manifests and their
repacked member closures against `bootstrap/proofs/checker/README.md` and
`bootstrap/proofs/beta_encoding/README.md`, plus the derivation gates'
diagnostic prefix entries packed on top of the pinned member bytes; each
gate's README records its own.

The fast `tests/bootstrap/*-identity.sh` gates check the exact bytes written
and that corrupted or truncated bytes are refused, against the pins. They
check the records of the programs that use the bytes, but never grep
documentation for copies of the hashes or byte counts: a pin repeated in
prose proves nothing new. The cited audit and profile documents explain each
artifact and its obligations.

`check-chain-hygiene.sh` is the only repository-layout check. It lists the
implemented compiler source and tape identities, lists
every kept source, test and bootstrap-tool directory, rejects other bootstrap
directories and native compiler identities above Alpha, stops a lower
compiler from reaching past its immediate successor, and checks that
manifest members and bootstrap step files omit the Rust producer.
It finds directories from tracked and untracked non-ignored files in a Git
checkout, or from the files in a source archive. Empty
directories left by a move do not count. `sh tests/bootstrap/chain-hygiene.sh` checks this inventory
behavior in disposable archive and checkout fixtures.
Alpha, Beta and Gamma may not contain subdirectories. Delta, Epsilon and D
may keep one folder per concept, but not a redundant root `compiler/`
directory. The separate `bootstrap/proofs/` directory holds the checker and
encoding theory; it is not another language rung.

`mbx xtask source-closure MANIFEST OUTPUT` checks source members, in the order
the manifest gives, and concatenates them. `GammaSourceClosureV1` selects `.gamma` members for the proof
closures; `DeltaSourceClosureV1` selects `.delta` members for the Epsilon
evaluator; `EpsilonSourceClosureV1` selects `.epsilon` members for Omega D. All use
the same rows: identity, length, digest and canonical relative path. The
manifest's directory must list every source file in it: a missing, extra,
wrong-language, stale or symlinked member is rejected before the output is
replaced. `--prefix ENTRY` puts one non-member entry file, a marked entry or
a gate's diagnostic header, before the packed members. It must be a regular file with the manifest's source suffix; a wrong
language, symlink, non-regular path or forbidden bytes are rejected first. Walking the directory only checks
membership, never choosing or ordering inputs.

`paths.sh` exports the canonical path of each chosen directory. It may name
future compiler artifacts before they exist; the layout check does not treat
them as present. Proof gates take
`OMEGA_PATH_DERIVATION_CHECKER_SOURCES` and
`OMEGA_PATH_BETA_ENCODING_SOURCES` from it.
Shell scripts only invoke things and can be replaced. Source-closure packing,
the Alpha Windows listing forge and Gamma evaluator runs are xtask commands.
None of them may parse an accepted language, lower code, invent proof
premises or decide admission.

The subdirectories hold sourceable scripts that write artifacts, or explicit
artifact build commands. Tests in `tests/` use them; canonical
artifacts stay in their language's directory.

## Retention and deletion

| Child/files | Present consumer | Deletion condition |
| --- | --- | --- |
| `paths.sh` | Alpha, Beta, Gamma, Delta, Epsilon, Omega, and topology gates that share exact owner paths | Absorb the remaining locators into a single cheaper canonical invocation mechanism, then delete this file and update every consumer atomically. |
| `check-chain-hygiene.sh` | Repository checks for direct-chain ownership, source purity, and retention | Replace it only with one canonical gate enforcing the same positive inventory and immediate-successor boundary more economically. |
| `alpha/` | Alpha seed selection and tape stamping used by current tests and tools. | Delete only when every caller has an equally direct canonical invocation. |
| `beta/` | Trusted Beta compiler materialization with the bound-identity check, and disposable program builds. | Delete only when every caller has an equally direct canonical invocation. |
| `gamma/evaluator_env.sh` | Selected Beta-authored functional Gamma evaluator materialization. | Delete only when every caller has an equally direct canonical invocation. |
| `delta/compiler_env.sh` | Bound identity and canonical closure materialization for the Gamma-authored Delta compiler, plus the bound staged-compiler development driver entry. | Delete only when every caller has an equally direct canonical invocation. |
| `epsilon/evaluator_env.sh` | Bound identity and canonical closure materialization for the Delta-authored Epsilon evaluator. | Delete only when every caller has an equally direct canonical invocation. |
| `omega/compiler_env.sh` | Bound identity and canonical closure materialization for the Epsilon-written Omega compiler D. | Delete only when every caller has an equally direct canonical invocation. |
| `proofs/sources_env.sh` | Bound identity and canonical member-closure materialization for the derivation checker and Beta encoding theory. | Delete only when every caller has an equally direct canonical invocation. |
| `mbx xtask source-closure` | Byte-exact assembly of the Gamma proof, Delta-written Epsilon evaluator, and Epsilon-written Omega D source closures. | Delete only when the selected lower compilers accept the same canonical member closures directly. |

The retired `verify-lattice.sh`, `test-paths.sh`, old bootstrap-role
wrapper, future-artifact locators, root compiler cache and receipt profiles
checked nothing of their own. They remain in Git history.
