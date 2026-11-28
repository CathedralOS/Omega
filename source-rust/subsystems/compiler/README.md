# Compiler

[`compile(request)`](compiler.rs) compiles one program. The request names the artifact the caller wants. Source and generated files arrive together for that compilation.

| Requested artifact | Input | Work |
| --- | --- | --- |
| Psi | Stage source | Check the source and return its published Psi. |
| Interpreter runtime | Stage source or authorized Psi, target and entry memory | Admit the selected entry and providers for the Psi interpreter. |
| Native image | Stage source and target | Publish Psi in memory and lower it for the target. |
| Native image | Authorized Psi and target | Lower the supplied Psi for the target. |

The [source pipeline](source-to-psi/source_to_psi.rs) and the [native pipeline](psi-to-native/psi_to_native.rs) meet at [Psi](psi/). For a source program, its checked publication supplies the exact target’s reachable provider choices, completed Build bindings and the behavior exclusions admitted in that same source stage. The host adds accepted package permissions and any reviewed installation assumptions for that deployment. For a saved program, the receiving host decodes the file and authorizes the exact artifact before it requests runtime provisions or native code.

Each pipeline keeps its own [source program forms](source-to-psi/representations/) or [native program forms](psi-to-native/representations/). The two drivers pass a [published Psi program](psi/) in memory. `--emit-psi` asks the source side to save it. A later invocation reopens and authorizes those bytes before admission.

An authored Build runs as a program. The package host supplies the capture and grants. The source compiler resolves the selected Build stage, judges the invocation and publishes the checked program. Build execution runs it in the interpreter, services its host calls and returns its plan. [Build source compilation](source-to-psi/driver/compile_build.rs) · [Build execution](build/evaluation/execution.rs)

For a Product, planning turns the returned declaration and package graph into a compiler request. The source driver publishes Psi before target admission. Saved Psi enters at authorization and joins the same native driver. [Product inputs](build/planning/product.rs) · [Source compilation](source-to-psi/source_to_psi.rs) · [Saved and source native inputs](psi-to-native/driver/inputs/published_application.rs)


Source preparation occasionally needs the value of a constant or a compile-time call. The source driver checks that smaller invocation, executes it in the Build interpreter and returns the value and evaluation record to the requesting stage. [Compile-time source work](source-to-psi/driver/compile_time_invocations/)

Native image production compiles the entry body and any selected private or component bodies with one body driver. An explicit validator request retains the originals needed to verify the emitted objects. [Native body stages](psi-to-native/driver/compile_body.rs) · [Stage validators](psi-to-native/verification/stage_validators.rs)

## Verification placement

During a compilation, each stage trusts its predecessor’s output contract. The compiler checks source proofs, external inputs and installation requirements at the boundaries below. Focused tests and explicit validator requests check the compiler's own output. [Compiler rules](../../../AGENTS.md#conventions-and-workflow) · [Validator selection](../../../wiki/spec/build/command_line.md#independent-stage-validators)

| Boundary | What is checked |
| --- | --- |
| Source to published Psi | The lowered-form checker judges the obligations of the lowered program. Publication ties that verdict to the program it publishes. Correctness also depends on lowering preserving the source program's meaning. [Source stages](source-to-psi/README.md) |
| Saved Psi and installed providers | The receiver decodes the artifact and applies host authorization to the exact bytes and producer. Installation checks the selected providers against the host's grants. The interpreter enforces bounds, access and liveness as it executes. [Psi admission](../../../wiki/spec/terminal-psi/product.md#publication-and-separate-consumption) |
| Psi to a native image | The backend enforces target constraints during emission. Independent format checks run in backend tests. A requested validator checks its documented property against retained inputs. Native/Psi agreement covers the executions and observations compared. [Native emission](../../../wiki/spec/build/machine_state_evidence.md#final-artifact-validation-placement) |
| Foreign packages and Build execution | The package host checks the source custody and acceptance records of foreign dependencies and confines Build services to their grants. [Package trust](../../../wiki/spec/packages/resolver_security.md#package-and-build-separation) |
