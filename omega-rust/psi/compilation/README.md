# Psi compilation

Stage 01 turns admitted source into a checked Terminal program. It owns the
source verdict, the evidence for that verdict and the logical providers chosen
for the requested roots. Stage 02 seals that result for later use. [Source
compiler](pipeline/01_source-compilation/README.md) · [Portable publication](../../omega/compilation/pipeline/02_portable-publication/src/lib.rs)

| Reader's question | Where to look |
| --- | --- |
| How does a file enter the compiler? | Each worker lexes and immediately parses one activated file, then reuses its token storage. [Source stream](pipeline/01_source-compilation/src/source_parsing.rs) |
| What do the operations pass forward? | [Source operation map](pipeline/README.md) and [program representations](representations/README.md) |
| Who decides whether the program is legal? | [Structural lowering](pipeline/01_source-compilation/src/operations/lowering/src/compilation.rs) constructs the checker input. The [lowered-form checker](pipeline/01_source-compilation/src/operations/checking/src/verdict.rs) issues the verdict. |
| Who produces and checks mathematical evidence? | The checker calls its [prover](pipeline/01_source-compilation/src/operations/checking/src/prove.rs). [Proof admission](proof-admission/README.md) checks the supplied evidence. |
| Who evaluates Build and source requests? | [Source evaluation](pipeline/01_source-compilation/src/evaluation/README.md) owns the requests, their accounting and the values returned to preparation. |
| How does the program leave the source owner? | The [accepted-source factory](pipeline/01_source-compilation/src/operations/production/src/accepted.rs) binds the verdict to its source. The [Terminal codec](representations/terminal-psi/codec/README.md) encodes and reads the canonical program. |

Interpreters consume published program representations. Source evaluation may
invoke an interpreter while stage 01 owns the active source request. The
[connected pipeline](../../pipeline.md#connected-program-route) follows the
result through target planning and publication.
