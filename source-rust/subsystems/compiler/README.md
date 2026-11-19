# Compiler

[`compile(request)`](compiler.rs) compiles one program. The request names the artifact the caller wants. Source and generated files arrive together for that compilation.

| Requested artifact | Input | Work |
| --- | --- | --- |
| Psi | Stage source | Check the source and return its published Psi. |
| Interpreter runtime | Stage source or authorized Psi, target and entry memory | Admit the selected entry and providers for the Psi engine. |
| Native image | Stage source and target | Publish Psi in memory and lower it for the target. |
| Native image | Authorized Psi and target | Lower the supplied Psi for the target. |

The [source pipeline](source-to-psi/source_to_psi.rs) and the [native pipeline](psi-to-native/psi_to_native.rs) meet at [Psi](psi/). For a source program, its checked publication supplies the exact target’s reachable provider choices, completed Build bindings and the behavior exclusions admitted in that same source stage. The host adds accepted package permissions and any reviewed installation assumptions for that deployment. For a saved program, the receiving host decodes the file and authorizes the exact artifact before it requests runtime provisions or native code.

Each pipeline keeps its own [source program forms](source-to-psi/pipeline/representations/) or [native program forms](psi-to-native/pipeline/representations/). Their shared [Psi format](psi/) is the handoff between them.

An authored Build runs as a program. The package host supplies the capture and grants. The source compiler resolves the selected Build stage, judges the invocation and publishes the checked program. Build execution runs it in the interpreter, services its host calls and returns its plan. [Build source compilation](source-to-psi/pipeline/build_invocation.rs) · [Build execution](build/execution/src/lib.rs)

For a Product, planning turns the returned declaration and package graph into a compiler request. The source driver publishes Psi before target admission. Saved Psi enters at authorization and joins the same native driver. [Product inputs](build/planning/product.rs) · [Source compilation](source-to-psi/source_to_psi.rs) · [Saved and source native inputs](psi-to-native/pipeline/prepare/published_application.rs)
