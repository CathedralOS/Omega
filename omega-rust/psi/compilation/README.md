# Psi compilation

Psi turns admitted files into a checked portable program through eight
numbered stages. Its source coordinator schedules the active files, roots
and logical provider choices. [Stage map](pipeline/README.md) · [Source coordinator](pipeline/source-compilation/README.md)

| Reader's question | Where to look |
| --- | --- |
| How does a file enter the compiler? | Each worker lexes and immediately parses one activated file, then reuses its token storage. [Source stream](pipeline/source-compilation/src/source_parsing.rs) |
| What do the operations pass forward? | [Source operation map](pipeline/README.md) and [program representations](representations/README.md) |
| Who decides whether the program is legal? | [Structural lowering](pipeline/05_typed-trees-to-terminal-psi/src/compilation.rs) constructs the checker input. The [lowered-form checker](pipeline/06_terminal-psi-to-checked-terminal-psi/src/verdict.rs) issues the verdict. |
| Who produces and checks mathematical evidence? | The checker calls its [prover](pipeline/06_terminal-psi-to-checked-terminal-psi/src/prove.rs). [Proof admission](proof-admission/README.md) checks the supplied evidence. |
| Who evaluates Build and source requests? | [Source evaluation](pipeline/source-compilation/src/evaluation/README.md) owns the requests, their accounting and the values returned to preparation. |
| How does the program leave the source owner? | The [accepted-source factory](pipeline/07_checked-terminal-psi-to-published-psi/src/accepted.rs) binds the verdict to its source. The [Terminal codec](representations/terminal-psi/codec/README.md) encodes and reads the canonical program. |

Interpreters consume published program representations. Source evaluation may
invoke an interpreter while the coordinator owns the active source request. The
[connected pipeline](../../pipeline.md#connected-program-route) follows the
result through target planning and publication.
