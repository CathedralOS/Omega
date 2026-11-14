# Psi compilation

This domain turns Omega source into Terminal Psi. It owns source meaning and
its evidence, not package acquisition, command policy or native realization.
The [Psi facade](psi-compiler/src/lib.rs) owns source-stage sequencing.
The [product coordinator](../../omega/compilation/compiler/src/target_batch.rs) owns
request sharing and target products; the application owns Build execution.

| Reader's question | Entrance and responsibility |
| --- | --- |
| Which stage transforms this program? | [pipeline/](pipeline/README.md): numbered source-to-Terminal transforms, each with its own operation entrance |
| What program does a stage consume or produce? | [representations/](representations/README.md): independent durable IR packages; Terminal codec and verification live beside their subject |
| Is this source declaration, call or value legal? | [program_validation.rs](checking/src/program_validation.rs): reusable judgments and retained analyses, not construction of the complete checked program |
| Where is the checked program constructed? | [Structure elaboration](pipeline/structure-elaboration/README.md) and [lowered discharge](pipeline/lowered-discharge/README.md): the lowered-form checker |
| Where are proof obligations produced and discharged? | [proof/lib.rs](proof/src/lib.rs): source obligation construction and search; [admission](proof/admission/README.md) independently checks supplied mathematical evidence |
| Where do evaluated values change the program? | [build-time-evaluation/src/lib.rs](build-time-evaluation/src/lib.rs): evaluation admission and source/typed result elaboration using the separately owned checked interpreter |
| Where is a Terminal artifact independently checked? | [Terminal verification](representations/terminal-psi/verification/README.md): reconstructs obligations from the portable artifact, without trusting its producer's decisions |

Directory containment is not dependency permission. Representations and shared
semantic libraries remain independent packages. Interpreters may consume those
libraries, but must not depend on producing stages in their production dependency
graph. Compile-time elaboration can invoke an interpreter; it cannot be placed
inside that interpreter merely because it evaluates a machine.

The Psi facade's [Terminal production sequence](psi-compiler/terminal-production/README.md)
coordinates Psi 06–08. Psi 06 validates checked permission ledgers at its
own entrance; Psi 08 owns publication and source-correspondence judgments.
The coordinator carries those results without deciding language rules. It is
not another Psi stage; see the [pipeline contract](../../pipeline.md).
