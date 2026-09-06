# Bootstrap chain manifest

[Chain overview](bootstrap_chain.md) | [Active work](../../../TASKS_BOOTSTRAP.md)

This ledger names only the selected chain. Missing artifacts remain explicit.

| Subject | Implementation | Canonical output | Current state |
| --- | --- | --- | --- |
| Alpha VM | audited native source/listing per host | Alpha execution | AlphaBootstrapV4 macOS arm64 and Windows x86-64 seeds retained; 33 macOS conformance checks and native-source reconstruction pass; Windows runtime unvalidated |
| Beta compiler | admitted Alpha tape with addressed Beta self-reconstruction source | `beta_compiler_bytecode.tape` | 1,792-byte tape retained; finite root audit, reconstruction, and differential gates pass |
| Gamma evaluator | Beta source | `gamma_evaluator_bytecode.tape` | selected 1,632-line / 46,484-byte typed scalar/effect evaluator; 8,355-byte tape; AlphaBootstrapV4 heap admits 40,265,318 pairs, with a separate full-capacity/adjacent slow gate; exact-name indexed function lookup, static validation, 100,000-step proper tails, exact bounded outcomes, provenance-tagged immutable pairs, marked buffered application results, profile-owned runtime failures, readable character literals, and self-augmentation pass |
| Delta compiler | Gamma application to canonical Gamma | exact `GammaComposedV1` identity and staged receipts | 3,023-line composition of the canonical entry and 58 shared manifested members; source envelope, lexical admission, retained balanced-tree parsing, and structural grammar precede global identity census, signatures, retained-node body typing, schema, complete expanded Gamma planning, capture-safe body-height normalization, and generic plan serialization; downstream numeric checks consume lexical admission; scalar/Bytes/nominal checking and emission execute; strict DCREQ, source-byte (3), lexical/structural syntax (4), integer range (5), duplicate identity (6/7/8), local/pattern conflicts and semantic type/name/arity/match failures (9–18), missing-main (19), and schema (20) failures publish owned DCOUT; census owns D30 authored function-row Incomplete(4), limit/requested 32,768/32,769, global constructor-row Incomplete(3), and total type-row Incomplete(2), both limit/requested 65,536/65,537, with duplicate lookup before provision and two builtin types included in the total; grammar owns expression parse-depth Incomplete(8), limit/requested 1,024/1,025; normalization extracts over-height fragments under Gamma's 255-list body bound without changing fitting-program receipts; helper count, runtime contexts, storage, other compiler resource/internal outcomes, full generated-profile admission, and edge closure remain open |
| Epsilon evaluator | Delta source executed through the selected lower route | interpreted Epsilon behavior | incomplete closure selected by `bootstrap/epsilon/compiler/epsilon_compiler.delta.sources`; current checking, value calls and returns, nested receiver places, state transfers, immutable views, all Console operations, and sum defaults/construction/transitions execute through the lower route; constructor fields establish immediately after each argument, and byte-range failure suppresses later arguments while retaining prior output; remaining checking, application resource guarantees, and exact final composition remain open |
| Omega `D` | Manifested Epsilon source closure interpreted by the Epsilon evaluator | `omega0_compiler_bytecode.tape` for Omega C | incomplete `bootstrap/omega/{compiler.epsilon.sources,compiler/}`; D and its `alpha_bootstrap` target are incomplete; tape absent |
| Omega `C` | Omega source compiled by D, then by omega0 | `omega_compiler_bytecode.tape` | incomplete `source/omega/{build.omg,main.omg}`; `alpha_bootstrap` root bound; tapes absent |

The former concatenative Gamma evaluator/compiler is not a manifest row. No
later fixed point repairs an open earlier row. Every row must stand on its
exact source, exact tape, written semantics, observation profile, checked
relation, and disclosed admissions.

## Alpha and Beta floor

| Target | Artifact | SHA-256 |
| --- | --- | --- |
| Darwin arm64 | `bootstrap/alpha/alpha_arm64_macos` | `4e1ca500d5c4a93eb3f487988e26bf63006c31ec398fdbafccc84a0a1224fac9` |
| Windows x86-64 | `bootstrap/alpha/alpha_x64_windows.exe` | `4fb6c5fc99e93d8e18f7247855d4a58a82dedf8705d3d24ea82e5b59d166edde` |

The trusted Beta compiler is a manifest row. Its 12,640-byte addressed Beta
source reconstructs its admitted 1,792-byte Alpha tape exactly. The Alpha VM and that
compiler tape form the current executable bootstrap floor.

## Retention rule

Repository-owned bootstrap material must specify a selected rung, implement its
single immediate edge, reconstruct that edge, or exercise one named failure of
that exact subject. Generality, historical investment, self-hosting, and
compatibility are not retention reasons. Git is the archive.
