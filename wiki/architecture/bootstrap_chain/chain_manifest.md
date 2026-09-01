# Bootstrap chain manifest

[Chain overview](bootstrap_chain.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

This ledger names only the selected chain. Missing artifacts remain explicit.

| Subject | Implementation | Canonical output | Current state |
| --- | --- | --- | --- |
| Alpha VM | audited native source/listing per host | Alpha execution | macOS arm64 and Windows x86-64 seeds retained; conformance passes |
| Beta evaluator | directly audited Alpha tape | Beta evaluation | absent |
| Gamma compiler | Beta source | `gamma_compiler_bytecode.tape` | source and tape absent |
| Delta compiler | Gamma source | `delta_compiler_bytecode.tape` | incomplete `source/delta/compiler/delta_compiler.gamma`; tape absent |
| Omega `D` | Delta source | `omega0_compiler_bytecode.tape` | incomplete `source/omega/omega_compiler.delta`; tape absent |
| Omega `C` | Omega source | `omega_compiler_bytecode.tape` | incomplete `source/omega/{build.omg,main.omg}`; tape absent |

No later fixed point repairs an open earlier row. Every row must stand on its
exact source, exact tape, written semantics, observation profile, checked
relation, and disclosed admissions.

## Alpha floor

| Target | Artifact | SHA-256 |
| --- | --- | --- |
| Darwin arm64 | `source/alpha/alpha_arm64_macos` | `5844f295e3ab843e1819aae0ca47d41ad99cef5e2193a5abee64e630b41c304c` |
| Windows x86-64 | `source/alpha/alpha_x64_windows.exe` | `ccce78bbef7cb5a538d4fb0e350a1c646233d179074e65b4ac7cca98c4a4a6f7` |

The off-chain Alpha Tape assembler is not a manifest row. Its 29,747-byte
source reconstructs its 6,816-byte tape exactly and may assist construction or
inspection without supplying compiler authority.

## Retention rule

Repository-owned bootstrap material must specify a selected rung, implement its
single immediate edge, reconstruct that edge, or exercise one named failure of
that exact subject. Generality, historical investment, self-hosting, and
compatibility are not retention reasons. Git is the archive.
