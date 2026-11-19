# Compilation

A source build admits the request at 00, compiles source in Psi at 01, and
publishes a portable program at 02. A native request continues from that
program with the target's bindings at 03. Stages 04–09 turn the plan into an
image. Stage 10 installs the requested artifacts and returns their receipts.
The [connected program route](../../pipeline.md#connected-program-route) names
the input and output of each stage.

| Directory | Responsibility |
| --- | --- |
| [compiler](compiler/README.md) | Coordinates source, Build, target batches and the requested product. |
| [Psi source compiler](../../psi/compilation/pipeline/01_source-compilation/src/lib.rs) | Streams each source unit through the lexer and parser, then resolves, types, lowers and checks the program. |
| [native compiler](native-compiler/README.md) | Runs the native stages for a published program, selected target and admitted execution inputs. |
| [pipeline](pipeline/) | Owns admission, portable publication, native stages 03–09 and product publication at 10. |
| [representations](representations/) | Defines the program records, selected provider plans and physical evidence that stages pass forward. |
| [target descriptions](target-description/) | Defines target profiles, instruction sets, calling conventions, register models and layout contracts. |
| [verification](verification/) | Checks provider correspondence, installation authority, encoding, frames and published objects. |
| [products](products/README.md) | Defines compilation requests, retained results, measurements and reports. |

The Build system calls the product coordinator. Packages supply admitted
source and dependency custody. [Build system](../build-system/README.md) ·
[Package compilation](../packages/package-compilation/README.md)
