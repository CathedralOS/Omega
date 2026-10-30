# Psi compilation

This domain turns Omega source into Terminal Psi. It owns source meaning and
its evidence, not package acquisition, command policy or native realization.
The [Psi facade](driver/src/lib.rs) owns source-stage sequencing.
The [product coordinator](../../omega/compilation/driver/src/compiler.rs) owns
request sharing and target products; the application owns Build execution.

| Reader's question | Entrance and responsibility |
| --- | --- |
| Which stage transforms this program? | [pipeline/](pipeline/README.md): numbered source-to-Terminal transforms, each with its own operation entrance |
| What program does a stage consume or produce? | [representations/](representations/README.md): independent durable IR packages; Terminal codec and verification live beside their subject |
| Is this source declaration, call or value legal? | [program_validation.rs](checking/src/program_validation.rs): reusable judgments and retained analyses, not construction of the complete checked program |
| Where is the checked program constructed? | [stage 04 checking.rs](pipeline/04_typed-trees-to-checked-trees/src/checking.rs): orders specialization, validation, facts, flow, ownership and proof checks |
| Where are proof obligations produced and discharged? | [proof/lib.rs](proof/src/lib.rs): source obligation construction and search; [admission](proof/admission/README.md) independently checks supplied mathematical evidence |
| Where do evaluated values change the program? | [build_time_evaluation.rs](compile-time/src/build_time_evaluation.rs): evaluation admission and source/typed result elaboration using the separately owned checked interpreter |
| Where is a Terminal artifact independently checked? | [Terminal verification](representations/terminal-psi/verification/README.md): reconstructs obligations from the portable artifact, without trusting its producer's decisions |

Directory containment is not dependency permission. Representations and shared
semantic libraries remain independent packages. Interpreters may consume those
libraries, but must not depend on producing stages in their production dependency
graph. Compile-time elaboration can invoke an interpreter; it cannot be placed
inside that interpreter merely because it evaluates a machine.

The Psi facade's [Terminal production sequence](driver/terminal-production/README.md)
coordinates stages 05–07. Stage 05 validates checked permission ledgers at its
own entrance; stage 07 owns publication and source-correspondence judgments.
The coordinator carries those results without deciding language rules. It is
not another Psi stage; see the [pipeline contract](../../pipeline.md).
