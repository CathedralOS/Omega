# Bootstrap chain manifest

[Chain overview](bootstrap_chain.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

This ledger names only the selected chain. Missing artifacts remain explicit.

| Subject | Implementation | Canonical output | Current state |
| --- | --- | --- | --- |
| Alpha VM | audited native source/listing per host | Alpha execution | macOS arm64 and Windows x86-64 seeds retained; conformance passes |
| Beta compiler | admitted Alpha tape with addressed Beta self-reconstruction source | `beta_compiler_bytecode.tape` | 1,792-byte tape retained; finite root audit, reconstruction, and differential gates pass |
| Gamma evaluator | Beta source | `gamma_evaluator_bytecode.tape` | selected 1,325-line typed scalar/effect evaluator; 6,934-byte tape; static validation, 100,000-step proper tails, exact bounded outcomes, provenance-tagged immutable pairs, profile-owned arithmetic traps, readable character literals, and self-augmentation pass |
| Delta compiler | Gamma source transformer to canonical Gamma | exact `GammaComposedV1` identity and staged receipts | 1,022-line stage enforces the source-byte envelope, lowers arbitrary-field recursive ADTs through immutable pairs, and validates global declaration order/uniqueness; List, Bytes-rope, and 3,001-function witnesses pass; normative Bytes and complete checking remain open |
| Epsilon compiler | Delta source to canonical Delta, then selected lower compilers | `epsilon_compiler_bytecode.tape` | incomplete `source/epsilon/compiler/epsilon_compiler.delta`; tape absent |
| Omega `D` | Epsilon source to canonical Epsilon, then selected lower compilers | `omega0_compiler_bytecode.tape` | incomplete `source/omega/omega_compiler.epsilon`; tape absent |
| Omega `C` | Omega source to canonical Epsilon, then selected lower compilers | `omega_compiler_bytecode.tape` | incomplete `source/omega/{build.omg,main.omg}`; tape absent |

The former concatenative Gamma evaluator/compiler is not a manifest row. No
later fixed point repairs an open earlier row. Every row must stand on its
exact source, exact tape, written semantics, observation profile, checked
relation, and disclosed admissions.

## Alpha and Beta floor

| Target | Artifact | SHA-256 |
| --- | --- | --- |
| Darwin arm64 | `source/alpha/alpha_arm64_macos` | `5844f295e3ab843e1819aae0ca47d41ad99cef5e2193a5abee64e630b41c304c` |
| Windows x86-64 | `source/alpha/alpha_x64_windows.exe` | `ccce78bbef7cb5a538d4fb0e350a1c646233d179074e65b4ac7cca98c4a4a6f7` |

The trusted Beta compiler is a manifest row. Its 12,640-byte addressed Beta
source reconstructs its admitted 1,792-byte Alpha tape exactly. The Alpha VM and that
compiler tape form the current executable bootstrap floor.

## Retention rule

Repository-owned bootstrap material must specify a selected rung, implement its
single immediate edge, reconstruct that edge, or exercise one named failure of
that exact subject. Generality, historical investment, self-hosting, and
compatibility are not retention reasons. Git is the archive.
