# Compiler

[`compile(request)`](compiler.rs) compiles one program. The request names the artifact the caller wants. Source and generated files arrive together for that compilation.

| Requested artifact | Input | Work |
| --- | --- | --- |
| Psi | Stage source | Check the source and return its published Psi. |
| Native image | Stage source and target | Publish Psi in memory and lower it for the target. |
| Native image | Authorized Psi and target | Lower the supplied Psi for the target. |

The [source pipeline](source-to-psi/source_to_psi.rs) and the [native pipeline](psi-to-native/psi_to_native.rs) meet at [Psi](psi/). For a saved program, the receiving host decodes the file and authorizes the exact artifact before it requests native code.

Each pipeline keeps its own [source program forms](source-to-psi/pipeline/representations/) or [native program forms](psi-to-native/pipeline/representations/). Their shared [Psi format](psi/) is the handoff between them.

A [project build](build/build.rs) calls this API for each program in its stage chain. The build driver runs planning programs in the interpreter and returns the final requested artifact.

This working-tree prototype shows the proposed interfaces. The CLI, concrete V2 build host, and existing workspace manifests still need integration.
