# Psi compilation

This domain turns Omega source into Terminal Psi. It owns source meaning
and its evidence; it does not acquire packages, set command policy or
produce native code. The [Psi facade](source-compiler/src/lib.rs) runs the
source stages in order. The [product coordinator](../../omega/compilation/compiler/src/target_batch.rs)
shares requests and builds target products.
The application runs Build.

| Reader's question | Where to look |
| --- | --- |
| Which stage transforms this program? | [pipeline/](pipeline/README.md): numbered source-to-Terminal stages, each with its own entry function |
| What program does a stage consume or produce? | [representations/](representations/README.md): one package per IR; the Terminal codec sits beside it |
| Is this source declaration, call or value legal? | [06 lowered-form checking](pipeline/06_terminal-psi-to-terminal-psi/src/verdict.rs): the only checker |
| Where is the checked program constructed? | [05 structural lowering](pipeline/05_typed-trees-to-terminal-psi/src/compilation.rs) builds the Terminal input; stage 06 judges it |
| Where are proof obligations produced and discharged? | [prove.rs](pipeline/06_terminal-psi-to-terminal-psi/src/prove.rs): the checker's prover; [admission](proof-admission/README.md) checks supplied mathematical evidence |
| Where do evaluated values change the program? | [build-time-evaluation/src/lib.rs](build-time-evaluation/src/lib.rs): admits evaluations and writes results into source and typed trees, using the separately owned checked interpreter |
| Where is a Terminal artifact independently checked? | [Terminal verification](representations/terminal-psi/verification/README.md): rebuilds obligations from the portable artifact without trusting its producer |

Sharing a directory does not permit a dependency. Representations and shared
semantic libraries stay separate packages. Interpreters may use those
libraries, but their production dependencies must not include a producing
stage. Compile-time elaboration may call an interpreter but does not belong
inside one because it evaluates a machine.

The [Terminal production sequence](terminal-production/README.md) runs the
lowered-form checker (stage 06), then the accepted-source factory publishes
the checked module and binds it to its source records. The sequence decides
no language rule and is not another Psi stage; see the
[pipeline contract](../../pipeline.md).
